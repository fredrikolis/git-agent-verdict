// Concern: one gate's decision about a commit message — what it demands, and what it refuses | Non-concern: running a review, or the wording of a rejection | IO: (message, index) -> pass or refusal

use crate::cli::Invocation;
use crate::declarations;
use crate::git;
use crate::report;
use crate::state;
use crate::trailer::{self, Verdict};

// git writes these subjects itself; they carry no review and must not be blocked.
fn auto_generated(raw: &str) -> bool {
    let subject = raw
        .lines()
        .find(|l| !l.starts_with('#') && !l.trim().is_empty())
        .unwrap_or_default();
    match subject.split_once(' ').map(|(head, _)| head) {
        Some("Merge") => git::in_progress("MERGE_HEAD"),
        Some("Revert") => git::in_progress("REVERT_HEAD"),
        _ => subject.starts_with("fixup!") || subject.starts_with("squash!"),
    }
}

// Repo-relative, as git matches it, and None for a path outside the worktree: git goes fatal on a pathspec it cannot place, and a rubric kept outside the repo — the $KB case the setup guide documents — can never be staged, so there is nothing to ask git about.
fn in_repo(path: &str) -> Option<String> {
    if std::path::Path::new(path).is_absolute() {
        git::relative_to_root(path)
    } else {
        Some(path.to_string())
    }
}

// What the repo gates by: the hook naming the gates, and every measure they judge against. A change to either is reviewed by the maintainer who made it, which is no review at all — so it is maintenance, out of scope here, and lands on its own.
pub fn machinery_staged() -> Result<Vec<String>, String> {
    let mut watched: Vec<String> = Vec::new();
    if let Some(hook) = git::hook_path().ok().and_then(|h| in_repo(&h)) {
        watched.push(hook);
    }
    if let Ok(hook) = declarations::read() {
        for gate in &hook.gates {
            for doc in gate.docs.iter().filter_map(|d| in_repo(d)) {
                if !watched.contains(&doc) {
                    watched.push(doc);
                }
            }
        }
    }
    if watched.is_empty() {
        return Ok(Vec::new());
    }
    let staged = git::staged(&watched)?;
    Ok(watched.into_iter().filter(|w| staged.contains(w)).collect())
}

// The one edit this tool makes to a message: every commit in a repo gated this way is agent-written, so a fixed attribution line is constant and carries nothing.
fn drop_agent_coauthor(msg_file: &str, raw: &str) -> Result<String, String> {
    if !raw.lines().any(trailer::is_agent_coauthor) {
        return Ok(raw.to_string());
    }
    let kept: Vec<&str> = raw
        .lines()
        .filter(|l| !trailer::is_agent_coauthor(l))
        .collect();
    let mut text = kept.join("\n");
    text.push('\n');
    std::fs::write(msg_file, &text).map_err(|e| format!("cannot rewrite {msg_file}: {e}"))?;
    Ok(text)
}

// The counts in the message are compared against the ones the reviewer actually reported. This is the only check that can catch a trailer that reads better than its review did.
fn traced(gate: &str, verdicts: &[Verdict]) -> Result<bool, String> {
    for verdict in verdicts {
        let Some(record) = state::lookup(&verdict.token)? else {
            report::untraceable(gate, &verdict.token);
            return Ok(false);
        };
        if record.gate != gate {
            let detail = format!("token= names a review of the {} gate", record.gate);
            report::mismatch(gate, &detail);
            return Ok(false);
        }
        let Some(recorded) = record.verdicts.first() else {
            report::mismatch(gate, "the review recorded no verdict");
            return Ok(false);
        };
        if recorded.counts != verdict.counts {
            let detail = format!(
                "the review reported {}, the trailer declares {}",
                recorded.counts.render(),
                verdict.counts.render()
            );
            report::mismatch(gate, &detail);
            return Ok(false);
        }
    }
    Ok(true)
}

fn read_verdicts(inv: &Invocation) -> Result<Option<Vec<Verdict>>, String> {
    let raw = std::fs::read_to_string(&inv.msg_file)
        .map_err(|e| format!("cannot read {}: {e}", inv.msg_file))?;
    if auto_generated(&raw) {
        return Ok(None);
    }
    let raw = drop_agent_coauthor(&inv.msg_file, &raw)?;
    let block = git::trailers(&inv.msg_file)?;
    let verdicts = match trailer::parse_for(&inv.gate, &block) {
        Ok(verdicts) => verdicts,
        Err(detail) => {
            report::malformed(&inv.gate, &detail);
            return Ok(Some(Vec::new()));
        }
    };
    if verdicts.is_empty() {
        let detail = if trailer::present_but_unparsed(&inv.gate, &raw, &block) {
            "the trailer exists but is not in the message's trailing paragraph, so git does not see it"
        } else {
            "the message needs this trailer and has none"
        };
        report::missing(inv, detail);
    }
    Ok(Some(verdicts))
}

pub fn check(inv: &Invocation) -> Result<bool, String> {
    let staged_machinery = machinery_staged()?;
    if !staged_machinery.is_empty() {
        report::maintenance(&staged_machinery);
        return Ok(false);
    }
    let unmatched = git::unmatched_literals(&inv.paths)?;
    if !unmatched.is_empty() {
        return Err(format!(
            "--path names nothing git tracks: {}",
            unmatched.join(", ")
        ));
    }
    if git::staged(&inv.paths)?.is_empty() {
        report::skipped(&inv.gate, &inv.paths);
        return Ok(true);
    }
    let Some(verdicts) = read_verdicts(inv)? else {
        return Ok(true);
    };
    if verdicts.is_empty() {
        return Ok(false);
    }
    if !traced(&inv.gate, &verdicts)? {
        return Ok(false);
    }
    // An advisory gate reports major=0 by construction, so nothing here needs to know which kind of gate this is.
    if verdicts.iter().any(Verdict::blocks) {
        report::blocked(&inv.gate, trailer::total(&verdicts).major);
        return Ok(false);
    }
    report::attested(&inv.gate, verdicts.len(), &verdicts);
    Ok(true)
}
