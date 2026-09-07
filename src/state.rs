// Concern: the recorded verdicts for the commit being written — their tokens, its intent, its resets, and the review it has open | Non-concern: running a review | IO: (gate, verdicts) -> token

use crate::git;
use crate::trailer::{Counts, Verdict};
use std::path::{Path, PathBuf};

const DIR: &str = "agent-verdict";
const PROGRESS: &str = "progress";
const INTENT: &str = "intent";
const RESETS: &str = "resets.log";
const PENDING: &str = "pending";
const PROPOSED: &str = "proposed-intent";

// A diary, not a vault: `--no-verify` exists. This only stops accidental edits and a grep.
fn digest(bytes: &[u8], seed: u64) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64 ^ seed;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

pub fn fingerprint(token: &str) -> String {
    format!("{:016x}", digest(token.as_bytes(), 0x5eed))
}

fn obfuscate(text: &[u8], token: &str) -> Vec<u8> {
    let key = token.as_bytes();
    text.iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()])
        .collect()
}

fn issue(gate: &str, material: &str) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let seed = format!("{gate}{material}{now}{}", std::process::id());
    format!(
        "{:016x}{:016x}",
        digest(seed.as_bytes(), 0x91d2),
        digest(seed.as_bytes(), 0xa5b7)
    )
}

fn root() -> Result<PathBuf, String> {
    git::git_path(DIR)
}

// Keyed on HEAD: the commit being reviewed doesn't exist yet, and HEAD moves only once it lands.
fn here() -> Result<PathBuf, String> {
    let dir = root()?.join(git::head_sha());
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    gc(&dir);
    Ok(dir)
}

// A rebase or checkout mid-review orphans a directory; dropped on the next write, not by a verb nobody runs.
fn gc(keep: &Path) {
    let Ok(root) = root() else { return };
    let Ok(entries) = std::fs::read_dir(&root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() && path != keep {
            let _ = std::fs::remove_dir_all(&path);
        }
    }
}

fn serialize(verdicts: &[Verdict]) -> String {
    verdicts
        .iter()
        .map(|v| {
            let Counts {
                major,
                moderate,
                minor,
            } = v.counts;
            format!("{}\t{major}:{moderate}:{minor}\t{}", v.reviewer, v.session)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn counts_from(field: &str) -> Option<Counts> {
    let mut parts = field.split(':');
    Some(Counts {
        major: parts.next()?.parse().ok()?,
        moderate: parts.next()?.parse().ok()?,
        minor: parts.next()?.parse().ok()?,
    })
}

fn deserialize(text: &str, token: &str) -> Option<Vec<Verdict>> {
    let mut verdicts = Vec::new();
    for line in text.lines() {
        let mut fields = line.split('\t');
        let reviewer = fields.next()?.to_string();
        let counts = counts_from(fields.next()?)?;
        let session = fields.next()?.to_string();
        verdicts.push(Verdict {
            reviewer,
            counts,
            token: token.to_string(),
            resets: 0,
            session,
        });
    }
    Some(verdicts)
}

pub struct Step {
    pub gate: String,
    pub token: String,
    pub blocked: bool,
}

pub fn progress() -> Result<Vec<Step>, String> {
    let path = here()?.join(PROGRESS);
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    let mut steps = Vec::new();
    for line in text.lines() {
        let mut fields = line.split('\t');
        let (Some(gate), Some(token), Some(outcome)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        steps.push(Step {
            gate: gate.to_string(),
            token: token.to_string(),
            blocked: outcome == "major",
        });
    }
    Ok(steps)
}

// Found still here means the round died in flight; its session is still on disk with what it had read.
pub struct Pending {
    pub gate: String,
    pub session: String,
}

pub fn open_round(gate: &str, session: &str) -> Result<(), String> {
    let path = here()?.join(PENDING);
    std::fs::write(&path, format!("{gate}\t{session}"))
        .map_err(|e| format!("cannot write {}: {e}", path.display()))
}

pub fn close_round() {
    if let Ok(dir) = here() {
        let _ = std::fs::remove_file(dir.join(PENDING));
    }
}

pub fn in_flight() -> Result<Option<Pending>, String> {
    let Ok(text) = std::fs::read_to_string(here()?.join(PENDING)) else {
        return Ok(None);
    };
    let mut fields = text.trim().split('\t');
    let (Some(gate), Some(session)) = (fields.next(), fields.next()) else {
        return Ok(None);
    };
    Ok(Some(Pending {
        gate: gate.to_string(),
        session: session.to_string(),
    }))
}

pub fn record(gate: &str, verdicts: &[Verdict], blocked: bool) -> Result<String, String> {
    let dir = here()?;
    // A marker outliving its verdict is a resume of a reviewer that already answered.
    close_round();
    let body = serialize(verdicts);
    let token = issue(gate, &body);
    let entry = dir.join(fingerprint(&token));
    let stored = obfuscate(format!("{gate}\n{body}").as_bytes(), &token);
    std::fs::write(&entry, stored).map_err(|e| format!("cannot write {}: {e}", entry.display()))?;
    let outcome = if blocked { "major" } else { "pass" };
    let line = format!("{gate}\t{token}\t{outcome}\n");
    let progress = dir.join(PROGRESS);
    let mut text = std::fs::read_to_string(&progress).unwrap_or_default();
    text.push_str(&line);
    std::fs::write(&progress, text)
        .map_err(|e| format!("cannot write {}: {e}", progress.display()))?;
    Ok(token)
}

pub fn intent() -> Result<Option<String>, String> {
    let path = here()?.join(INTENT);
    Ok(std::fs::read_to_string(path).ok())
}

// Never the aim a reviewer is briefed against — only an accepted intent is that.
pub fn proposed() -> Result<Option<String>, String> {
    let path = here()?.join(PROPOSED);
    Ok(std::fs::read_to_string(path).ok())
}

pub fn propose(intent: &str) -> Result<(), String> {
    let path = here()?.join(PROPOSED);
    std::fs::write(&path, intent).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

// Accepted, so the proposal has served its purpose; or refused, so it must not be asked again under the same wording.
pub fn settle_intent(accepted: bool) -> Result<(), String> {
    let dir = here()?;
    if accepted {
        let proposed = std::fs::read_to_string(dir.join(PROPOSED))
            .map_err(|e| format!("cannot read the proposed intent: {e}"))?;
        std::fs::write(dir.join(INTENT), proposed)
            .map_err(|e| format!("cannot write {}: {e}", dir.join(INTENT).display()))?;
    }
    let _ = std::fs::remove_file(dir.join(PROPOSED));
    Ok(())
}

pub struct Record {
    pub gate: String,
    pub verdicts: Vec<Verdict>,
}

pub fn lookup(token: &str) -> Result<Option<Record>, String> {
    let entry = here()?.join(fingerprint(token));
    let Ok(stored) = std::fs::read(&entry) else {
        return Ok(None);
    };
    let text = String::from_utf8_lossy(&obfuscate(&stored, token)).into_owned();
    let Some((gate, body)) = text.split_once('\n') else {
        return Ok(None);
    };
    Ok(deserialize(body, token).map(|verdicts| Record {
        gate: gate.to_string(),
        verdicts,
    }))
}

// Outside the per-commit directory a reset clears: a log that a reset erases is a log of nothing.
pub fn log_reset(reason: &str) -> Result<u32, String> {
    let root = root()?;
    std::fs::create_dir_all(&root).map_err(|e| format!("cannot create {}: {e}", root.display()))?;
    let path = root.join(RESETS);
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    text.push_str(&format!("{}\t{reason}\n", git::head_sha()));
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let dir = root.join(git::head_sha());
    let _ = std::fs::remove_dir_all(&dir);
    resets()
}

pub fn resets() -> Result<u32, String> {
    Ok(reasons()?.len() as u32)
}

pub fn reasons() -> Result<Vec<String>, String> {
    let path = root()?.join(RESETS);
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(Vec::new());
    };
    let head = git::head_sha();
    Ok(text
        .lines()
        .filter_map(|l| l.strip_prefix(&head)?.strip_prefix('\t'))
        .map(str::to_string)
        .collect())
}
