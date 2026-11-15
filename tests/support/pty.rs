#![allow(dead_code)]
//! Bounded real-terminal driver. Child, reader thread and channel are all reclaimed.
use portable_pty::{CommandBuilder, MasterPty, PtySize};
use std::{
    collections::VecDeque,
    io::{Read, Write},
    num::NonZeroU16,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};
pub struct Pty {
    master: Option<Box<dyn MasterPty + Send>>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Option<Box<dyn Write + Send>>,
    receiver: Option<mpsc::Receiver<Vec<u8>>>,
    reader: Option<std::thread::JoinHandle<()>>,
    parser: vt100::Parser,
    raw: VecDeque<u8>,
    exited: bool,
    original_flags: Option<Vec<u8>>,
}
impl Pty {
    pub fn spawn(mut command: CommandBuilder, cols: u16, rows: u16) -> Self {
        command.cwd(std::env::current_dir().unwrap());
        let pair = portable_pty::native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let original_flags = pair
            .master
            .get_termios()
            .map(|t| t.local_flags.bits().to_be_bytes().to_vec());
        let child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);
        let mut input = pair.master.try_clone_reader().unwrap();
        let writer = pair.master.take_writer().unwrap();
        let (sender, receiver) = mpsc::sync_channel(16);
        let reader = std::thread::spawn(move || {
            let mut bytes = [0; 4096];
            loop {
                match input.read(&mut bytes) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if sender.send(bytes[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Self {
            master: Some(pair.master),
            child,
            writer: Some(writer),
            receiver: Some(receiver),
            reader: Some(reader),
            parser: vt100::Parser::new(
                NonZeroU16::new(rows).unwrap(),
                NonZeroU16::new(cols).unwrap(),
                0,
            ),
            raw: VecDeque::new(),
            exited: false,
            original_flags,
        }
    }
    pub fn send(&mut self, bytes: &[u8]) {
        self.writer.as_mut().unwrap().write_all(bytes).unwrap();
        self.writer.as_mut().unwrap().flush().unwrap();
    }
    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.parser.set_size(
            NonZeroU16::new(rows).unwrap(),
            NonZeroU16::new(cols).unwrap(),
        );
        self.master
            .as_ref()
            .unwrap()
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
    }
    pub fn read(&mut self, timeout: Duration) {
        if let Ok(bytes) = self.receiver.as_ref().unwrap().recv_timeout(timeout) {
            self.parser.process(&bytes);
            self.raw.extend(bytes);
            while self.raw.len() > 1024 * 1024 {
                self.raw.pop_front();
            }
        }
    }
    pub fn screen(&self) -> String {
        self.parser.screen().contents()
    }
    pub fn wait(&mut self, marker: &str, seconds: u64) {
        self.wait_until(Duration::from_secs(seconds), |text| text.contains(marker));
    }
    pub fn wait_until(&mut self, timeout: Duration, predicate: impl Fn(&str) -> bool) {
        let deadline = Instant::now() + timeout;
        while !predicate(&self.screen()) {
            if Instant::now() >= deadline {
                self.save_failure();
                panic!(
                    "terminal predicate deadline; last screen:\n{}",
                    self.screen()
                );
            }
            assert!(!self.exited, "terminal already exited");
            self.read(Duration::from_millis(10));
        }
    }
    pub fn command(&mut self, command: &str, view: &str) {
        self.send(format!("{command}\r").as_bytes());
        self.wait(&format!("> {view} |"), 60);
    }
    pub fn exit(&mut self, seconds: u64) -> u32 {
        let deadline = Instant::now() + Duration::from_secs(seconds);
        loop {
            self.read(Duration::from_millis(10));
            if let Some(status) = self.child.try_wait().unwrap() {
                self.exited = true;
                for _ in 0..8 {
                    self.read(Duration::from_millis(5));
                }
                return status.exit_code();
            }
            if Instant::now() >= deadline {
                self.save_failure();
                panic!("PTY child failed to exit");
            }
        }
    }
    pub fn restored(&self) -> bool {
        let flags = self
            .master
            .as_ref()
            .unwrap()
            .get_termios()
            .map(|t| t.local_flags.bits().to_be_bytes().to_vec());
        flags == self.original_flags && !self.parser.screen().alternate_screen()
    }
    fn save_failure(&self) {
        if let Some(dir) = std::env::var_os("E2E_LOG_FILE")
            .and_then(|p| Path::new(&p).parent().map(Path::to_owned))
        {
            let _ = std::fs::create_dir_all(&dir);
            let text = cnpg_watch::logs::redact(&self.screen(), cnpg_watch::cli::Profile::Secure);
            let _ = std::fs::write(dir.join("terminal-vt100-screen.txt"), text);
            let path = dir.join("terminal-vt100-private.bin");
            let mut options = std::fs::OpenOptions::new();
            options.create(true).write(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            if let Ok(mut file) = options.open(path) {
                let _ = file.write_all(&self.raw.iter().copied().collect::<Vec<_>>());
            }
        }
    }
}
impl Drop for Pty {
    fn drop(&mut self) {
        if !self.exited {
            let _ = self.child.kill();
            let until = Instant::now() + Duration::from_secs(2);
            while Instant::now() < until {
                if self.child.try_wait().ok().flatten().is_some() {
                    break;
                }
                self.read(Duration::from_millis(5));
            }
        }
        self.writer.take();
        self.master.take();
        self.receiver.take();
        if let Some(reader) = self.reader.take() {
            let until = Instant::now() + Duration::from_secs(2);
            while !reader.is_finished() && Instant::now() < until {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(
                reader.is_finished(),
                "PTY reader leaked after child shutdown"
            );
            reader.join().unwrap();
        }
    }
}
