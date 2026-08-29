#![cfg(unix)]

use std::ffi::{CStr, OsStr};
use std::io::Write;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use nothing_tui::AppState;
use nothing_tui::keyscript::{parse_keys, replay_keys};

const HELLO_SCRIPT: &str = "\
$
\"
h
e
l
l
o
,
space
w
o
r
l
d
\"
";

const GREETING_SCRIPT: &str = "\
>
l
i
n
e
=
r
e
a
d
l
i
n
e
tab
$
\"
h
e
l
l
o
,
space
\"
&
l
i
n
e
";

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_nothing")
}

fn scratch_dir() -> std::path::PathBuf {
    let dir = std::env::temp_dir().join("nothing-cli-authoring-tests");
    std::fs::create_dir_all(&dir).unwrap();
    dir
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
        size.ws_row = 40;
        size.ws_col = 120;
        libc::ioctl(slave, libc::TIOCSWINSZ, &size);

        let flags = libc::fcntl(master, libc::F_GETFL);
        libc::fcntl(master, libc::F_SETFL, flags | libc::O_NONBLOCK);

        (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave))
    }
}

fn read_available(fd: RawFd) -> Option<String> {
    let mut buf = [0u8; 4096];
    let got = unsafe { libc::read(fd, buf.as_mut_ptr().cast(), buf.len()) };
    if got > 0 {
        Some(String::from_utf8_lossy(&buf[..got as usize]).to_string())
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

struct Editor {
    master: OwnedFd,
    child: std::process::Child,
}

impl Editor {
    fn open(path: &std::path::Path) -> Editor {
        let (master, slave) = open_pty();
        let stdin = slave.try_clone().expect("the pty slave clones");
        let stdout = slave.try_clone().expect("the pty slave clones");
        let stderr = slave.try_clone().expect("the pty slave clones");

        let mut command = Command::new(bin());
        command
            .arg("edit")
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
        let child = command.spawn().expect("the editor starts under a pty");
        drop(slave);

        Editor { master, child }
    }

    fn drain(&mut self, window: Duration) -> String {
        let deadline = Instant::now() + window;
        let mut seen = String::new();
        while Instant::now() < deadline {
            match read_available(self.master.as_raw_fd()) {
                Some(text) => seen.push_str(&text),
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
        seen
    }

    fn wait_for_first_frame(&mut self) -> String {
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut seen = String::new();
        while Instant::now() < deadline {
            seen.push_str(&self.drain(Duration::from_millis(100)));
            if seen.contains("undo") {
                return seen;
            }
        }
        panic!("the editor never drew its key line; it wrote: {seen:?}");
    }

    fn type_script(&mut self, script: &str) -> String {
        let mut screen = String::new();
        for key in parse_keys(script).expect("the script parses") {
            write_all(self.master.as_raw_fd(), &encode(&key));
            screen.push_str(&self.drain(Duration::from_millis(30)));
        }
        screen
    }

    fn quit(mut self) {
        write_all(self.master.as_raw_fd(), &[0x11]);
        let deadline = Instant::now() + Duration::from_secs(30);
        let mut tail = String::new();
        loop {
            tail.push_str(&self.drain(Duration::from_millis(100)));
            match self.child.try_wait().expect("the editor is waitable") {
                Some(status) => {
                    assert!(status.success(), "the editor exited with {status}");
                    return;
                }
                None if Instant::now() < deadline => continue,
                None => {
                    let _ = self.child.kill();
                    panic!("the editor did not quit on ctrl-q; it wrote: {tail:?}");
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
fn the_keystrokes_for_hello_world_build_a_command_in_the_editor() {
    let state = replay_keys(HELLO_SCRIPT, AppState::empty()).expect("the script replays");
    assert_eq!(state.text(), "print \"hello, world\"");
}

#[test]
fn the_keystrokes_for_a_greeting_build_a_command_that_reads_a_line() {
    let state = replay_keys(GREETING_SCRIPT, AppState::empty()).expect("the script replays");
    assert_eq!(
        state.text(),
        "bind line <- readline in print (\"hello, \" ++ line)"
    );
}

#[test]
fn a_hello_world_typed_into_the_real_editor_prints_when_it_is_run() {
    let path = scratch_dir().join("hello.n");
    std::fs::remove_file(&path).ok();

    let mut editor = Editor::open(&path);
    editor.wait_for_first_frame();
    let mut screen = editor.type_script(HELLO_SCRIPT);
    screen.push_str(&editor.drain(Duration::from_millis(300)));
    editor.quit();

    assert!(
        screen.contains("print"),
        "the editor never showed the program it was being given: {screen:?}"
    );
    assert!(path.exists(), "quitting the editor wrote the file");

    let (code, stdout, stderr) = nothing(&[OsStr::new("run"), path.as_os_str()], "");
    assert_eq!(code, 0, "run failed: {stderr}");
    assert_eq!(
        stdout, "hello, world\n",
        "the run printed exactly the text that was typed into the editor"
    );
}

#[test]
fn a_greeting_typed_into_the_real_editor_reads_a_line_when_it_is_run() {
    let path = scratch_dir().join("greeting.n");
    std::fs::remove_file(&path).ok();

    let mut editor = Editor::open(&path);
    editor.wait_for_first_frame();
    editor.type_script(GREETING_SCRIPT);
    editor.drain(Duration::from_millis(300));
    editor.quit();

    let (code, stdout, stderr) = nothing(&[OsStr::new("run"), path.as_os_str()], "Ada Lovelace\n");
    assert_eq!(code, 0, "run failed: {stderr}");
    assert_eq!(stdout, "hello, Ada Lovelace\n");
}
