// Concern: everything the tool prints — the skip line, each rejection, the reviewer block that remedies it | Non-concern: deciding whether a gate passed | IO: (gate, reason) -> stderr

use crate::trailer::key_for;
use crate::{Brief, Invocation};

const TEMPLATE: &str = include_str!("prompt.md");
const LADDER: &str = include_str!("ladder.md");
const LADDER_SIMPLE: &str = include_str!("ladder-simple.md");

pub fn skipped(gate: &str, paths: &[String]) {
    eprintln!(
        "git-agent-verdict: {gate}: skipped (no staged file matches {})",
        paths.join(", ")
    );
}

// Only a built-in template carries an annotation line; an override is a repo's own file, and eating its first line would be a silent edit.
fn built_in(text: &str) -> String {
    text.lines().skip(1).collect::<Vec<_>>().join("\n")
}

pub fn prompt(gate: &str, docs: &[String], brief: &Brief) -> Result<String, String> {
    let template = match &brief.prompt {
        Some(path) => {
            std::fs::read_to_string(path).map_err(|e| format!("--override-prompt {path}: {e}"))?
        }
        None => built_in(TEMPLATE),
    };
    let docs = docs
        .iter()
        .map(|d| format!("  {d}"))
        .collect::<Vec<_>>()
        .join("\n");
    let ladder = built_in(if brief.simple { LADDER_SIMPLE } else { LADDER });
    Ok(template
        .replace("{{gate}}", gate)
        .replace("{{docs}}", &docs)
        .replace("{{ladder}}", &ladder))
}

// The zero is shown only where one is demanded: every other count is a slot for what the reviewer actually reported, and a literal 0 in the shape is an invitation to write one.
fn shape(inv: &Invocation) -> String {
    let key = key_for(&inv.gate);
    let tail = if inv.per_file { " file=<path>" } else { "" };
    let major = if inv.brief.simple { "<n>" } else { "0" };
    format!("  {key}: reviewer=<id> major={major} moderate=<n> minor=<n>{tail}")
}

// What the counts cost is the one thing an advisory gate states differently, so it is the one thing spelled out per mode.
const EARNED: &str = "\
Earned by a review you run yourself: spawn a reviewer in a fresh context, hand it the
block below, fix every MODERATE it names, then write the counts it REPORTED into the
trailer. Only major=0 passes; there is no re-review. Trailers must be the LAST paragraph.";

const EARNED_SIMPLE: &str = "\
Earned by a review you run yourself: spawn a reviewer in a fresh context, hand it the
block below, then write the counts it reported into the trailer. This gate is advisory:
nothing it finds blocks the commit. Trailers must be the LAST paragraph.";

pub fn missing(inv: &Invocation, detail: &str) -> Result<(), String> {
    eprintln!("\ngit-agent-verdict: {}: REVIEW GATE FAILED\n", inv.gate);
    eprintln!("MISSING — {detail}\n");
    eprintln!("{}\n", shape(inv));
    eprintln!(
        "{}\n",
        if inv.brief.simple {
            EARNED_SIMPLE
        } else {
            EARNED
        }
    );
    eprintln!("── FORWARD BELOW THIS LINE ──");
    eprintln!("{}", prompt(&inv.gate, &inv.docs, &inv.brief)?);
    Ok(())
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

// All three counts, because a passing commit now carries findings: only major= had to be zero.
pub fn attested(gate: &str, count: usize, counts: (u32, u32, u32)) {
    let (major, moderate, minor) = counts;
    eprintln!(
        "git-agent-verdict: {gate}: attested ({count} verdict(s), major={major} moderate={moderate} minor={minor})"
    );
}

pub fn blocked(gate: &str, major: u32) {
    eprintln!("\ngit-agent-verdict: {gate}: DECLARED BLOCKER");
    eprintln!("  major={major} (must be 0)");
    eprintln!("\nA MAJOR is not the author's to patch. The fix is re-planned by an agent that did");
    eprintln!("not write the change, then implemented and reviewed afresh.");
}

// The install command is in the line: the reader is a hook's stderr, and an agent told only that the binary is old will otherwise invent one.
pub fn stale(want: &str, have: &str) {
    eprintln!(
        "git-agent-verdict: {have} is older than the required {want}: cargo install git-agent-verdict --version '>={want}'"
    );
}

pub fn malformed(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: {gate}: MALFORMED TRAILER\n  {detail}");
}
