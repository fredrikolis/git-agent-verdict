// Concern: running the declared reviewer over a brief, and reading the verdict lines it closes with | Non-concern: what the reviewer is asked, or what its numbers mean | IO: (command, brief) -> verdicts

use crate::git;
use crate::trailer::{Counts, Verdict};
use std::io::Write;
use std::process::{Command, Stdio};

const RUNNER_KEY: &str = "agent-verdict.runner";

pub struct Runner {
    pub cmd: String,
}

// Host configuration, not a repo's: a repo that declared its reviewer would pick one for every maintainer, and they do not share a machine, a budget or a preferred agent.
pub fn configured() -> Result<Runner, String> {
    let cmd = git::config(RUNNER_KEY).ok_or_else(|| {
        format!("no reviewer configured: git config --global {RUNNER_KEY} \"<command reading a brief on stdin>\"")
    })?;
    Ok(Runner { cmd })
}

// Demanded, not defaulted: the brief states the line this tool will read, so a runner that omits a field has broken a contract, and a label invented here would put a guess on the record instead of saying so.
fn required(fields: &str, name: &str) -> Result<String, String> {
    fields
        .split_whitespace()
        .find_map(|f| f.strip_prefix(&format!("{name}=")))
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| format!("the reviewer's {MARKER} line carries no {name}=, which the brief asks it to report"))
}

pub const MARKER: &str = "VERDICT:";

// A refusal is not a count, so it is not a verdict shape: an advisory gate blocks on it exactly as a graded one does, which is what the old guard could not do.
pub const REFUSED: &str = "refused";

// Through a shell because the declaration is a command line a repo writes for itself, not an argv this tool composes.
pub fn invoke(runner: &Runner, brief: &str) -> Result<String, String> {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(&runner.cmd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run the declared reviewer ({}): {e}", runner.cmd))?;
    let mut stdin = child.stdin.take().ok_or("the reviewer took no stdin")?;
    // A reviewer that answers without reading closes the pipe first; that is its business, and the verdict it prints is still the verdict.
    if let Err(e) = stdin.write_all(brief.as_bytes()) {
        if e.kind() != std::io::ErrorKind::BrokenPipe {
            return Err(format!("cannot brief the reviewer: {e}"));
        }
    }
    drop(stdin);
    let out = child
        .wait_with_output()
        .map_err(|e| format!("the reviewer did not finish: {e}"))?;
    if !out.status.success() {
        return Err(format!("the reviewer exited {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn counts_from(fields: &str, simple: bool) -> Result<Counts, String> {
    let mut found: [Option<u32>; 4] = [None; 4];
    for field in fields.split_whitespace() {
        let Some((name, raw)) = field.split_once('=') else {
            continue;
        };
        let slot = match name {
            "major" => 0,
            "moderate" => 1,
            "minor" => 2,
            "findings" => 3,
            _ => continue,
        };
        found[slot] = raw.parse().ok();
    }
    match (simple, found) {
        (true, [.., Some(findings)]) => Ok(Counts::Advisory { findings }),
        (false, [Some(major), Some(moderate), Some(minor), _]) => Ok(Counts::Graded {
            major,
            moderate,
            minor,
        }),
        (true, _) => Err("the reviewer's VERDICT line carries no findings=".to_string()),
        (false, _) => {
            Err("the reviewer's VERDICT line needs major=, moderate= and minor=".to_string())
        }
    }
}

// The reviewer's numbers are read here and never retyped by the author: that is the whole reason the tool runs the review rather than handing out a brief.
pub fn verdicts(output: &str, simple: bool) -> Result<Vec<Verdict>, String> {
    let mut verdicts = Vec::new();
    for line in output.lines() {
        let Some(rest) = line.trim().strip_prefix(MARKER) else {
            continue;
        };
        if rest.trim() == REFUSED {
            let detail = "the reviewer refused the brief: an intent that argues for the change is graded instead of the change";
            return Err(detail.to_string());
        }
        verdicts.push(Verdict {
            reviewer: required(rest, "reviewer")?,
            counts: counts_from(rest, simple)?,
            token: String::new(),
            resets: 0,
            session: required(rest, "session")?,
        });
    }
    if verdicts.is_empty() {
        return Err(format!(
            "the reviewer closed with no `{MARKER}` line, so it reported nothing this tool can record"
        ));
    }
    Ok(verdicts)
}
