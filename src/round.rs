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

// What the work is handed: where to write, and a way to say which gate it has reached. Nothing about what it is reviewing, because this side does not know and must not learn.
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

    // The description moves as the round does, so anything that finds the claim taken can say what is being reviewed rather than only that something is. The narration moves with it: what the run says while reviewing a gate belongs beside that gate's findings, not in one file spanning all of them.
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

// One directory per commit being written, named before the first reviewer runs, so a caller can be told where to look before there is anything to look at and every gate since the last reset is listed together.
fn dir_for(_round: &str) -> Result<PathBuf, String> {
    let dir = report::verdicts_dir()?.join(crate::git::head_sha());
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    Ok(dir)
}

// Where the last round wrote, kept beside the claim rather than in the commit diary: a diary is dropped the moment HEAD moves, which is exactly when the report of the review that let it move is asked for.
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

// The pointer, not the logs: a landed commit keeps its reports and stops answering for the next one.
pub fn forget_last() {
    if let Ok(path) = crate::git::git_path("agent-verdict.last") {
        let _ = std::fs::remove_file(path);
    }
}

// A reset drops the verdicts, so it drops what they wrote: what a later listing shows is what has been reviewed since.
pub fn abandon_logs() {
    if let Ok(path) = crate::git::git_path("agent-verdict.last") {
        let _ = std::fs::remove_file(path);
    }
    if let Ok(dir) = report::verdicts_dir() {
        let _ = std::fs::remove_dir_all(dir.join(crate::git::head_sha()));
    }
}

// The round's last word about itself. Written whatever happens, so a directory without one is a round that died before it could speak.
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

// Opened by the process that will hold it and by nothing else. A reader finds a writer exactly while a round lives, so the end of a round is an end-of-file rather than a fact somebody has to record.
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

// Two forks and a session of its own: one fork leaves the caller's tree the moment the middle process exits, and the session leaves its process group. A reaper that walks either finds nothing of this round. The work is a closure because after the fork the child already holds everything the caller had — passing it a verb to look up would put this side in the business of knowing what a review is.
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
    // Described before the fork, so a caller arriving in the moment between the spawn and the first gate finds a round rather than an unreadable claim. `pid 0` is what says the round has not named itself yet, and nothing signals it until it has.
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

// One line or none: a round that dies before it can name itself closes the descriptor, and the read ends rather than waiting on a process that will never write.
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

// Everything from here runs with nobody watching: stdout and stderr are a file, the caller may already be gone, and what this leaves behind is the only account of what happened.
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

// Re-pointed rather than opened once: the process writes to whichever file the work it is doing belongs to.
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

// A read that cannot block on the open and cannot miss the close. Opening a pipe for reading without a writer succeeds at once, and the read that follows tells the two apart: nothing to read and no writer is an end-of-file, while nothing to read with a writer holding the other end is the round still running. Only then is there anything to wait for, and the wait ends when that writer closes.
fn hung_up() -> Result<(), String> {
    use std::os::unix::fs::OpenOptionsExt;
    let path = pipe_path()?;
    let file = match std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&path)
    {
        Ok(file) => file,
        // Nothing has ever opened one here, which is an answer, not a fault.
        Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(why) => return Err(format!("cannot open {}: {why}", path.display())),
    };
    let mut byte = [0u8; 1];
    loop {
        let read = unsafe { libc::read(file.as_raw_fd(), byte.as_mut_ptr().cast(), 1) };
        if read == 0 {
            return Ok(());
        }
        if read < 0 {
            let why = std::io::Error::last_os_error();
            match why.kind() {
                std::io::ErrorKind::WouldBlock => {}
                std::io::ErrorKind::Interrupted => continue,
                _ => return Err(format!("cannot read {}: {why}", path.display())),
            }
        }
        let mut watched = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // No deadline: a round ends when its process does, and that is the event being waited for.
        if unsafe { libc::poll(&mut watched, 1, -1) } < 0
            && std::io::Error::last_os_error().kind() != std::io::ErrorKind::Interrupted
        {
            return Err(format!(
                "cannot wait on {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
    }
}

// What the round decided, read and never judged: it wrote its own conclusion, and a second opinion here would be one nobody asked for.
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

// Ending a round is something a person does on purpose, so it is a verb rather than a signal somebody has to find a pid for. Every verdict already earned is kept: throwing those away to stop one gate is paying twice.
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
