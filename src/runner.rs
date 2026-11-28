//! Runs an extension's executable and streams its output.

use std::{
    io::{self, BufRead, BufReader, Read},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
};

/// Maximum bytes buffered per output line before excess bytes are discarded until newline.
const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, PartialEq)]
pub enum JobEvent {
    Line(String),
    /// Exit code, or why the process could not be waited for.
    Exit(Result<i32, String>),
}

struct ProcessState {
    child: Option<Child>,
    #[cfg(windows)]
    job: Option<std::os::windows::io::OwnedHandle>,
    #[cfg(unix)]
    pgid: Option<u32>,
}

/// Lets the UI stop a running job and its entire descendant process tree.
#[derive(Clone)]
pub struct JobHandle {
    state: Arc<Mutex<ProcessState>>,
    cancelled: Arc<AtomicBool>,
}

impl JobHandle {
    /// A handle with no process behind it, for tests, headless scenes, or pending spawns.
    pub fn detached() -> JobHandle {
        JobHandle {
            state: Arc::new(Mutex::new(ProcessState {
                child: None,
                #[cfg(windows)]
                job: None,
                #[cfg(unix)]
                pgid: None,
            })),
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        if let Ok(mut state) = self.state.lock() {
            #[cfg(windows)]
            if let Some(job) = state.job.take() {
                terminate_job(&job);
            }
            #[cfg(unix)]
            if let Some(pgid) = state.pgid.take() {
                let _ = Command::new("kill")
                    .args(["-KILL", &format!("-{pgid}")])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            if let Some(child) = state.child.as_mut() {
                let _ = child.kill();
            }
        }
    }
}

pub struct Spec {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub cwd: PathBuf,
}

/// Starts the process; every stdout/stderr line and finally the exit arrive through `send`.
pub fn spawn(spec: Spec, send: Arc<dyn Fn(JobEvent) + Send + Sync>) -> Result<JobHandle, String> {
    let handle = JobHandle::detached();
    spawn_into(handle.clone(), spec, send)?;
    Ok(handle)
}

/// Starts the process attached to an existing `JobHandle` (supports cancellation before spawn).
pub fn spawn_into(handle: JobHandle, spec: Spec, send: Arc<dyn Fn(JobEvent) + Send + Sync>) -> Result<(), String> {
    if handle.is_cancelled() {
        send(JobEvent::Exit(Ok(-1)));
        return Ok(());
    }
    std::fs::create_dir_all(&spec.cwd).map_err(|e| e.to_string())?;
    let mut command = Command::new(&spec.program);
    command
        .args(&spec.args)
        .envs(spec.env.iter().map(|(k, v)| (k, v)))
        // Ask for @flacora event lines (track, progress, saved files).
        .env("FLACORA_EVENTS", "1")
        .current_dir(&spec.cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    no_console_window(&mut command);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command.spawn().map_err(|e| format!("Could not start {}: {e}", name(&spec.program)))?;
    #[cfg(windows)]
    let job = assign_job_object(&child);
    #[cfg(unix)]
    let pgid = Some(child.id());
    let cancelled = handle.cancelled.clone();
    let readers: Vec<_> = [
        child.stdout.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
        child.stderr.take().map(|s| Box::new(s) as Box<dyn Read + Send>),
    ]
    .into_iter()
    .flatten()
    .map(|stream| {
        let send = send.clone();
        let cancelled = cancelled.clone();
        thread::spawn(move || {
            let mut reader = BufReader::new(stream);
            let mut line = Vec::new();
            while read_bounded_line(&mut reader, &mut line, MAX_LINE_BYTES).is_ok_and(|n| n > 0) {
                if !cancelled.load(Ordering::Relaxed) {
                    let text = String::from_utf8_lossy(&line).trim_end().to_owned();
                    if !text.is_empty() {
                        send(JobEvent::Line(text));
                    }
                }
                line.clear();
            }
        })
    })
    .collect();
    if let Ok(mut state) = handle.state.lock() {
        state.child = Some(child);
        #[cfg(windows)]
        {
            state.job = job;
        }
        #[cfg(unix)]
        {
            state.pgid = pgid;
        }
    }
    if handle.is_cancelled() {
        handle.cancel();
    }
    let waiter = handle;
    thread::spawn(move || {
        // Poll try_wait unlocked so cancel() can acquire the mutex immediately; once the direct
        // child exits, terminate any orphaned descendants so inherited pipes close promptly.
        let mut delay_ms = 2u64;
        let exit = loop {
            let step = {
                let Ok(mut state) = waiter.state.lock() else {
                    break Err("The process state lock was poisoned".into());
                };
                match state.child.as_mut() {
                    Some(child) => match child.try_wait() {
                        Ok(Some(status)) => {
                            state.child = None;
                            Some(Ok(status.code().unwrap_or(-1)))
                        }
                        Ok(None) => None,
                        Err(e) => {
                            state.child = None;
                            Some(Err(e.to_string()))
                        }
                    },
                    None => Some(Err("The process handle was lost".into())),
                }
            };
            if let Some(result) = step {
                break result;
            }
            thread::sleep(std::time::Duration::from_millis(delay_ms));
            delay_ms = (delay_ms * 2).min(20);
        };
        if let Ok(mut state) = waiter.state.lock() {
            #[cfg(windows)]
            if let Some(job) = state.job.take() {
                terminate_job(&job);
            }
            #[cfg(unix)]
            if let Some(pgid) = state.pgid.take() {
                let _ = Command::new("kill")
                    .args(["-KILL", &format!("-{pgid}")])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
        }
        for reader in readers {
            let _ = reader.join();
        }
        send(JobEvent::Exit(exit));
    });
    Ok(())
}

/// Reads until `\n` or `\r`, storing at most `limit` bytes in `out` and discarding any overflow.
fn read_bounded_line(reader: &mut impl BufRead, out: &mut Vec<u8>, limit: usize) -> io::Result<usize> {
    let mut total = 0usize;
    loop {
        let buf = reader.fill_buf()?;
        if buf.is_empty() {
            return Ok(total);
        }
        let newline = buf.iter().position(|&b| b == b'\n' || b == b'\r');
        let consume = newline.map_or(buf.len(), |i| i + 1);
        let payload = newline.map_or(buf, |i| &buf[..i]);
        let room = limit.saturating_sub(out.len());
        if room > 0 {
            let take = payload.len().min(room);
            out.extend_from_slice(&payload[..take]);
        }
        reader.consume(consume);
        total += consume;
        if newline.is_some() {
            return Ok(total);
        }
    }
}

#[cfg(windows)]
fn assign_job_object(child: &Child) -> Option<std::os::windows::io::OwnedHandle> {
    use std::{
        mem,
        os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
        ptr,
    };
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION, JobObjectExtendedLimitInformation, SetInformationJobObject,
    };
    unsafe {
        let handle = CreateJobObjectW(ptr::null(), ptr::null());
        if handle.is_null() {
            return None;
        }
        let job = OwnedHandle::from_raw_handle(handle);
        let mut limits: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = mem::zeroed();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if SetInformationJobObject(
            handle,
            JobObjectExtendedLimitInformation,
            (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
            mem::size_of_val(&limits) as u32,
        ) == 0
            || AssignProcessToJobObject(handle, child.as_raw_handle()) == 0
        {
            return None;
        }
        Some(job)
    }
}

#[cfg(windows)]
fn terminate_job(job: &std::os::windows::io::OwnedHandle) {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::JobObjects::TerminateJobObject;
    unsafe {
        let _ = TerminateJobObject(job.as_raw_handle(), 1);
    }
}

fn name(program: &Path) -> String {
    program.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Keeps console programs from flashing a window when the GUI starts them.
#[cfg(windows)]
pub fn no_console_window(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
pub fn no_console_window(_: &mut Command) {}

const ALLOWED_OPEN_EXTENSIONS: [&str; 22] = [
    "flac", "m4a", "mp3", "ogg", "wav", "opus", "aac", "alac", "aiff",
    "lrc", "ttml", "srt", "vtt", "txt", "json",
    "jpg", "jpeg", "png", "webp",
    "cue", "log", "pdf",
];

pub fn is_safe_open_file(path: &Path) -> bool {
    path.extension().is_some_and(|ext| {
        ALLOWED_OPEN_EXTENSIONS
            .iter()
            .any(|good| ext.eq_ignore_ascii_case(good))
    })
}

fn spawn_detached(mut command: Command) -> io::Result<()> {
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    let mut child = command.spawn()?;
    #[cfg(unix)]
    thread::spawn(move || {
        let _ = child.wait();
    });
    #[cfg(not(unix))]
    let _ = &mut child;
    Ok(())
}

/// Opens a local file or folder with the system's default app.
pub fn open_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File or folder not found: {}", path.display()));
    }
    let canonical = path.canonicalize().map_err(|e| format!("Could not resolve path: {e}"))?;
    if canonical.is_file() && !is_safe_open_file(&canonical) {
        return Err("Refusing to open an unsupported file type.".into());
    }
    #[cfg(target_os = "macos")]
    if canonical.is_dir()
        && canonical.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("app"))
    {
        return Err("Refusing to open an application bundle.".into());
    }
    let target = strip_win_verbatim(&canonical);
    let program = if cfg!(windows) { "explorer" } else if cfg!(target_os = "macos") { "open" } else { "xdg-open" };
    let mut command = Command::new(program);
    command.arg(&target);
    spawn_detached(command).map_err(|e| format!("Could not open {}: {e}", target.display()))
}

/// Shows a local file selected in the system file manager.
pub fn reveal(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }
    let canonical = path.canonicalize().map_err(|e| format!("Could not resolve path: {e}"))?;
    let target = strip_win_verbatim(&canonical);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let display = target.to_string_lossy();
        if display.contains('"') || display.chars().any(char::is_control) {
            return Err("Could not reveal path containing unsupported characters.".into());
        }
        let cleaned = display.trim_end_matches('\\');
        let mut command = Command::new("explorer");
        command.raw_arg(format!("/select,\"{cleaned}\""));
        spawn_detached(command).map_err(|e| format!("Could not reveal {}: {e}", target.display()))
    }
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg("-R").arg(&target);
        spawn_detached(command).map_err(|e| format!("Could not reveal {}: {e}", target.display()))
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        let parent = target.parent().ok_or_else(|| "File has no parent folder.".to_string())?;
        let mut command = Command::new("xdg-open");
        command.arg(parent);
        spawn_detached(command).map_err(|e| format!("Could not reveal {}: {e}", target.display()))
    }
}

/// Strips Windows `\\?\` verbatim prefix before handing a canonical path to Explorer.
pub(crate) fn strip_win_verbatim(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        let text = path.to_string_lossy();
        if let Some(stripped) = text.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{stripped}"));
        }
        if let Some(stripped) = text.strip_prefix(r"\\?\") {
            return PathBuf::from(stripped);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{sync::mpsc::channel, time::Duration};

    #[test]
    fn bounded_line_reader_caps_overflow_and_splits_carriage_returns() {
        let mut cursor = io::Cursor::new(b"0123456789abcdef\nnext\rthird\r\n");
        let mut line = Vec::new();
        assert!(read_bounded_line(&mut cursor, &mut line, 8).unwrap() > 0);
        assert_eq!(line, b"01234567");
        line.clear();
        assert!(read_bounded_line(&mut cursor, &mut line, 8).unwrap() > 0);
        assert_eq!(line, b"next");
        line.clear();
        assert!(read_bounded_line(&mut cursor, &mut line, 8).unwrap() > 0);
        assert_eq!(line, b"third");
        line.clear();
        assert!(read_bounded_line(&mut cursor, &mut line, 8).unwrap() > 0);
        assert!(line.is_empty());
        assert_eq!(read_bounded_line(&mut cursor, &mut line, 8).unwrap(), 0);
    }

    #[test]
    fn spawn_streams_output_and_cancel_terminates_process_tree() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = channel();
        let spec = if cfg!(windows) {
            Spec {
                program: PathBuf::from("cmd"),
                args: vec!["/C".into(), "echo hello".into()],
                env: Vec::new(),
                cwd: dir.path().to_path_buf(),
            }
        } else {
            Spec {
                program: PathBuf::from("sh"),
                args: vec!["-c".into(), "echo hello".into()],
                env: Vec::new(),
                cwd: dir.path().to_path_buf(),
            }
        };
        let _handle = spawn(spec, Arc::new(move |e| { let _ = tx.send(e); })).unwrap();
        let mut saw_hello = false;
        let mut exited_ok = false;
        while let Ok(event) = rx.recv_timeout(Duration::from_secs(5)) {
            match event {
                JobEvent::Line(l) if l == "hello" => saw_hello = true,
                JobEvent::Exit(Ok(0)) => {
                    exited_ok = true;
                    break;
                }
                _ => {}
            }
        }
        assert!(saw_hello && exited_ok);

        // Cancel a long-running process with a child process and verify prompt exit.
        let (tx, rx) = channel();
        let spec = if cfg!(windows) {
            Spec {
                program: PathBuf::from("cmd"),
                args: vec!["/C".into(), "ping -n 30 127.0.0.1 >nul".into()],
                env: Vec::new(),
                cwd: dir.path().to_path_buf(),
            }
        } else {
            Spec {
                program: PathBuf::from("sh"),
                args: vec!["-c".into(), "sleep 30".into()],
                env: Vec::new(),
                cwd: dir.path().to_path_buf(),
            }
        };
        let handle = spawn(spec, Arc::new(move |e| { let _ = tx.send(e); })).unwrap();
        thread::sleep(Duration::from_millis(100));
        handle.cancel();
        let mut exited = false;
        while let Ok(event) = rx.recv_timeout(Duration::from_secs(5)) {
            if matches!(event, JobEvent::Exit(_)) {
                exited = true;
                break;
            }
        }
        assert!(exited, "cancelled process tree must exit promptly");
    }

    #[test]
    fn open_file_allowlist_permits_media_and_blocks_scripts() {
        for good in ["song.flac", "lyrics.LRC", "meta.json", "cover.webp", "track.m4a", "notes.txt"] {
            assert!(is_safe_open_file(Path::new(good)), "{good} should be allowed");
        }
        for bad in ["bad.exe", "bad.dll", "bad.hta", "bad.py", "bad.jar", "bad.app", "noext"] {
            assert!(!is_safe_open_file(Path::new(bad)), "{bad} should be rejected");
        }
    }
}
