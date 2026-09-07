// Concern: one review at a time in a repository — taking the lock, and what it records about the review holding it | Non-concern: an interrupted review | IO: (repo) -> guard, live review

use crate::git;
use std::io::{Seek, Write};
use std::os::unix::io::AsRawFd;
use std::time::{SystemTime, UNIX_EPOCH};

// The kernel holds the claim, not a pid in a file — released only when the last holder exits. flock is advisory and unreliable over NFS.
pub struct Held {
    file: std::fs::File,
}

pub enum Landed {
    Landing,
    Round(Live),
}

pub struct Live {
    pub label: String,
    pub round: String,
    pub started: u64,
    pub ceiling: u64,
    pub pid: u32,
    pub gate: String,
}

const TERMINATOR: &str = ".";

const LANDING: &str = "landing";

impl Landed {
    fn render(&self) -> String {
        match self {
            Landed::Landing => format!("{LANDING}\n{TERMINATOR}\n"),
            Landed::Round(live) => format!(
                "{}\n{}\n{}\n{}\n{}\n{}\n{TERMINATOR}\n",
                live.label, live.round, live.started, live.ceiling, live.pid, live.gate
            ),
        }
    }

    fn parse(text: &str) -> Option<Landed> {
        let lines: Vec<&str> = text.lines().collect();
        if lines.first() == Some(&LANDING) {
            return (lines.get(1) == Some(&TERMINATOR)).then_some(Landed::Landing);
        }
        if lines.len() < 7 || lines[6] != TERMINATOR {
            return None;
        }
        Some(Landed::Round(Live {
            label: lines[0].to_string(),
            round: lines[1].to_string(),
            started: lines[2].parse().ok()?,
            ceiling: lines[3].parse().ok()?,
            pid: lines[4].parse().ok()?,
            gate: lines[5].to_string(),
        }))
    }
}

impl Live {
    pub fn how_long(&self) -> u64 {
        now().saturating_sub(self.started)
    }
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Held {
    // Rewritten, not appended: the description is of the round now, which moves as fields become known.
    pub fn describe(&self, live: &Landed) -> Result<(), String> {
        let mut file = self
            .file
            .try_clone()
            .map_err(|e| format!("cannot write the lock file: {e}"))?;
        file.set_len(0)
            .map_err(|e| format!("cannot write the lock file: {e}"))?;
        // Truncating moves no offset; without this, the write lands past the end.
        file.rewind()
            .map_err(|e| format!("cannot write the lock file: {e}"))?;
        file.write_all(live.render().as_bytes())
            .map_err(|e| format!("cannot write the lock file: {e}"))
    }
}

fn path() -> Result<std::path::PathBuf, String> {
    git::git_path("agent-verdict.lock")
}

pub fn describing() -> Option<Landed> {
    Landed::parse(&std::fs::read_to_string(path().ok()?).ok()?)
}

pub fn take() -> Result<Held, String> {
    let path = path()?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    // A refusal, not a wait: an unbounded wait for the lock is the failure this exists to prevent.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let why = std::io::Error::last_os_error();
        if why.kind() != std::io::ErrorKind::WouldBlock {
            return Err(format!("cannot lock {}: {why}", path.display()));
        }
        return Err(occupied());
    }
    Ok(Held { file })
}

fn occupied() -> String {
    let here = git::toplevel().unwrap_or_else(|_| "<the repo root>".to_string());
    match describing() {
        None => format!("a review is starting in {here}; retry"),
        Some(Landed::Landing) => format!("a commit is being written in {here}; retry"),
        Some(Landed::Round(live)) => format!(
            "a review is already running in {here} — {}, elapsed {}s, pid {}.\nuse either:\n  git agent-verdict await --repo {here}\n  git agent-verdict abort --repo {here}",
            live.label,
            live.how_long(),
            live.pid
        ),
    }
}
