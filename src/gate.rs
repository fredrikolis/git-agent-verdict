// Concern: one gate's decision about a commit message — what it demands, and what it refuses | Non-concern: running a review, or the wording of a rejection | IO: (message, index) -> pass or refusal

use crate::cli::Invocation;
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

// The inverse verb: fire BECAUSE a yardstick is staged, and always refuse — judging a change to the measure against that same measure is circular.
pub fn staged_rubrics(docs: &[String]) -> Result<Vec<String>, String> {
    let in_repo: Vec<String> = docs
        .iter()
        .filter_map(|d| git::relative_to_root(d))
        .collect();
    if in_repo.is_empty() {
        return Ok(Vec::new());
    }
    let staged = git::staged(&in_repo)?;
    Ok(in_repo.into_iter().filter(|d| staged.contains(d)).collect())
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

// Runs before any gate, so a rubric belonging to a LATER gate is caught without first paying for an earlier gate's review. The per-gate guard stays the backstop, so drift here only costs an early exit.
pub fn rubric_guard(docs: &[String]) -> Result<bool, String> {
    let rubrics = staged_rubrics(docs)?;
    if rubrics.is_empty() {
        return Ok(true);
    }
    report::preflight(&rubrics);
    Ok(false)
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
    let rubrics = staged_rubrics(&inv.docs)?;
    if !rubrics.is_empty() {
        report::circular(&inv.gate, &rubrics);
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
    // A simple gate demands the review and records it; what the review found is the author's to act on, so no count of it is a blocker.
    if !inv.brief.simple && verdicts.iter().any(Verdict::blocks) {
        report::blocked(&inv.gate, trailer::total(&verdicts).major());
        return Ok(false);
    }
    report::attested(&inv.gate, verdicts.len(), &verdicts);
    Ok(true)
}
