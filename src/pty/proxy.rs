//! In-process PTY proxy interceptor for OSC 52 sequences.
//!
//! Creates dual PTY pairs using `libc::openpty`:
//! - Inner PTY: child shell/process connects to inner slave.
//! - Outer PTY: VTE connects to outer master.
//!
//! Background threads forward stdin/stdout and intercept OSC 52 sequences.

use std::os::unix::io::RawFd;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, Ordering};
use std::sync::Arc;
use std::thread;

use crate::pty::osc52::{encode_osc52_response, Osc52Event, Osc52Operation, Osc52StreamParser, Osc52Target};

/// A PTY master/slave file descriptor pair created via `libc::openpty`.
#[derive(Debug)]
pub struct ProxyPtyPair {
    pub master_fd: RawFd,
    pub slave_fd: RawFd,
}

impl ProxyPtyPair {
    pub fn open() -> Result<Self, std::io::Error> {
        let mut master: libc::c_int = 0;
        let mut slave: libc::c_int = 0;
        let res = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if res != 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(Self {
            master_fd: master,
            slave_fd: slave,
        })
    }

    /// Disarms RAII drop closing and returns the raw file descriptors.
    pub fn into_raw(mut self) -> (RawFd, RawFd) {
        let master = self.master_fd;
        let slave = self.slave_fd;
        self.master_fd = -1;
        self.slave_fd = -1;
        (master, slave)
    }

    /// Opens a PTY pair with raw attributes on the slave end to prevent
    /// canonical line buffering, echo, or signal translations during proxy forwarding,
    /// while preserving standard control characters (e.g. Backspace VERASE = 0x7f).
    pub fn open_raw() -> Result<Self, std::io::Error> {
        let mut master: libc::c_int = 0;
        let mut slave: libc::c_int = 0;
        let res = unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if res != 0 {
            return Err(std::io::Error::last_os_error());
        }
        unsafe {
            let mut raw_tio: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(slave, &mut raw_tio) == 0 {
                libc::cfmakeraw(&mut raw_tio);
                // Explicitly ensure standard Backspace erase code (ASCII DEL: 0x7f)
                raw_tio.c_cc[libc::VERASE] = 0x7f;
                libc::tcsetattr(slave, libc::TCSANOW, &raw_tio);
            }
        }
        Ok(Self {
            master_fd: master,
            slave_fd: slave,
        })
    }
}

impl Drop for ProxyPtyPair {
    fn drop(&mut self) {
        if self.master_fd >= 0 {
            unsafe { libc::close(self.master_fd); }
            self.master_fd = -1;
        }
        if self.slave_fd >= 0 {
            unsafe { libc::close(self.slave_fd); }
            self.slave_fd = -1;
        }
    }
}

/// Dual-PTY proxy interceptor forwarding I/O and filtering OSC 52 sequences.
pub struct PtyProxy {
    inner_master: AtomicI32,
    inner_slave: AtomicI32,
    outer_master: AtomicI32,
    outer_slave: Arc<AtomicI32>,
    running: Arc<AtomicBool>,
    last_rows: AtomicU16,
    last_cols: AtomicU16,
}

impl PtyProxy {
    pub fn new() -> Result<Self, std::io::Error> {
        let (inner_master, inner_slave) = ProxyPtyPair::open()?.into_raw();
        let (outer_master, outer_slave) = ProxyPtyPair::open_raw()?.into_raw();

        Ok(Self {
            inner_master: AtomicI32::new(inner_master),
            inner_slave: AtomicI32::new(inner_slave),
            outer_master: AtomicI32::new(outer_master),
            outer_slave: Arc::new(AtomicI32::new(outer_slave)),
            running: Arc::new(AtomicBool::new(true)),
            last_rows: AtomicU16::new(0),
            last_cols: AtomicU16::new(0),
        })
    }

    pub fn inner_slave_fd(&self) -> RawFd {
        self.inner_slave.load(Ordering::SeqCst)
    }

    pub fn inner_master_fd(&self) -> RawFd {
        self.inner_master.load(Ordering::SeqCst)
    }

    pub fn outer_master_fd(&self) -> RawFd {
        self.outer_master.load(Ordering::SeqCst)
    }

    pub fn outer_slave_fd(&self) -> RawFd {
        self.outer_slave.load(Ordering::SeqCst)
    }

    fn close_slot(slot: &AtomicI32) {
        let fd = slot.swap(-1, Ordering::SeqCst);
        if fd >= 0 {
            unsafe {
                libc::close(fd);
            }
        }
    }

    /// Closes the inner slave descriptor in this process.
    ///
    /// Must be called by the parent process after spawning child process
    /// so the inner master receives EOF when child process exits.
    pub fn close_inner_slave(&self) {
        Self::close_slot(&self.inner_slave);
    }

    /// Closes the outer master descriptor if not already taken.
    pub fn close_outer_master(&self) {
        Self::close_slot(&self.outer_master);
    }

    /// Takes ownership of the outer master file descriptor.
    pub fn take_outer_master(&self) -> Option<RawFd> {
        let fd = self.outer_master.swap(-1, Ordering::SeqCst);
        if fd >= 0 {
            Some(fd)
        } else {
            None
        }
    }

    /// Spawns background worker threads to forward I/O and intercept OSC 52 events.
    pub fn start<F, Q>(&self, on_osc52_event: F, on_query: Q)
    where
        F: Fn(Osc52Event) + Send + Sync + 'static,
        Q: Fn(Vec<Osc52Target>) -> Option<Vec<u8>> + Send + Sync + 'static,
    {
        let running = Arc::clone(&self.running);
        let on_event = Arc::new(on_osc52_event);
        let on_query_arc = Arc::new(on_query);

        let outer_slave = self.outer_slave_fd();
        let inner_master = self.inner_master_fd();

        // Thread 1: User Input (outer_slave -> inner_master)
        let r1 = Arc::clone(&running);
        thread::Builder::new()
            .name("tilix-pty-input".into())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                while r1.load(Ordering::Relaxed) {
                    let n = unsafe { libc::read(outer_slave, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                    if n <= 0 {
                        break;
                    }
                    let _ = unsafe { libc::write(inner_master, buf.as_ptr() as *const libc::c_void, n as usize) };
                }
            })
            .expect("Failed to spawn pty input forwarder");

        // Thread 2: Child Output (inner_master -> parser -> outer_slave)
        let r2 = Arc::clone(&running);
        let running_exit = Arc::clone(&running);
        let outer_slave_for_close = Arc::clone(&self.outer_slave);
        thread::Builder::new()
            .name("tilix-pty-output".into())
            .spawn(move || {
                let mut parser = Osc52StreamParser::new();
                let mut buf = [0u8; 4096];
                while r2.load(Ordering::Relaxed) {
                    let n = unsafe { libc::read(inner_master, buf.as_mut_ptr() as *mut libc::c_void, buf.len()) };
                    if n <= 0 {
                        break;
                    }
                    let (passthrough, events) = parser.process(&buf[..n as usize]);
                    if !passthrough.is_empty() {
                        let _ = unsafe {
                            libc::write(
                                outer_slave,
                                passthrough.as_ptr() as *const libc::c_void,
                                passthrough.len(),
                            )
                        };
                    }
                    for event in events {
                        if event.operation == Osc52Operation::Query {
                            if let Some(content) = on_query_arc(event.targets.clone()) {
                                if let Some(&target) = event.targets.first() {
                                    let reply = encode_osc52_response(target, &content);
                                    let _ = unsafe {
                                        libc::write(
                                            inner_master,
                                            reply.as_ptr() as *const libc::c_void,
                                            reply.len(),
                                        )
                                    };
                                }
                            }
                        } else {
                            on_event(event);
                        }
                    }
                }
                running_exit.store(false, Ordering::Relaxed);
                // Disarm/close outer_slave so outer_master (VTE) sees EOF and input thread unblocks
                Self::close_slot(&outer_slave_for_close);
            })
            .expect("Failed to spawn pty output forwarder");
    }

    /// Propagates window dimensions to the inner PTY master.
    /// Deduplicates calls so identical dimensions do not generate redundant kernel SIGWINCH signals.
    pub fn set_window_size(&self, rows: u16, cols: u16) {
        if rows == 0 || cols == 0 {
            return;
        }
        let prev_r = self.last_rows.swap(rows, Ordering::SeqCst);
        let prev_c = self.last_cols.swap(cols, Ordering::SeqCst);
        if prev_r == rows && prev_c == cols {
            return;
        }

        let ws = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        let im = self.inner_master_fd();
        if im >= 0 {
            unsafe {
                libc::ioctl(im, libc::TIOCSWINSZ, &ws);
            }
        }
    }

    /// Synchronizes window dimensions from outer slave to inner master.
    pub fn sync_size_from_outer(&self) {
        let os = self.outer_slave_fd();
        if os < 0 {
            return;
        }
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        if unsafe { libc::ioctl(os, libc::TIOCGWINSZ, &mut ws) } == 0
            && ws.ws_row > 0
            && ws.ws_col > 0
        {
            self.set_window_size(ws.ws_row, ws.ws_col);
        }
    }

    pub fn shutdown(&self) {
        self.running.store(false, Ordering::Relaxed);
    }
}

impl Drop for PtyProxy {
    fn drop(&mut self) {
        self.shutdown();
        Self::close_slot(&self.inner_master);
        Self::close_slot(&self.inner_slave);
        Self::close_slot(&self.outer_master);
        Self::close_slot(&self.outer_slave);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proxy_pty_pair_open() {
        let pair = ProxyPtyPair::open().expect("openpty should succeed");
        assert!(pair.master_fd > 0);
        assert!(pair.slave_fd > 0);
        assert_ne!(pair.master_fd, pair.slave_fd);
        // Automatic Drop closes both master_fd and slave_fd
    }

    #[test]
    fn test_pty_proxy_creation_and_fds() {
        let proxy = PtyProxy::new().expect("PtyProxy::new should succeed");
        assert!(proxy.inner_master_fd() > 0);
        assert!(proxy.inner_slave_fd() > 0);
        assert!(proxy.outer_master_fd() > 0);
        assert!(proxy.outer_slave_fd() > 0);

        // Window resize should not crash
        proxy.set_window_size(24, 80);
        proxy.sync_size_from_outer();

        // Testing close_inner_slave and take_outer_master
        proxy.close_inner_slave();
        assert_eq!(proxy.inner_slave_fd(), -1);

        let om = proxy.take_outer_master().expect("outer master should be available");
        assert!(om > 0);
        assert_eq!(proxy.outer_master_fd(), -1);
        unsafe { libc::close(om); }

        proxy.shutdown();
        // Drop automatically closes remaining FDs (inner_master, outer_slave)
    }

    #[test]
    fn test_pty_proxy_forwarding_and_osc52_interception() {
        use std::sync::mpsc;
        use std::time::Duration;

        let proxy = PtyProxy::new().expect("PtyProxy::new should succeed");
        let (tx, rx) = mpsc::channel();

        proxy.start(
            move |event| {
                let _ = tx.send(event);
            },
            |_targets| Some(b"queried_data".to_vec()),
        );

        // Send regular text through outer_master (simulating user keystroke)
        let test_input = b"ls -la\n";
        unsafe {
            libc::write(proxy.outer_master_fd(), test_input.as_ptr() as *const libc::c_void, test_input.len());
        }

        // Read from inner_slave (simulating shell process reading stdin)
        let mut read_buf = [0u8; 64];
        let mut read_len = 0;
        for _ in 0..10 {
            let n = unsafe { libc::read(proxy.inner_slave_fd(), read_buf.as_mut_ptr() as *mut libc::c_void, read_buf.len()) };
            if n > 0 {
                read_len = n as usize;
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(&read_buf[..read_len], test_input);

        // Simulate shell writing OSC 52 sequence to inner_slave
        let osc52_seq = b"\x1b]52;c;aGVsbG8=\x07";
        unsafe {
            libc::write(proxy.inner_slave_fd(), osc52_seq.as_ptr() as *const libc::c_void, osc52_seq.len());
        }

        let event = rx.recv_timeout(Duration::from_millis(500)).expect("Should receive OSC 52 event");
        assert_eq!(event.targets, vec![Osc52Target::Clipboard]);
        assert_eq!(event.operation, Osc52Operation::Write(b"hello".to_vec()));

        proxy.shutdown();
        // Drop automatically closes remaining FDs
    }
}
