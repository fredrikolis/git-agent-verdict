// Concern: one review at a time in a repo — taking the claim, and passing it to the reviewer that outlives the run | Non-concern: what runs while it is held | IO: (repo) -> guard

use crate::git;
use std::os::unix::io::AsRawFd;
use std::time::{SystemTime, UNIX_EPOCH};

// The kernel holds the claim, not a pid written in a file. A run killed from outside leaves its reviewer working, and a claim that ended with the run would let the next attest open a second review beside a live one. Held on the descriptor and inherited across the spawn, it is released when the last process holding it exits. Advisory and per-descriptor, which also bounds it: over NFS flock is emulated or ignored, so two machines sharing one checkout would each believe they hold it.
pub struct Held {
    _file: std::fs::File,
}

// The descriptor is this run's only while the guard owns the file. Left set afterwards, the number is one the kernel may have handed to something else, and a later spawn would open that up instead.
impl Drop for Held {
    fn drop(&mut self) {
        CLAIM.store(-1, std::sync::atomic::Ordering::Relaxed);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// When the claim was taken, and nothing else. No pid: the run that writes one is usually the first to die while its reviewer holds the claim long after, and a reader sent to a pid that is already gone concludes the opposite of the truth.
fn note(path: &std::path::Path) {
    let _ = std::fs::write(path, now().to_string());
}

fn how_long(path: &std::path::Path) -> String {
    match std::fs::read_to_string(path)
        .ok()
        .and_then(|text| text.trim().parse::<u64>().ok())
    {
        // Absent, zero or in the future is a note nobody wrote for the claim being reported on. One left by an earlier run and read before the holder rewrites it still reads as its own age, which is why this number is a sentence and never a decision.
        Some(taken) if taken > 0 && taken <= now() => format!("{}s so far", now() - taken),
        _ => "for a time this run cannot tell".to_string(),
    }
}

pub fn take() -> Result<Held, String> {
    let path = git::git_path("agent-verdict.lock")?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let fd = file.as_raw_fd();
    // A refusal rather than a wait: waiting is what the caller would have written by hand, and a wait that never ends is the failure this exists to prevent. Held is one answer and broken is another, so a claim this run could not even ask about is not reported as a review to wait behind.
    if unsafe { libc::flock(fd, libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let why = std::io::Error::last_os_error();
        if why.kind() != std::io::ErrorKind::WouldBlock {
            return Err(format!("cannot claim {}: {why}", path.display()));
        }
        return Err(format!(
            "another review is already running in this repo — {}.\nOne at a time: two runs review the same gate, pay for it twice, and the second to finish drops the first's verdict.\nWait for it, or end what holds it: {}",
            how_long(&path),
            holders(&path)
        ));
    }
    CLAIM.store(fd, std::sync::atomic::Ordering::Relaxed);
    note(&path);
    Ok(Held { _file: file })
}

// The descriptor stays close-on-exec for every other child this run spawns, the git commands and the hook it re-runs, and is opened up for the agent it hands the round to. Cleared on the file itself instead, the claim would ride into every one of them, and any that outlived the run would hold the repo for reasons nobody could see.
static CLAIM: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(-1);

// The claim passes to the child and on to whatever that child spawns, and nothing here can take it back. Ending what holds it is the caller's, which is why it puts the child in its own group first.
pub fn passed_to(command: &mut std::process::Command) {
    let fd = CLAIM.load(std::sync::atomic::Ordering::Relaxed);
    if fd < 0 {
        return;
    }
    use std::os::unix::process::CommandExt;
    // In the child, after the fork and before the exec: clearing the one flag on its own copy, so the claim survives into the reviewer and nothing else.
    unsafe {
        command.pre_exec(move || {
            let flags = libc::fcntl(fd, libc::F_GETFD);
            if flags >= 0 {
                libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC);
            }
            Ok(())
        });
    }
}

// Which processes hold the claim, read off the kernel rather than off a pid this tool wrote down. The claim is inherited on purpose, so the holder is usually not the run that took it and may be a reviewer whose parent was reaped an hour ago; nothing else in the repo records it. This run holds the file open while it asks, so it is the one holder that is never worth naming.
fn holders(path: &std::path::Path) -> String {
    let Ok(claim) = std::fs::canonicalize(path) else {
        return NOBODY.to_string();
    };
    let held = from_proc(&claim).or_else(|| from_lsof(&claim));
    match held {
        Some(held) if !held.is_empty() => format!("\n  {}", held.join("\n  ")),
        Some(_) => {
            "nothing this user can see holds it, so it belongs to another account".to_string()
        }
        None => NOBODY.to_string(),
    }
}

const NOBODY: &str = "nothing here can tell what holds it";

fn ours(pid: u32) -> bool {
    pid == std::process::id()
}

// Linux, where every process publishes what it has open. None is not the same as an empty list: a kernel that does not publish this cannot say nobody holds the claim, and the two are reported differently.
fn from_proc(claim: &std::path::Path) -> Option<Vec<String>> {
    let mut held = Vec::new();
    for process in std::fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = process.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if ours(pid) {
            continue;
        }
        let Ok(open) = std::fs::read_dir(process.path().join("fd")) else {
            continue;
        };
        if open
            .flatten()
            .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|at| at == claim))
        {
            let named = std::fs::read_to_string(process.path().join("comm")).unwrap_or_default();
            held.push(ending(pid, named.trim()));
        }
    }
    Some(held)
}

// Everywhere else, macos included, where the same question is only answerable by asking a tool that may not be installed.
fn from_lsof(claim: &std::path::Path) -> Option<Vec<String>> {
    let asked = std::process::Command::new("lsof")
        .args(["-t", "--"])
        .arg(claim)
        .output()
        .ok()?;
    Some(
        String::from_utf8_lossy(&asked.stdout)
            .lines()
            .filter_map(|line| line.trim().parse::<u32>().ok())
            .filter(|pid| !ours(*pid))
            .map(|pid| ending(pid, "holding this repo"))
            .collect(),
    )
}

fn ending(pid: u32, named: &str) -> String {
    format!("kill {pid}   # {named}")
}
