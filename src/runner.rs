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

// A refusal is not a count, so it is not a verdict shape: an advisory gate blocks on it exactly as a graded one does.
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
    // Written from its own thread, because both pipes are bounded: a reviewer that talks while the brief goes in fills stdout and waits for a read this side cannot reach until its own write finishes.
    let text = brief.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(text.as_bytes()));
    let out = child
        .wait_with_output()
        .map_err(|e| format!("the reviewer did not finish: {e}"))?;
    // A reviewer that answers without reading closes the pipe first; that is its business, and the verdict it prints is still the verdict.
    match writer.join() {
        Err(_) => return Err("the brief was never written to the reviewer".to_string()),
        Ok(Err(e)) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            return Err(format!("cannot brief the reviewer: {e}"));
        }
        Ok(_) => {}
    }
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
        // Named but unreadable is not the same as absent: read as absent it would be reported as a missing field, sending the author after the wrong fault.
        found[slot] = Some(raw.parse().map_err(|_| {
            format!("the reviewer's {MARKER} line has {name}={raw}, which is not a number")
        })?);
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

// Everything the reviewer said that was not its verdict line. The counts say how much was found; only this says what, and an author told to address a finding it cannot read has been told nothing.
pub fn findings(output: &str) -> String {
    output
        .lines()
        .filter(|l| !l.trim().starts_with(MARKER))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
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
    // One review, one verdict: the rest is recorded under a single token and rendered as a trailer apiece, which the gate reads as trailers contradicting the review they name.
    if verdicts.len() > 1 {
        return Err(format!(
            "the reviewer closed with {} `{MARKER}` lines; the brief asks for one, and which of them is the review is not this tool's to guess",
            verdicts.len()
        ));
    }
    Ok(verdicts)
}
