// Concern: everything the tool prints — the skip line, each rejection, the reviewer block that remedies it | Non-concern: deciding whether a gate passed | IO: (gate, reason) -> stderr

use crate::trailer::key_for;
use crate::Invocation;

const TEMPLATE: &str = include_str!("prompt.md");

pub fn skipped(gate: &str, paths: &[String]) {
    eprintln!(
        "git-agent-verdict: {gate}: skipped (no staged file matches {})",
        paths.join(", ")
    );
}

pub fn prompt(gate: &str, docs: &[String]) -> String {
    let docs = docs
        .iter()
        .map(|d| format!("  {d}"))
        .collect::<Vec<_>>()
        .join("\n");
    TEMPLATE
        .lines()
        .skip(1)
        .collect::<Vec<_>>()
        .join("\n")
        .replace("{{gate}}", gate)
        .replace("{{docs}}", &docs)
}

fn shape(inv: &Invocation) -> String {
    let key = key_for(&inv.gate);
    let tail = if inv.per_file { " file=<path>" } else { "" };
    format!("  {key}: reviewer=<id> major=0 moderate=0 minor=<n>{tail}")
}

pub fn missing(inv: &Invocation, detail: &str) {
    eprintln!("\ngit-agent-verdict: {}: REVIEW GATE FAILED\n", inv.gate);
    eprintln!("MISSING — {detail}\n");
    eprintln!("{}\n", shape(inv));
    eprintln!("Earned by a review you run yourself: spawn a reviewer in a fresh context, hand it");
    eprintln!("the block below, iterate `re-review` until it reports major=0 and moderate=0, then");
    eprintln!("write its counts into the trailer. Trailers must be the LAST paragraph.\n");
    eprintln!("── FORWARD BELOW THIS LINE ──");
    eprintln!("{}", prompt(&inv.gate, &inv.docs));
}

fn refused(label: &str, judged_by: &str, rubrics: &[String]) {
    eprintln!("\ngit-agent-verdict: {label}: RUBRIC IS STAGED\n");
    eprintln!("  {}", rubrics.join("\n  "));
    eprintln!("\nThis commit changes a yardstick {judged_by} is judged against.");
    eprintln!("Judging a change to the measure against that same measure is circular, so it");
    eprintln!("lands on its own, unreviewed:\n\n  git commit --no-verify\n");
    eprintln!("Keep it in a SEPARATE commit from any other change, which still needs its review.");
}

pub fn circular(gate: &str, rubrics: &[String]) {
    refused(gate, &format!("the {gate} review"), rubrics);
}

// The preflight names no gate: it holds the whole hook's rubrics, and the staged one may belong to any of them.
pub fn preflight(rubrics: &[String]) {
    refused(crate::GUARD_LABEL, "a review in this hook", rubrics);
}

pub fn attested(gate: &str, count: usize, minor: u32) {
    eprintln!("git-agent-verdict: {gate}: attested ({count} verdict(s), minor={minor})");
}

pub fn blocked(gate: &str, major: u32, moderate: u32) {
    eprintln!("\ngit-agent-verdict: {gate}: DECLARED BLOCKER");
    eprintln!("  major={major} moderate={moderate} (both must be 0)");
}

pub fn malformed(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: {gate}: MALFORMED TRAILER\n  {detail}");
}
