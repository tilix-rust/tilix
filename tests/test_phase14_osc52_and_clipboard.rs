#![allow(dead_code)]
#![allow(unused_imports)]
#![allow(deprecated)]

#[path = "../src/model/mod.rs"]
pub mod model;

#[path = "../src/pty/mod.rs"]
pub mod pty;

#[path = "../src/ui/mod.rs"]
pub mod ui;

#[path = "../src/app.rs"]
pub mod app;

use std::sync::mpsc;
use std::time::Duration;

use gtk4 as gtk;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;

use model::keybindings::{ActionCategory, ACTION_CATALOG};
use model::layout::PaneId;
use model::profile::Profile;
use pty::osc52::{
    encode_osc52_response, Osc52Event, Osc52Operation, Osc52StreamParser, Osc52Target,
    MAX_OSC52_PAYLOAD_SIZE,
};
use pty::proxy::{ProxyPtyPair, PtyProxy};
use ui::session_view::SessionView;
use ui::terminal_pane::TerminalPane;
use ui::window::{run_gtk_test, TilixWindow};

#[test]
fn test_phase14_osc52_parser_target_permutations() {
    let mut parser = Osc52StreamParser::new();

    // Standard targets
    let targets_to_test = [
        ('c', Osc52Target::Clipboard),
        ('p', Osc52Target::Primary),
        ('q', Osc52Target::Secondary),
        ('0', Osc52Target::CutBuffer(0)),
        ('1', Osc52Target::CutBuffer(1)),
        ('7', Osc52Target::CutBuffer(7)),
    ];

    for (ch, expected_target) in targets_to_test {
        let seq = format!("\x1b]52;{};?\x07", ch);
        let (pt, events) = parser.process(seq.as_bytes());
        assert!(pt.is_empty());
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].targets, vec![expected_target]);
        assert_eq!(events[0].operation, Osc52Operation::Query);
    }

    // Compound target: 'cp'
    let compound_seq = b"\x1b]52;cp;?\x07";
    let (pt_comp, ev_comp) = parser.process(compound_seq);
    assert!(pt_comp.is_empty());
    assert_eq!(ev_comp.len(), 1);
    assert_eq!(ev_comp[0].targets, vec![Osc52Target::Clipboard, Osc52Target::Primary]);

    // Empty target: defaults to Clipboard ('c')
    let default_seq = b"\x1b]52;;?\x07";
    let (pt_def, ev_def) = parser.process(default_seq);
    assert!(pt_def.is_empty());
    assert_eq!(ev_def.len(), 1);
    assert_eq!(ev_def[0].targets, vec![Osc52Target::Clipboard]);
}

#[test]
fn test_phase14_osc52_parser_operations() {
    let mut parser = Osc52StreamParser::new();

    // Write operation
    let payload = glib::base64_encode(b"Clipboard Content");
    let write_seq = format!("\x1b]52;c;{}\x07", payload);
    let (pt_w, ev_w) = parser.process(write_seq.as_bytes());
    assert!(pt_w.is_empty());
    assert_eq!(ev_w.len(), 1);
    assert_eq!(ev_w[0].targets, vec![Osc52Target::Clipboard]);
    assert_eq!(ev_w[0].operation, Osc52Operation::Write(b"Clipboard Content".to_vec()));

    // Query operation
    let query_seq = b"\x1b]52;c;?\x07";
    let (pt_q, ev_q) = parser.process(query_seq);
    assert!(pt_q.is_empty());
    assert_eq!(ev_q.len(), 1);
    assert_eq!(ev_q[0].targets, vec![Osc52Target::Clipboard]);
    assert_eq!(ev_q[0].operation, Osc52Operation::Query);

    // Clear operation
    let clear_seq = b"\x1b]52;c;\x07";
    let (pt_c, ev_c) = parser.process(clear_seq);
    assert!(pt_c.is_empty());
    assert_eq!(ev_c.len(), 1);
    assert_eq!(ev_c[0].targets, vec![Osc52Target::Clipboard]);
    assert_eq!(ev_c[0].operation, Osc52Operation::Clear);
}

#[test]
fn test_phase14_osc52_streaming_chunks() {
    let mut parser = Osc52StreamParser::new();

    // BEL terminator split across chunks
    let chunk1 = b"some terminal output \x1b]52;c";
    let chunk2 = b";aGVsbG8=";
    let chunk3 = b"\x07trailing output";

    let (pt1, ev1) = parser.process(chunk1);
    assert_eq!(pt1, b"some terminal output ");
    assert!(ev1.is_empty());

    let (pt2, ev2) = parser.process(chunk2);
    assert!(pt2.is_empty());
    assert!(ev2.is_empty());

    let (pt3, ev3) = parser.process(chunk3);
    assert_eq!(pt3, b"trailing output");
    assert_eq!(ev3.len(), 1);
    assert_eq!(ev3[0].operation, Osc52Operation::Write(b"hello".to_vec()));

    // 7-bit ST terminator (\x1b\\) split across chunks
    let st_chunk1 = b"\x1b]52;p;d29ybGQ=\x1b";
    let st_chunk2 = b"\\more text";
    let (st_pt1, st_ev1) = parser.process(st_chunk1);
    assert!(st_pt1.is_empty());
    assert!(st_ev1.is_empty());

    let (st_pt2, st_ev2) = parser.process(st_chunk2);
    assert_eq!(st_pt2, b"more text");
    assert_eq!(st_ev2.len(), 1);
    assert_eq!(st_ev2[0].targets, vec![Osc52Target::Primary]);
    assert_eq!(st_ev2[0].operation, Osc52Operation::Write(b"world".to_vec()));

    // 8-bit ST terminator (0x9C)
    let mut st8_seq = format!("\x1b]52;q;{}", glib::base64_encode(b"secondary")).into_bytes();
    st8_seq.push(0x9C);
    let (st8_pt, st8_ev) = parser.process(&st8_seq);
    assert!(st8_pt.is_empty());
    assert_eq!(st8_ev.len(), 1);
    assert_eq!(st8_ev[0].targets, vec![Osc52Target::Secondary]);
    assert_eq!(st8_ev[0].operation, Osc52Operation::Write(b"secondary".to_vec()));
}

#[test]
fn test_phase14_osc52_passthrough_cleanliness() {
    let mut parser = Osc52StreamParser::new();

    // Standard ANSI and CSI output
    let terminal_text = b"\x1b[?2004huser@host:~$ \x1b[01;32mls\x1b[00m\r\nfile.txt\r\n";
    let (pt1, ev1) = parser.process(terminal_text);
    assert_eq!(pt1, terminal_text);
    assert!(ev1.is_empty());

    // Non-52 OSC sequence, e.g. OSC 7 (directory notification)
    let osc7_seq = b"\x1b]7;file://hostname/home/user/code\x07next text";
    let (pt2, ev2) = parser.process(osc7_seq);
    assert_eq!(pt2, osc7_seq);
    assert!(ev2.is_empty());
}

#[test]
fn test_phase14_osc52_payload_size_limit() {
    let mut parser = Osc52StreamParser::new();
    let header = b"\x1b]52;c;";
    let (pt1, ev1) = parser.process(header);
    assert!(pt1.is_empty());
    assert!(ev1.is_empty());

    // Send chunks totaling more than MAX_OSC52_PAYLOAD_SIZE (5 MB)
    let big_chunk = vec![b'B'; 1024 * 1024];
    for _ in 0..6 {
        let (_pt, ev) = parser.process(&big_chunk);
        assert!(ev.is_empty(), "Exceeding safety limit must never emit an event");
    }

    // After overflow, state resets to Ground
    let (pt_term, ev_term) = parser.process(b"\x07PostOverflowText");
    assert_eq!(pt_term, b"\x07PostOverflowText");
    assert!(ev_term.is_empty());
}

#[test]
fn test_phase14_pty_proxy_forwarding_and_interception() {
    let proxy = PtyProxy::new().expect("Proxy creation should succeed");
    let (tx, rx) = mpsc::channel();

    proxy.start(
        move |event| {
            let _ = tx.send(event);
        },
        |targets| {
            if targets.contains(&Osc52Target::Clipboard) {
                Some(b"mock_queried_clipboard".to_vec())
            } else {
                None
            }
        },
    );

    // 1. Forwarding outer_master -> inner_slave (stdin)
    let user_keystrokes = b"echo 'hello from user'\n";
    unsafe {
        libc::write(
            proxy.outer_master_fd(),
            user_keystrokes.as_ptr() as *const libc::c_void,
            user_keystrokes.len(),
        );
    }

    let mut buf = [0u8; 128];
    let mut received_len = 0;
    for _ in 0..10 {
        let n = unsafe {
            libc::read(
                proxy.inner_slave_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n > 0 {
            received_len = n as usize;
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(&buf[..received_len], user_keystrokes);

    // 2. OSC 52 Write interception inner_slave -> outer_slave
    let osc52_msg = format!("\x1b]52;c;{}\x07", glib::base64_encode(b"proxy_clip_test"));
    unsafe {
        libc::write(
            proxy.inner_slave_fd(),
            osc52_msg.as_ptr() as *const libc::c_void,
            osc52_msg.len(),
        );
    }

    let event = rx
        .recv_timeout(Duration::from_millis(500))
        .expect("Should receive OSC 52 write event");
    assert_eq!(event.targets, vec![Osc52Target::Clipboard]);
    assert_eq!(event.operation, Osc52Operation::Write(b"proxy_clip_test".to_vec()));

    // 3. Window size propagation
    proxy.set_window_size(30, 100);
    proxy.sync_size_from_outer();

    proxy.shutdown();
    // Drop automatically closes all remaining FDs cleanly
}

#[test]
fn test_phase14_profile_clipboard_defaults_and_serde() {
    let p = Profile::default();
    assert!(!p.copy_on_select);
    assert!(p.enable_osc52);
    assert!(!p.osc52_allow_query);

    // Deserialization without Phase 14 keys
    let legacy_json = r#"{
        "id": "legacy-profile",
        "name": "Legacy Test"
    }"#;
    let deserialized: Profile =
        serde_json::from_str(legacy_json).expect("Legacy profile JSON should deserialize");
    assert!(!deserialized.copy_on_select);
    assert!(deserialized.enable_osc52);
    assert!(!deserialized.osc52_allow_query);

    // Roundtrip with custom settings
    let custom = Profile {
        copy_on_select: true,
        enable_osc52: false,
        osc52_allow_query: true,
        ..Default::default()
    };

    let json = serde_json::to_string(&custom).expect("Serialization should succeed");
    let roundtrip: Profile =
        serde_json::from_str(&json).expect("Deserialization should succeed");
    assert!(roundtrip.copy_on_select);
    assert!(!roundtrip.enable_osc52);
    assert!(roundtrip.osc52_allow_query);
}

#[test]
fn test_phase14_keybinding_catalog_clipboard_actions() {
    assert_eq!(ACTION_CATALOG.len(), 32);
    assert_eq!(ActionCategory::Clipboard.title(), "Clipboard & Edit");

    let clipboard_actions: Vec<_> = ACTION_CATALOG
        .iter()
        .filter(|d| d.category == ActionCategory::Clipboard)
        .collect();
    assert_eq!(clipboard_actions.len(), 5);

    let copy_act = clipboard_actions.iter().find(|d| d.id == "win.copy").unwrap();
    assert_eq!(copy_act.title, "Copy");
    assert_eq!(copy_act.default_accels, &["<Primary><Shift>c", "<Primary>Insert"]);

    let copy_html_act = clipboard_actions.iter().find(|d| d.id == "win.copy-html").unwrap();
    assert_eq!(copy_html_act.title, "Copy as HTML");
    assert!(copy_html_act.default_accels.is_empty());

    let paste_act = clipboard_actions.iter().find(|d| d.id == "win.paste").unwrap();
    assert_eq!(paste_act.title, "Paste");
    assert_eq!(paste_act.default_accels, &["<Primary><Shift>v", "<Shift>Insert"]);

    let paste_primary_act = clipboard_actions.iter().find(|d| d.id == "win.paste-primary").unwrap();
    assert_eq!(paste_primary_act.title, "Paste Primary Selection");
    assert!(paste_primary_act.default_accels.is_empty());

    let select_all_act = clipboard_actions.iter().find(|d| d.id == "win.select-all").unwrap();
    assert_eq!(select_all_act.title, "Select All");
    assert_eq!(select_all_act.default_accels, &["<Primary><Shift>a"]);
}

#[test]
fn test_phase14_terminal_pane_clipboard_methods() {
    run_gtk_test(|| {
        let pane = TerminalPane::new(PaneId(1), None);

        // Exercise all clipboard methods
        pane.copy_clipboard();
        pane.copy_html();
        pane.paste_clipboard();
        pane.paste_primary();
        pane.select_all();

        // Verify context menu is attached
        let menu_model = pane.context_menu_model();
        assert!(menu_model.is_some(), "Context menu model must be attached to terminal");
    });
}

#[test]
fn test_phase14_session_view_clipboard_dispatch() {
    run_gtk_test(|| {
        let session_view = SessionView::new();

        // Active pane clipboard dispatch methods
        session_view.copy_clipboard_active();
        session_view.copy_html_active();
        session_view.paste_clipboard_active();
        session_view.paste_primary_active();
        session_view.select_all_active();
    });
}

#[test]
fn test_phase14_window_clipboard_actions_registered() {
    run_gtk_test(|| {
        let app = adw::Application::builder()
            .application_id("com.gexperts.Tilix.TestPhase14")
            .build();

        let window = TilixWindow::new_empty(&app);
        let win_widget = window.window();

        let actions_to_check = [
            "copy",
            "copy-html",
            "paste",
            "paste-primary",
            "cut",
            "select-all",
        ];

        for action_name in actions_to_check {
            assert!(
                win_widget.lookup_action(action_name).is_some(),
                "Action '{}' must be registered on TilixWindow",
                action_name
            );
        }
    });
}

#[test]
fn test_phase14_pty_proxy_fd_lifecycle_and_size_sync() {
    let proxy = PtyProxy::new().expect("PtyProxy should create");
    let inner_m = proxy.inner_master_fd();
    let inner_s = proxy.inner_slave_fd();
    let outer_m = proxy.outer_master_fd();
    let outer_s = proxy.outer_slave_fd();

    assert!(inner_m > 0);
    assert!(inner_s > 0);
    assert!(outer_m > 0);
    assert!(outer_s > 0);

    // Test explicit set_window_size
    proxy.set_window_size(42, 137);
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let res = unsafe { libc::ioctl(inner_m, libc::TIOCGWINSZ, &mut ws) };
    assert_eq!(res, 0);
    assert_eq!(ws.ws_row, 42);
    assert_eq!(ws.ws_col, 137);

    // Calling set_window_size with identical dimensions is safely deduplicated
    proxy.set_window_size(42, 137);
    let mut ws_dup: libc::winsize = unsafe { std::mem::zeroed() };
    let res_dup = unsafe { libc::ioctl(inner_m, libc::TIOCGWINSZ, &mut ws_dup) };
    assert_eq!(res_dup, 0);
    assert_eq!(ws_dup.ws_row, 42);
    assert_eq!(ws_dup.ws_col, 137);

    // Test sync_size_from_outer
    let outer_ws = libc::winsize {
        ws_row: 55,
        ws_col: 120,
        ws_xpixel: 0,
        ws_ypixel: 0,
    };
    unsafe {
        libc::ioctl(outer_s, libc::TIOCSWINSZ, &outer_ws);
    }
    proxy.sync_size_from_outer();
    let mut read_ws: libc::winsize = unsafe { std::mem::zeroed() };
    let res = unsafe { libc::ioctl(inner_m, libc::TIOCGWINSZ, &mut read_ws) };
    assert_eq!(res, 0);
    assert_eq!(read_ws.ws_row, 55);
    assert_eq!(read_ws.ws_col, 120);

    // Test close_inner_slave disarms and closes FD
    proxy.close_inner_slave();
    assert_eq!(proxy.inner_slave_fd(), -1);

    // Test take_outer_master takes ownership
    let taken_om = proxy.take_outer_master().expect("outer master should be present");
    assert_eq!(taken_om, outer_m);
    assert_eq!(proxy.outer_master_fd(), -1);
    unsafe { libc::close(taken_om); }

    // Drop proxy cleans up remaining inner_m and outer_s cleanly without double-free
    drop(proxy);
}

#[test]
fn test_phase14_pty_proxy_ctrl_d_and_cursor_query_raw_passthrough() {
    use std::time::Duration;

    let proxy = PtyProxy::new().expect("PtyProxy should initialize in raw mode");
    proxy.start(|_| {}, |_| None);

    // Shell runs in raw mode on inner_slave
    unsafe {
        let mut tio: libc::termios = std::mem::zeroed();
        libc::cfmakeraw(&mut tio);
        libc::tcsetattr(proxy.inner_slave_fd(), libc::TCSANOW, &tio);
        let flags = libc::fcntl(proxy.inner_slave_fd(), libc::F_GETFL);
        libc::fcntl(proxy.inner_slave_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    // 1. Verify Ctrl+D (\x04) is delivered as a 1-byte raw payload rather than swallowed as EOF (0 bytes)
    let ctrl_d = b"\x04";
    unsafe {
        libc::write(
            proxy.outer_master_fd(),
            ctrl_d.as_ptr() as *const libc::c_void,
            ctrl_d.len(),
        );
    }

    let mut buf = [0u8; 16];
    let mut received_len = 0;
    for _ in 0..20 {
        let n = unsafe {
            libc::read(
                proxy.inner_slave_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n > 0 {
            received_len = n as usize;
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    assert_eq!(received_len, 1);
    assert_eq!(buf[0], 0x04);

    // 2. Verify cursor position report (\x1b[1;1R without newline) is delivered immediately without line buffering
    let cursor_report = b"\x1b[1;1R";
    unsafe {
        libc::write(
            proxy.outer_master_fd(),
            cursor_report.as_ptr() as *const libc::c_void,
            cursor_report.len(),
        );
    }

    let mut report_buf = [0u8; 32];
    let mut report_len = 0;
    for _ in 0..20 {
        let n = unsafe {
            libc::read(
                proxy.inner_slave_fd(),
                report_buf.as_mut_ptr() as *mut libc::c_void,
                report_buf.len(),
            )
        };
        if n > 0 {
            report_len = n as usize;
            break;
        }
        std::thread::sleep(Duration::from_millis(15));
    }
    assert_eq!(&report_buf[..report_len], cursor_report);

    proxy.shutdown();
}

#[test]
fn test_phase14_open_raw_preserves_verase_backspace() {
    let pair = ProxyPtyPair::open_raw().expect("open_raw should succeed");
    let mut tio: libc::termios = unsafe { std::mem::zeroed() };
    let res = unsafe { libc::tcgetattr(pair.slave_fd, &mut tio) };
    assert_eq!(res, 0);

    // Verify VERASE is set to 0x7f (ASCII DEL) so VTE backspace binding detects it
    assert_eq!(
        tio.c_cc[libc::VERASE],
        0x7f,
        "VERASE must be 0x7f for Backspace to function properly in VTE"
    );

    // Verify raw mode is active (ICANON and ECHO cleared)
    assert_eq!(tio.c_lflag & libc::ICANON, 0);
    assert_eq!(tio.c_lflag & libc::ECHO, 0);
}

