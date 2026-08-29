#![cfg(unix)]

use std::ffi::{CStr, OsStr};
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nothing_tui::keyscript::parse_keys;
use nothing_tui::tutorial::STEPS;

const ROWS: usize = 40;
const COLS: usize = 120;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nothing")
}

fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("nothing-cli-tutorial-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn keys_through(upto: usize) -> String {
    STEPS[..upto]
        .iter()
        .flat_map(|step| step.keys.split_whitespace())
        .collect::<Vec<_>>()
        .join("\n")
}

fn plain(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for c in chars.by_ref() {
                if c.is_ascii_alphabetic() || c == '~' {
                    break;
                }
            }
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

struct Screen {
    cells: Vec<Vec<char>>,
    row: usize,
    col: usize,
    pending: String,
}

impl Screen {
    fn new() -> Screen {
        Screen {
            cells: vec![vec![' '; COLS]; ROWS],
            row: 0,
            col: 0,
            pending: String::new(),
        }
    }

    fn feed(&mut self, text: &str) {
        let buffered = std::mem::take(&mut self.pending) + text;
        let mut rest = buffered.as_str();
        loop {
            let Some(at) = rest.find('\u{1b}') else {
                self.write(rest);
                return;
            };
            self.write(&rest[..at]);
            let tail = &rest[at + 1..];
            let end = tail
                .char_indices()
                .find(|(_, c)| c.is_ascii_alphabetic() || *c == '~')
                .map(|(i, c)| i + c.len_utf8());
            match end {
                None => {
                    self.pending = rest[at..].to_string();
                    return;
                }
                Some(end) => {
                    self.control(&tail[..end]);
                    rest = &tail[end..];
                }
            }
        }
    }

    fn write(&mut self, text: &str) {
        for c in text.chars() {
            match c {
                '\r' => self.col = 0,
                '\n' => {
                    self.col = 0;
                    self.row = (self.row + 1).min(ROWS - 1);
                }
                c if (c as u32) >= 0x20 => {
                    if self.row < ROWS && self.col < COLS {
                        self.cells[self.row][self.col] = c;
                    }
                    self.col += 1;
                }
                _ => {}
            }
        }
    }

    fn control(&mut self, seq: &str) {
        let Some(rest) = seq.strip_prefix('[') else {
            return;
        };
        let Some(final_byte) = rest.chars().last() else {
            return;
        };
        let params = &rest[..rest.len() - final_byte.len_utf8()];
        if params.starts_with('?') {
            if params.contains("1049") {
                self.clear();
            }
            return;
        }
        match final_byte {
            'H' | 'f' => {
                let mut parts = params.split(';');
                let row = parts.next().unwrap_or("").parse::<usize>().unwrap_or(1);
                let col = parts.next().unwrap_or("").parse::<usize>().unwrap_or(1);
                self.row = row.saturating_sub(1).min(ROWS - 1);
                self.col = col.saturating_sub(1).min(COLS - 1);
            }
            'J' => self.clear(),
            'K' => {
                if self.row < ROWS {
                    for cell in self.cells[self.row][self.col.min(COLS)..].iter_mut() {
                        *cell = ' ';
                    }
                }
            }
            _ => {}
        }
    }

    fn clear(&mut self) {
        for row in self.cells.iter_mut() {
            row.fill(' ');
        }
        self.row = 0;
        self.col = 0;
    }

    fn text(&self) -> String {
        self.cells
            .iter()
            .map(|row| row.iter().collect::<String>().trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn open_pty() -> (OwnedFd, OwnedFd) {
    unsafe {
        let master = libc::posix_openpt(libc::O_RDWR | libc::O_NOCTTY);
        assert!(master >= 0, "posix_openpt failed");
        assert_eq!(libc::grantpt(master), 0, "grantpt failed");
        assert_eq!(libc::unlockpt(master), 0, "unlockpt failed");
        let name = libc::ptsname(master);
        assert!(!name.is_null(), "ptsname failed");
        let path = CStr::from_ptr(name).to_owned();
        let slave = libc::open(path.as_ptr(), libc::O_RDWR | libc::O_NOCTTY);
        assert!(slave >= 0, "opening the pty slave failed");

        let mut size: libc::winsize = std::mem::zeroed();
        size.ws_row = ROWS as u16;
        size.ws_col = COLS as u16;
        libc::ioctl(slave, libc::TIOCSWINSZ, &size);

        let flags = libc::fcntl(master, libc::F_GETFL);
        libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);

        (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave))
    }
}

fn read_available(fd: RawFd) -> Option<Vec<u8>> {
    let mut buf = [0u8; 4096];
    let got = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
    if got > 0 {
        Some(buf[..got as usize].to_vec())
    } else {
        None
    }
}

fn write_all(fd: RawFd, bytes: &[u8]) {
    let mut sent = 0usize;
    while sent < bytes.len() {
        let put = unsafe { libc::write(fd, bytes[sent..].as_ptr().cast(), bytes.len() - sent) };
        assert!(put > 0, "the pty refused a keystroke");
        sent += put as usize;
    }
}

struct Session {
    master: OwnedFd,
    child: std::process::Child,
    screen: Screen,
    bytes: Vec<u8>,
    raw: String,
}

impl Session {
    fn open(path: &std::path::Path) -> Session {
        let (master, slave) = open_pty();
        let stdin = slave.try_clone().expect("the pty slave clones");
        let stdout = slave.try_clone().expect("the pty slave clones");
        let stderr = slave.try_clone().expect("the pty slave clones");

        let mut command = Command::new(bin());
        command
            .arg("tutorial")
            .arg(path)
            .env("TERM", "xterm")
            .stdin(Stdio::from(stdin))
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                if libc::ioctl(0, libc::TIOCSCTTY as libc::c_ulong, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().expect("the tutorial starts under a pty");
        drop(slave);

        Session {
            master,
            child,
            screen: Screen::new(),
            bytes: Vec::new(),
            raw: String::new(),
        }
    }

    fn drain(&mut self, window: Duration) {
        let deadline = Instant::now() + window;
        while Instant::now() < deadline {
            match read_available(self.master.as_raw_fd()) {
                Some(chunk) => {
                    self.bytes.extend_from_slice(&chunk);
                    let upto = match std::str::from_utf8(&self.bytes) {
                        Ok(_) => self.bytes.len(),
                        Err(err) => err.valid_up_to() + err.error_len().unwrap_or(0),
                    };
                    let text = String::from_utf8_lossy(&self.bytes[..upto]).to_string();
                    self.bytes.drain(..upto);
                    self.screen.feed(&text);
                    self.raw.push_str(&text);
                }
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
    }

    fn wait_for_first_frame(&mut self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while Instant::now() < deadline {
            self.drain(Duration::from_millis(100));
            if self.screen.text().contains("C-q quit") {
                return;
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
        panic!(
            "the tutorial never drew its key line; it wrote: {:?}",
            self.raw
        );
    }

    fn type_script(&mut self, script: &str) {
        for key in parse_keys(script).expect("the script parses") {
            write_all(self.master.as_raw_fd(), &encode(&key));
            self.drain(Duration::from_millis(30));
        }
        self.drain(Duration::from_millis(200));
    }

    fn screen(&self) -> String {
        self.screen.text()
    }

    fn quit(mut self) -> String {
        write_all(self.master.as_raw_fd(), &[0x11]);
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut tail = String::new();
        loop {
            let before = self.raw.len();
            self.drain(Duration::from_millis(100));
            tail.push_str(&self.raw[before..]);
            match self.child.try_wait().expect("the tutorial is waitable") {
                Some(status) => {
                    assert!(
                        status.success(),
                        "the tutorial exited with {status}: {tail:?}"
                    );
                    let before = self.raw.len();
                    self.drain(Duration::from_millis(200));
                    tail.push_str(&self.raw[before..]);
                    return plain(&tail);
                }
                None if Instant::now() < deadline => continue,
                None => {
                    let _ = self.child.kill();
                    let _ = self.child.wait();
                    panic!("the tutorial did not quit on ctrl-q; it wrote: {tail:?}");
                }
            }
        }
    }
}

fn encode(key: &KeyEvent) -> Vec<u8> {
    match key.code {
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => {
            vec![(c as u8) & 0x1f]
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Tab => vec![b'\t'],
        KeyCode::BackTab => b"\x1b[Z".to_vec(),
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Delete => b"\x1b[3~".to_vec(),
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        other => panic!("no terminal encoding for {other:?}"),
    }
}

fn nothing(args: &[&OsStr], stdin: &str) -> (i32, String, String) {
    let mut child = Command::new(bin())
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the nothing binary runs");
    child
        .stdin
        .take()
        .expect("stdin is piped")
        .write_all(stdin.as_bytes())
        .expect("the program reads its input");
    let out = child.wait_with_output().expect("the run finishes");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
    )
}

#[test]
fn the_tutorial_pane_names_the_first_step_and_advances_on_the_keys_it_asks_for() {
    let path = scratch_dir().join("advance.n");
    std::fs::remove_file(&path).ok();

    let mut session = Session::open(&path);
    session.wait_for_first_frame();
    assert!(
        plain(&session.raw).contains("tutorial: editing"),
        "the path is printed before the editor opens: {:?}",
        plain(&session.raw)
    );

    let opening = session.screen();
    assert!(opening.contains("tutorial 1/9"), "{opening}");
    assert!(opening.contains("Step 1 of 9"), "{opening}");
    assert!(opening.contains("▸ Write a function"), "{opening}");
    assert!(opening.contains("· Repair it and finish"), "{opening}");

    session.type_script(&keys_through(1));
    let after_one = session.screen();
    assert!(
        after_one.contains("Step 2 of 9"),
        "one keystroke satisfied the structural check and the pane moved on:\n{after_one}"
    );
    assert!(after_one.contains("✓ Write a function"), "{after_one}");
    assert!(after_one.contains("▸ Name the parameter"), "{after_one}");
    assert!(after_one.contains("λ»x0«:?. ⦇⦈"), "{after_one}");

    session.type_script(&STEPS[1].keys.replace(' ', "\n"));
    let after_two = session.screen();
    assert!(after_two.contains("Step 3 of 9"), "{after_two}");
    assert!(after_two.contains("✓ Name the parameter"), "{after_two}");

    let tail = session.quit();
    assert!(tail.contains("tutorial: saved"), "{tail:?}");
    assert!(
        tail.contains("stopped on step 3 of 9"),
        "an unfinished tutorial says where it stopped rather than running it: {tail:?}"
    );
    assert!(path.exists(), "quitting wrote the file");
}

#[test]
fn a_half_finished_tutorial_resumes_where_it_stopped_with_no_progress_file() {
    let path = scratch_dir().join("resume.n");
    std::fs::remove_file(&path).ok();

    let mut first = Session::open(&path);
    first.wait_for_first_frame();
    first.type_script(&keys_through(5));
    let tail = first.quit();
    assert!(tail.contains("stopped on step 6 of 9"), "{tail:?}");

    let beside: Vec<String> = std::fs::read_dir(scratch_dir())
        .expect("the scratch directory is readable")
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.file_name().to_string_lossy().to_string())
        .filter(|name| name.starts_with("resume"))
        .collect();
    assert_eq!(
        beside,
        vec!["resume.n".to_string()],
        "the tutorial keeps no progress file of its own"
    );

    let mut second = Session::open(&path);
    second.wait_for_first_frame();
    let opening = second.screen();
    assert!(
        opening.contains("Step 6 of 9"),
        "reopening the same file resumed from the document itself:\n{opening}"
    );
    assert!(opening.contains("▸ Start a second definition"), "{opening}");
    assert!(
        opening.contains("λwho:Str. \"hello, \" ++ who"),
        "{opening}"
    );
    second.quit();
}

#[test]
fn the_whole_tutorial_typed_into_the_real_editor_runs_when_it_is_finished() {
    let path = scratch_dir().join("full.n");
    std::fs::remove_file(&path).ok();

    let mut session = Session::open(&path);
    session.wait_for_first_frame();

    session.type_script(&keys_through(STEPS.len() - 1));
    let quarantined = session.screen();
    assert!(
        quarantined.contains("print ⦇»greet«⦈"),
        "the quarantine is on screen:\n{quarantined}"
    );
    assert!(
        quarantined.contains("1 quarantined"),
        "and the status line counts it:\n{quarantined}"
    );
    assert!(quarantined.contains("Step 9 of 9"), "{quarantined}");
    assert!(
        quarantined.contains("▸ Repair it and finish"),
        "{quarantined}"
    );

    session.type_script(&STEPS[STEPS.len() - 1].keys.replace(' ', "\n"));
    let done = session.screen();
    assert!(done.contains("tutorial · done"), "{done}");
    assert!(done.contains("All 9 steps are done."), "{done}");
    assert!(done.contains("nothing run"), "{done}");
    assert!(done.contains("✓ Repair it and finish"), "{done}");
    assert!(
        done.contains("print »(greet \"world\")«"),
        "the program it built is on screen:\n{done}"
    );
    assert!(
        !done.contains("quarantined"),
        "and nothing is left quarantined:\n{done}"
    );

    let tail = session.quit();
    assert!(tail.contains("tutorial: saved"), "{tail:?}");
    assert!(
        tail.contains("hello, world"),
        "quitting a finished tutorial performs the program it built: {tail:?}"
    );

    let (code, stdout, stderr) = nothing(&[OsStr::new("run"), path.as_os_str()], "");
    assert_eq!(code, 0, "run failed: {stderr}");
    assert_eq!(stdout, "hello, world\n");

    let (code, stdout, stderr) = nothing(&[OsStr::new("check"), path.as_os_str()], "");
    assert_eq!(code, 0, "check failed: {stderr}");
    assert!(stdout.contains("well-typed: true"), "{stdout}");
}

#[test]
fn tutorial_help_names_the_default_file() {
    let (code, stdout, _) = nothing(&[OsStr::new("tutorial"), OsStr::new("--help")], "");
    assert_eq!(code, 0);
    assert!(stdout.contains("tutorial.n"), "{stdout}");
    assert!(stdout.contains("nothing tutorial [<file>]"), "{stdout}");
}
