//! Pure domain OSC 52 parser, operations, and streaming state machine.
//!
//! Handles OSC 52 escape sequences for reading, writing, and clearing terminal
//! clipboards. Pure domain implementation with no GTK widget dependencies.

pub const MAX_OSC52_PAYLOAD_SIZE: usize = 5 * 1024 * 1024; // 5 MB safety bound

/// Target clipboard buffer for OSC 52 operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Osc52Target {
    Clipboard,     // 'c' - Standard desktop clipboard
    Primary,       // 'p' - Primary selection clipboard
    Secondary,     // 'q' - Secondary selection
    CutBuffer(u8), // '0'..='7' - X11 cut buffers 0-7
}

impl Osc52Target {
    pub fn from_char(c: char) -> Option<Self> {
        match c {
            'c' | 'C' => Some(Self::Clipboard),
            'p' | 'P' => Some(Self::Primary),
            'q' | 'Q' => Some(Self::Secondary),
            '0'..='7' => Some(Self::CutBuffer(c.to_digit(10).unwrap() as u8)),
            _ => None,
        }
    }

    pub fn to_char(self) -> char {
        match self {
            Self::Clipboard => 'c',
            Self::Primary => 'p',
            Self::Secondary => 'q',
            Self::CutBuffer(idx) => char::from_digit(idx as u32, 10).unwrap_or('0'),
        }
    }
}

/// Operation requested by an OSC 52 sequence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Osc52Operation {
    Write(Vec<u8>), // Decoded payload data to write to clipboard
    Query,          // '?' - Client requests clipboard content
    Clear,          // Empty payload - Client requests clipboard clear
}

/// An extracted OSC 52 event with targets and operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Osc52Event {
    pub targets: Vec<Osc52Target>,
    pub operation: Osc52Operation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParserState {
    Ground,
    Escape,
    OscHeader,
    OscTargets,
    OscPayload,
    EscapeInPayload,
}

/// Streaming parser for scanning terminal child output and extracting OSC 52 sequences.
pub struct Osc52StreamParser {
    state: ParserState,
    header_buf: Vec<u8>,
    targets_buf: Vec<u8>,
    payload_buf: Vec<u8>,
}

impl Default for Osc52StreamParser {
    fn default() -> Self {
        Self::new()
    }
}

impl Osc52StreamParser {
    pub fn new() -> Self {
        Self {
            state: ParserState::Ground,
            header_buf: Vec::with_capacity(8),
            targets_buf: Vec::with_capacity(16),
            payload_buf: Vec::with_capacity(1024),
        }
    }

    /// Feeds an incoming byte chunk from child output.
    ///
    /// Returns:
    /// 1. Passthrough bytes that should be forwarded to VTE.
    /// 2. Extracted OSC 52 events.
    pub fn process(&mut self, chunk: &[u8]) -> (Vec<u8>, Vec<Osc52Event>) {
        let mut passthrough = Vec::with_capacity(chunk.len());
        let mut events = Vec::new();

        for &b in chunk {
            match self.state {
                ParserState::Ground => {
                    if b == 0x1B {
                        self.state = ParserState::Escape;
                    } else {
                        passthrough.push(b);
                    }
                }
                ParserState::Escape => {
                    if b == b']' {
                        self.state = ParserState::OscHeader;
                        self.header_buf.clear();
                    } else if b == 0x1B {
                        // Consecutive ESC bytes: emit earlier ESC and stay in Escape
                        passthrough.push(0x1B);
                    } else {
                        self.state = ParserState::Ground;
                        passthrough.push(0x1B);
                        passthrough.push(b);
                    }
                }
                ParserState::OscHeader => {
                    self.header_buf.push(b);
                    if self.header_buf == b"52;" {
                        self.state = ParserState::OscTargets;
                        self.targets_buf.clear();
                    } else if !b"52;".starts_with(&self.header_buf) {
                        // Not an OSC 52 sequence - flush buffered header to passthrough
                        self.state = ParserState::Ground;
                        passthrough.push(0x1B);
                        passthrough.push(b']');
                        passthrough.extend_from_slice(&self.header_buf);
                        self.header_buf.clear();
                    }
                }
                ParserState::OscTargets => {
                    if b == b';' {
                        self.state = ParserState::OscPayload;
                        self.payload_buf.clear();
                    } else if b == 0x07 || b == 0x1B || b == 0x9C || self.targets_buf.len() >= 64 {
                        // Premature terminator or malformed targets - abort sequence
                        self.reset_state();
                    } else {
                        self.targets_buf.push(b);
                    }
                }
                ParserState::OscPayload => {
                    if b == 0x07 || b == 0x9C {
                        // BEL or 8-bit ST terminator
                        if let Some(event) = self.finish_sequence() {
                            events.push(event);
                        }
                        self.reset_state();
                    } else if b == 0x1B {
                        self.state = ParserState::EscapeInPayload;
                    } else if self.payload_buf.len() < MAX_OSC52_PAYLOAD_SIZE {
                        self.payload_buf.push(b);
                    } else {
                        // Exceeded maximum allowable payload - abort sequence safely
                        self.reset_state();
                    }
                }
                ParserState::EscapeInPayload => {
                    if b == b'\\' {
                        // 7-bit ST terminator (\x1b\\)
                        if let Some(event) = self.finish_sequence() {
                            events.push(event);
                        }
                        self.reset_state();
                    } else if b == 0x1B {
                        // Consecutive ESC in payload: record first ESC and stay in EscapeInPayload
                        if self.payload_buf.len() + 1 < MAX_OSC52_PAYLOAD_SIZE {
                            self.payload_buf.push(0x1B);
                        }
                    } else {
                        self.state = ParserState::OscPayload;
                        if self.payload_buf.len() + 2 < MAX_OSC52_PAYLOAD_SIZE {
                            self.payload_buf.push(0x1B);
                            self.payload_buf.push(b);
                        }
                    }
                }
            }
        }

        (passthrough, events)
    }

    fn reset_state(&mut self) {
        self.state = ParserState::Ground;
        self.header_buf.clear();
        self.targets_buf.clear();
        self.payload_buf.clear();
    }

    fn finish_sequence(&self) -> Option<Osc52Event> {
        let targets_str = std::str::from_utf8(&self.targets_buf).unwrap_or("");
        let mut targets: Vec<Osc52Target> = targets_str
            .chars()
            .filter_map(Osc52Target::from_char)
            .collect();

        // Default target is Clipboard ('c') if empty
        if targets.is_empty() {
            targets.push(Osc52Target::Clipboard);
        }

        let payload_str = std::str::from_utf8(&self.payload_buf).unwrap_or("").trim();

        let operation = if payload_str == "?" {
            Osc52Operation::Query
        } else if payload_str.is_empty() {
            Osc52Operation::Clear
        } else {
            let decoded = glib::base64_decode(payload_str);
            Osc52Operation::Write(decoded)
        };

        Some(Osc52Event { targets, operation })
    }
}

/// Encodes an OSC 52 response string for query operations.
pub fn encode_osc52_response(target: Osc52Target, data: &[u8]) -> String {
    let encoded = glib::base64_encode(data);
    format!("\x1b]52;{};{}\x1b\\", target.to_char(), encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_osc52_target_from_to_char() {
        assert_eq!(Osc52Target::from_char('c'), Some(Osc52Target::Clipboard));
        assert_eq!(Osc52Target::from_char('C'), Some(Osc52Target::Clipboard));
        assert_eq!(Osc52Target::from_char('p'), Some(Osc52Target::Primary));
        assert_eq!(Osc52Target::from_char('P'), Some(Osc52Target::Primary));
        assert_eq!(Osc52Target::from_char('q'), Some(Osc52Target::Secondary));
        assert_eq!(Osc52Target::from_char('0'), Some(Osc52Target::CutBuffer(0)));
        assert_eq!(Osc52Target::from_char('7'), Some(Osc52Target::CutBuffer(7)));
        assert_eq!(Osc52Target::from_char('8'), None);
        assert_eq!(Osc52Target::from_char('x'), None);

        assert_eq!(Osc52Target::Clipboard.to_char(), 'c');
        assert_eq!(Osc52Target::Primary.to_char(), 'p');
        assert_eq!(Osc52Target::Secondary.to_char(), 'q');
        assert_eq!(Osc52Target::CutBuffer(3).to_char(), '3');
    }

    #[test]
    fn test_osc52_write_operation_with_bel() {
        let mut parser = Osc52StreamParser::new();
        let payload = glib::base64_encode(b"Hello World");
        let seq = format!("\x1b]52;c;{}\x07", payload);

        let (passthrough, events) = parser.process(seq.as_bytes());
        assert!(passthrough.is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].targets, vec![Osc52Target::Clipboard]);
        assert_eq!(events[0].operation, Osc52Operation::Write(b"Hello World".to_vec()));
    }

    #[test]
    fn test_osc52_write_operation_with_st() {
        let mut parser = Osc52StreamParser::new();
        let payload = glib::base64_encode(b"Rust Tilix");
        let seq = format!("\x1b]52;p;{}\x1b\\", payload);

        let (passthrough, events) = parser.process(seq.as_bytes());
        assert!(passthrough.is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].targets, vec![Osc52Target::Primary]);
        assert_eq!(events[0].operation, Osc52Operation::Write(b"Rust Tilix".to_vec()));
    }

    #[test]
    fn test_osc52_write_operation_with_8bit_st() {
        let mut parser = Osc52StreamParser::new();
        let payload = glib::base64_encode(b"8-bit terminator");
        let mut seq = format!("\x1b]52;c;{}", payload).into_bytes();
        seq.push(0x9C);

        let (passthrough, events) = parser.process(&seq);
        assert!(passthrough.is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].targets, vec![Osc52Target::Clipboard]);
        assert_eq!(events[0].operation, Osc52Operation::Write(b"8-bit terminator".to_vec()));
    }

    #[test]
    fn test_osc52_query_operation() {
        let mut parser = Osc52StreamParser::new();
        let seq = b"\x1b]52;c;?\x07";

        let (passthrough, events) = parser.process(seq);
        assert!(passthrough.is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].targets, vec![Osc52Target::Clipboard]);
        assert_eq!(events[0].operation, Osc52Operation::Query);
    }

    #[test]
    fn test_osc52_clear_operation() {
        let mut parser = Osc52StreamParser::new();
        let seq = b"\x1b]52;c;\x07";

        let (passthrough, events) = parser.process(seq);
        assert!(passthrough.is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].targets, vec![Osc52Target::Clipboard]);
        assert_eq!(events[0].operation, Osc52Operation::Clear);
    }

    #[test]
    fn test_osc52_compound_targets_and_default() {
        let mut parser = Osc52StreamParser::new();
        // Compound target 'cp'
        let payload = glib::base64_encode(b"compound");
        let seq1 = format!("\x1b]52;cp;{}\x07", payload);
        let (_, events1) = parser.process(seq1.as_bytes());
        assert_eq!(events1.len(), 1);
        assert_eq!(events1[0].targets, vec![Osc52Target::Clipboard, Osc52Target::Primary]);

        // Empty target defaults to Clipboard
        let seq2 = format!("\x1b]52;;{}\x07", payload);
        let (_, events2) = parser.process(seq2.as_bytes());
        assert_eq!(events2.len(), 1);
        assert_eq!(events2[0].targets, vec![Osc52Target::Clipboard]);
    }

    #[test]
    fn test_osc52_streaming_split_chunks() {
        let mut parser = Osc52StreamParser::new();
        let chunk1 = b"prefix text \x1b]52;c";
        let chunk2 = b";aGVsbG8=";
        let chunk3 = b"\x1b\\suffix text";

        let (pt1, ev1) = parser.process(chunk1);
        assert_eq!(pt1, b"prefix text ");
        assert!(ev1.is_empty());

        let (pt2, ev2) = parser.process(chunk2);
        assert!(pt2.is_empty());
        assert!(ev2.is_empty());

        let (pt3, ev3) = parser.process(chunk3);
        assert_eq!(pt3, b"suffix text");
        assert_eq!(ev3.len(), 1);
        assert_eq!(ev3[0].targets, vec![Osc52Target::Clipboard]);
        assert_eq!(ev3[0].operation, Osc52Operation::Write(b"hello".to_vec()));
    }

    #[test]
    fn test_osc52_non_osc52_passthrough() {
        let mut parser = Osc52StreamParser::new();

        // Regular terminal text and CSI sequences
        let input = b"Hello \x1b[31mRed\x1b[0m World\n";
        let (pt, ev) = parser.process(input);
        assert_eq!(pt, input);
        assert!(ev.is_empty());

        // Other OSC sequences, e.g. OSC 7 (current working directory)
        let osc7 = b"\x1b]7;file://localhost/home/user\x07Normal text";
        let (pt7, ev7) = parser.process(osc7);
        assert_eq!(pt7, osc7);
        assert!(ev7.is_empty());
    }

    #[test]
    fn test_osc52_payload_size_limit() {
        let mut parser = Osc52StreamParser::new();
        let header = b"\x1b]52;c;";
        let (pt1, ev1) = parser.process(header);
        assert!(pt1.is_empty());
        assert!(ev1.is_empty());

        // Feed chunks exceeding MAX_OSC52_PAYLOAD_SIZE (5 MB)
        let big_chunk = vec![b'A'; 1024 * 1024];
        for _ in 0..6 {
            let (_pt, ev) = parser.process(&big_chunk);
            assert!(ev.is_empty(), "No OSC 52 events should be emitted during or after overflow");
        }

        // Parser should have reset due to limit overflow, so subsequent terminator produces no event
        let (pt_term, ev_term) = parser.process(b"\x07Normal");
        assert_eq!(pt_term, b"\x07Normal");
        assert!(ev_term.is_empty());
    }

    #[test]
    fn test_encode_osc52_response() {
        let resp = encode_osc52_response(Osc52Target::Clipboard, b"secret");
        assert_eq!(resp, format!("\x1b]52;c;{}\x1b\\", glib::base64_encode(b"secret")));
    }
}
