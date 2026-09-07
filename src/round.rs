// Concern: a review that outlives the caller that started it — its detachment, its lock, its output, and waiting one out | Non-concern: what a reviewer is asked | IO: (work) -> outcome

use crate::lock::{self, Held, Landed, Live};
use crate::report;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq)]
pub enum Outcome {
    Clean,
    Blocked,
}

impl Outcome {
    fn named(self) -> &'static str {
        match self {
            Outcome::Clean => "PASSED",
            Outcome::Blocked => "BLOCKED",
        }
    }
}

// Nothing here says what is being reviewed: this side does not know and must not learn.
pub struct Round {
    id: String,
    dir: PathBuf,
    held: Held,
    label: &'static str,
    ceiling: Duration,
}

impl Round {
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn at_gate(&self, gate: &str) {
        if !gate.is_empty() {
            redirect(
                &self
                    .dir
                    .join(format!("{}-{gate}.stdout.log", report::next_log(&self.dir))),
            );
        }
        let _ = self.held.describe(&Landed::Round(Live {
            label: self.label.to_string(),
            round: self.id.clone(),
            started: lock::now(),
            ceiling: self.ceiling.as_secs(),
            pid: std::process::id(),
            gate: gate.to_string(),
        }));
    }
}

pub struct Started {
    pub pid: u32,
    pub at: PathBuf,
}

const STATUS: &str = "status";

fn pipe_path() -> Result<PathBuf, String> {
    crate::git::git_path("agent-verdict.round")
}

fn dir_for(_round: &str) -> Result<PathBuf, String> {
    let dir = report::verdicts_dir()?.join(crate::git::head_sha());
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

// Kept beside the claim, not the commit diary: the diary is dropped the moment HEAD moves.
fn remember(at: &Path) -> Result<(), String> {
    let path = crate::git::git_path("agent-verdict.last")?;
    std::fs::write(&path, at.to_string_lossy().as_bytes())
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

pub fn last_at() -> Option<PathBuf> {
    let text = std::fs::read_to_string(crate::git::git_path("agent-verdict.last").ok()?).ok()?;
    let at = text.trim().to_string();
    (!at.is_empty()).then(|| PathBuf::from(at))
}

pub fn forget_last() {
    if let Ok(path) = crate::git::git_path("agent-verdict.last") {
        let _ = std::fs::remove_file(path);
    }
}

pub fn abandon_logs() {
    if let Ok(path) = crate::git::git_path("agent-verdict.last") {
        let _ = std::fs::remove_file(path);
    }
    if let Ok(dir) = report::verdicts_dir() {
        let _ = std::fs::remove_dir_all(dir.join(crate::git::head_sha()));
    }
}

// Written whatever happens: a directory without one is a round that died before it could speak.
fn conclude(at: &Path, status: &str) {
    let _ = std::fs::write(at.join(STATUS), format!("{status}\n"));
}

fn concluded(at: &Path) -> Option<String> {
    Some(
        std::fs::read_to_string(at.join(STATUS))
            .ok()?
            .trim()
            .to_string(),
    )
}

// A reader finds a writer exactly while a round lives, so the round ending is an EOF, not a recorded fact.
fn open_pipe() -> Result<std::fs::File, String> {
    let path = pipe_path()?;
    let name = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .map_err(|_| "the repo's path cannot be named".to_string())?;
    unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
    let fd = unsafe { libc::open(name.as_ptr(), libc::O_RDWR | libc::O_CLOEXEC) };
    if fd < 0 {
        return Err(format!(
            "cannot open {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    Ok(unsafe { <std::fs::File as std::os::unix::io::FromRawFd>::from_raw_fd(fd) })
}

// Double fork plus setsid detaches the round from the caller's process tree and group.
pub fn spawn<W>(
    held: Held,
    label: &'static str,
    ceiling: Duration,
    work: W,
) -> Result<Started, String>
where
    W: FnOnce(&Round) -> Result<Outcome, String>,
{
    let id = crate::agent::fresh_id();
    let at = dir_for(&id)?;
    let pipe = open_pipe()?;
    let mut handshake = [0; 2];
    if unsafe { libc::pipe(handshake.as_mut_ptr()) } != 0 {
        return Err(format!(
            "cannot open a handshake: {}",
            std::io::Error::last_os_error()
        ));
    }
    // pid 0 says the round has not named itself yet.
    held.describe(&Landed::Round(Live {
        label: label.to_string(),
        round: id.clone(),
        started: lock::now(),
        ceiling: ceiling.as_secs(),
        pid: 0,
        gate: String::new(),
    }))?;
    let (reading, writing) = (handshake[0], handshake[1]);
    match unsafe { libc::fork() } {
        -1 => Err(format!(
            "cannot start a round: {}",
            std::io::Error::last_os_error()
        )),
        0 => {
            unsafe { libc::close(reading) };
            if unsafe { libc::fork() } != 0 {
                // The middle process ends at once, so the round is reparented before its caller can be killed with it.
                unsafe { libc::_exit(0) };
            }
            carry(
                Round {
                    id,
                    dir: at,
                    held,
                    label,
                    ceiling,
                },
                pipe,
                work,
                writing,
            );
            unsafe { libc::_exit(0) };
        }
        middle => {
            unsafe { libc::close(writing) };
            let pid = told_pid(reading);
            let mut reaped = 0;
            unsafe { libc::waitpid(middle, &mut reaped, 0) };
            drop(pipe);
            drop(held);
            match pid {
                Some(pid) => Ok(Started { pid, at }),
                None => Err("the review process did not start and recorded no reason".to_string()),
            }
        }
    }
}

// One line or none: a round dying before it names itself closes the descriptor, ending the read.
fn told_pid(reading: i32) -> Option<u32> {
    let mut said = Vec::new();
    let mut byte = [0u8; 64];
    loop {
        let read = unsafe { libc::read(reading, byte.as_mut_ptr().cast(), byte.len()) };
        if read <= 0 {
            break;
        }
        said.extend_from_slice(&byte[..read as usize]);
    }
    unsafe { libc::close(reading) };
    String::from_utf8_lossy(&said).trim().parse().ok()
}

// Nobody is watching from here: stdout/stderr are a file, and the caller may already be gone.
fn carry<W>(round: Round, pipe: std::fs::File, work: W, writing: i32)
where
    W: FnOnce(&Round) -> Result<Outcome, String>,
{
    unsafe { libc::setsid() };
    redirect(&round.dir().join("round.stdout.log"));
    crate::signals::arm(crate::signals::Posture::Round);
    round.at_gate("");
    let _ = remember(round.dir());
    let told = format!("{}\n", std::process::id());
    unsafe { libc::write(writing, told.as_ptr().cast(), told.len()) };
    unsafe { libc::close(writing) };
    match work(&round) {
        Ok(outcome) => conclude(round.dir(), outcome.named()),
        Err(why) => {
            eprintln!("git-agent-verdict: error: {why}");
            conclude(round.dir(), why.lines().next().unwrap_or("no verdict"));
        }
    }
    drop(pipe);
    drop(round);
}

fn redirect(to: &Path) {
    let quiet = std::fs::File::open("/dev/null");
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(to);
    if let (Ok(quiet), Ok(log)) = (quiet, log) {
        unsafe {
            libc::dup2(quiet.as_raw_fd(), 0);
            libc::dup2(log.as_raw_fd(), 1);
            libc::dup2(log.as_raw_fd(), 2);
        }
    }
}

// A FIFO opens at once with no writer; the read after tells EOF apart from "still running".
fn hung_up() -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = pipe_path()?;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(file) => file,
        // No FIFO ever opened here is an answer, not a fault.
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(why) => return Err(format!("cannot open {}: {why}", path.display())),
    };
    let fd = file.as_raw_fd();
    let mut byte = [0u8; 1];
    loop {
        let read = unsafe { libc::read(fd, byte.as_mut_ptr().cast(), 1) };
        if read == 0 {
            return Ok(());
        }
        if read > 0 {
            continue;
        }
        match std::io::Error::last_os_error().kind() {
            std::io::ErrorKind::Interrupted => continue,
            std::io::ErrorKind::WouldBlock => break,
            _ => {
                return Err(format!(
                    "cannot read {}: {}",
                    path.display(),
                    std::io::Error::last_os_error()
                ))
            }
        }
    }
    // POSIX guarantees this blocking read returns 0 once the last writer closes: no polling.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags & !libc::O_NONBLOCK) } < 0 {
        return Err(format!(
            "cannot wait on {}: {}",
            path.display(),
            std::io::Error::last_os_error()
        ));
    }
    loop {
        let read = unsafe { libc::read(fd, byte.as_mut_ptr().cast(), 1) };
        if read == 0 {
            return Ok(());
        }
        if read < 0 && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted {
            return Err(format!(
                "cannot wait on {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
    }
}

pub fn wait() -> Result<bool, String> {
    hung_up()?;
    let Some(at) = last_at() else {
        return Err(format!(
            "no review has run for this commit\n  git agent-verdict attest --repo {} --intent \"<intent: one line>\"",
            report::repo_root()
        ));
    };
    let said = concluded(&at);
    report::awaited(&at, said.as_deref());
    match said.as_deref() {
        Some("PASSED") => Ok(true),
        Some("BLOCKED") => Ok(false),
        Some(why) => Err(why.to_string()),
        None => Err("the review process exited without recording a verdict".to_string()),
    }
}

// Verdicts already earned are kept: throwing those away to stop one gate is paying twice.
pub fn abort(abandon: impl FnOnce() -> Vec<(String, report::Standing)>) -> Result<bool, String> {
    if lock::take().is_ok() {
        report::nothing_to_abort();
        return Ok(true);
    }
    let live = match lock::describing() {
        Some(Landed::Round(live)) => live,
        Some(Landed::Landing) => {
            return Err(format!(
                "a commit is being written in {}; there is no review to abort",
                report::repo_root()
            ))
        }
        None => return Err("the lock is being written; retry".to_string()),
    };
    if live.pid == 0 {
        return Err(format!(
            "a review is starting in {}; retry",
            report::repo_root()
        ));
    }
    // The round process ends its own reviewer, which is the only process that knows what it spawned.
    unsafe { libc::kill(live.pid as i32, libc::SIGTERM) };
    hung_up()?;
    let standings = abandon();
    let at = last_at();
    if let Some(at) = &at {
        if concluded(at).is_none() {
            conclude(at, "ABORTED");
        }
    }
    report::aborted(live.pid, at.as_deref(), &standings);
    Ok(true)
}
