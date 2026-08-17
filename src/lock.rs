// Concern: one attest at a time in a repo — taking the claim, proving an abandoned one dead, releasing it | Non-concern: what runs while it is held | IO: (repo) -> guard

use crate::git;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

// Held for as long as the run is, and released by going out of scope: an early return on any refusal below would otherwise leave the repo claimed by a process that has exited.
pub struct Held {
    path: PathBuf,
}

impl Drop for Held {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

// The name, not the path it was found at: a caller reaching this through PATH leaves a bare `git-agent-verdict` in argv, and a claim compared against a full path would read every live holder as dead.
fn base(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

fn mine() -> String {
    std::env::current_exe().map_or_else(
        |_| "git-agent-verdict".to_string(),
        |p| base(&p.to_string_lossy()).to_string(),
    )
}

// A number alone would wedge a repo once pids came round again, so the claim carries what runs under it and is believed only while that matches. Asked of `ps`, not /proc, which macOS lacks: there every pid reads as dead and every claim is stolen — a guard holding nothing while appearing to. A box that cannot answer keeps its claim, since taking it anyway is the race again.
fn running(pid: &str, exe: &str) -> bool {
    let Ok(out) = std::process::Command::new("ps")
        .args(["-p", pid, "-o", "args="])
        .output()
    else {
        return true;
    };
    if !out.status.success() {
        return false;
    }
    // argv[0] alone, so a process merely carrying the name in an argument is not mistaken for one of these.
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace()
        .next()
        .is_some_and(|argv0| base(argv0) == exe)
}

fn holder(text: &str) -> Option<(String, u64, String)> {
    let mut fields = text.trim().split('\t');
    let pid = fields.next()?.to_string();
    let since = fields.next()?.parse().ok()?;
    let exe = fields.next()?.to_string();
    Some((pid, since, exe))
}

// A refusal rather than a wait: waiting is what the caller would have written by hand, and a wait that never ends is the failure this exists to prevent. How long it has been held is the number that says whether to wait or to kill.
fn refusal(text: &str, path: &std::path::Path) -> Option<String> {
    let (pid, since, exe) = holder(text)?;
    if !running(&pid, &exe) {
        return None;
    }
    let held = now().saturating_sub(since);
    Some(format!(
        "another attest is already running in this repo — pid {pid}, {held}s so far.\nOne at a time: two runs review the same gate, pay for it twice, and the second to finish drops the first's verdict.\nWait for it, or kill it. The claim is {}.",
        path.display()
    ))
}

pub fn take() -> Result<Held, String> {
    let path = git::git_path("agent-verdict.lock")?;
    let claim = format!("{}\t{}\t{}", std::process::id(), now(), mine());
    for _ in 0..2 {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                use std::io::Write;
                let _ = file.write_all(claim.as_bytes());
                return Ok(Held { path });
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                if let Some(said) = refusal(&text, &path) {
                    return Err(said);
                }
                // Nothing is running under it: a run that was killed leaves this behind, and a repo no command can enter again is worse than the race the claim prevents. Said rather than cleared in silence — the claim is this tool's own evidence that the last run died, and how long it had been going when it did, which nothing else here would ever mention.
                if let Some((pid, since, _)) = holder(&text) {
                    crate::report::abandoned(&pid, &path, now().saturating_sub(since));
                }
                let _ = std::fs::remove_file(&path);
            }
            Err(e) => return Err(format!("cannot claim {}: {e}", path.display())),
        }
    }
    Err(format!(
        "cannot claim {}: it is being taken and dropped faster than this run can read it",
        path.display()
    ))
}
