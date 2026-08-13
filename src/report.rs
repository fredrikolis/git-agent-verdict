// Concern: everything the tool prints — the skip line, each rejection, and what `attest` narrates as it runs | Non-concern: deciding whether a gate passed | IO: (gate, reason) -> stderr

use crate::cli::Invocation;
use crate::git;
use crate::state;
use crate::trailer::{self, key_for, Verdict, COUNTS_SHAPE};

pub fn skipped(gate: &str, paths: &[String]) {
    eprintln!(
        "git-agent-verdict: {gate}: skipped — no staged file matches {}",
        paths.join(", ")
    );
}

fn shape(inv: &Invocation) -> String {
    let key = key_for(&inv.gate);
    format!("{key}: reviewer=<id> {COUNTS_SHAPE} token=<issued>")
}

// The remedy is one command: the tool runs the review itself, so nothing here asks the author to brief anyone.
pub fn missing(inv: &Invocation, detail: &str) {
    eprintln!("\ngit-agent-verdict: {}: REVIEW GATE FAILED\n", inv.gate);
    eprintln!("  missing: {detail}");
    eprintln!("  wanted:  {}\n", shape(inv));
    eprintln!("  git agent-verdict attest --intent \"<the aim, one flat line>\"\n");
    eprintln!("  - runs each gate in turn, records what the reviewer reported");
    eprintln!("  - commits once every gate is attested");
    eprintln!("  - composes the message from --intent; this message file is discarded");
}

pub fn circular(gate: &str, rubrics: &[String]) {
    eprintln!("\ngit-agent-verdict: {gate}: RUBRIC IS STAGED\n");
    eprintln!("  {}\n", rubrics.join("\n  "));
    eprintln!("  circular: the {gate} review judges against a measure this commit changes");
    eprintln!("  - stage the rubric alone and attest: {gate} stands aside, and any gate whose");
    eprintln!("    --path reaches it still reviews it");
    eprintln!("  - every other change stays in its own commit, and still needs its review");
}

// Not a refusal: its measure is the whole of what is staged, so there is no other change for this gate to judge and nothing to separate the rubric from.
pub fn abstained(gate: &str, rubrics: &[String]) {
    eprintln!(
        "git-agent-verdict: {gate}: stood aside — the commit is only its measure ({})",
        rubrics.join(", ")
    );
}

fn summarize(verdicts: &[Verdict]) -> String {
    trailer::total(verdicts).render()
}

pub fn attested(gate: &str, count: usize, verdicts: &[Verdict]) {
    eprintln!(
        "git-agent-verdict: {gate}: attested ({count} verdict(s), {})",
        summarize(verdicts)
    );
}

pub fn blocked(gate: &str, major: u32) {
    eprintln!("\ngit-agent-verdict: {gate}: DECLARED BLOCKER");
    eprintln!("  major={major} (must be 0)");
    eprintln!("  - fix what the review named; the remedy is yours");
    eprintln!("  - the gate reopens only on a fresh attest of {gate}");
}

// A trailer whose token names no entry is the one thing a well-formed forgery looks like, so it says what it is rather than what is malformed.
pub fn untraceable(gate: &str, token: &str) {
    eprintln!("\ngit-agent-verdict: {gate}: UNKNOWN TOKEN\n");
    eprintln!("  token={token} matches no review recorded for this HEAD");
    eprintln!("  - run: git agent-verdict attest --intent \"…\"");
    eprintln!("    it reviews what is left, then commits with trailers it can trace");
}

pub fn mismatch(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: {gate}: TRAILER CONTRADICTS THE REVIEW\n  {detail}");
}

// The install command is in the line: the reader is a hook's stderr, and an agent told only that the binary is wrong will otherwise invent one.
pub fn stale(want: &str, have: &str) {
    eprintln!(
        "git-agent-verdict: {have} is older than the required {want}: cargo install git-agent-verdict --version '^{want}'"
    );
}

// Named apart from stale because the remedy is the opposite one: the binary is ahead, and what a hook declares was written against a grammar this release no longer speaks.
pub fn incompatible(want: &str, have: &str) {
    eprintln!(
        "git-agent-verdict: {have} is not the {want} line this hook declares its gates against: cargo install git-agent-verdict --version '^{want}'"
    );
}

pub fn malformed(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: {gate}: MALFORMED TRAILER\n  {detail}");
}

// Outside the repo and outside the diary: the diary is dropped the moment HEAD moves, which is exactly when an author wants to re-read what the review said about the commit that just landed.
fn log_path(gate: &str) -> Result<std::path::PathBuf, String> {
    let root = git::toplevel()?;
    let name = std::path::Path::new(&root)
        .file_name()
        .map_or_else(|| "repo".to_string(), |n| n.to_string_lossy().into_owned());
    let slug = format!("{name}-{}", &state::fingerprint(&root)[..8]);
    let home = std::env::var("HOME").map_err(|_| "no HOME to write the review log under")?;
    let dir = std::path::Path::new(&home)
        .join(".agent-verdicts")
        .join(slug);
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let head = git::head_sha();
    let mut n = 1;
    while dir.join(format!("{head}-{n}-{gate}.log")).exists() {
        n += 1;
    }
    Ok(dir.join(format!("{head}-{n}-{gate}.log")))
}

pub fn logged(gate: &str, findings: &str) -> Option<std::path::PathBuf> {
    let path = log_path(gate).ok()?;
    std::fs::write(&path, findings).ok()?;
    Some(path)
}

pub fn reviewing(gate: &str) {
    eprintln!("git-agent-verdict: {gate}: reviewing…");
}

pub fn reviewed(
    gate: &str,
    verdicts: &[Verdict],
    blocked: bool,
    next: Option<&str>,
    findings: &str,
) {
    // The verdict on stdout, the report on disk: a review runs to hundreds of lines, and an author reading the tail of a stream misses the findings above it.
    println!("{gate}: {}", summarize(verdicts));
    if !findings.is_empty() {
        match logged(gate, findings) {
            Some(path) => eprintln!("\nsee the full report: {}", path.display()),
            None => eprintln!("\n{findings}"),
        }
    }
    eprintln!();
    if blocked {
        eprintln!("MAJOR — gate not passed. Fix what the review named, then attest again.");
        return;
    }
    match next {
        Some(gate) => eprintln!("next: address the findings, then attest again for the {gate} gate"),
        None => eprintln!(
            "next: attest again, same intent — no gate left, so that run writes the trailers and commits"
        ),
    }
}

// Said on stdout, beside the verdicts: an agent reading that channel would otherwise have to infer a landed commit from git's own output, and the run that follows a guess is a second commit.
pub fn committed(trailers: &[String], out: &str) {
    println!(
        "committed {} — every gate attested, nothing left to run",
        git::head_sha()
    );
    print!("{out}");
    if trailers.is_empty() {
        eprintln!("  no verdict: no gate read this commit — see above for which, and why");
        return;
    }
    // The counts reach the message from the diary rather than from whoever read the review, and nothing in between could have retyped them.
    for line in trailers {
        eprintln!("  {line}");
    }
}

// A staged path no gate read, and which of the two reasons it was.
pub struct Unread {
    pub file: String,
    // The gate that covers it and is judged by it, where one does: it stood aside rather than measuring a change to its own measure.
    pub judged_by: Option<String>,
}

// A trailer says what a gate read, never what it did not: without this, the paths nothing reached look covered by the verdicts beside them.
pub fn unreviewed(unread: &[Unread]) {
    if unread.is_empty() {
        return;
    }
    let width = unread.iter().map(|u| u.file.len()).max().unwrap_or(0);
    eprintln!("\ngit-agent-verdict: LANDING UNREVIEWED\n");
    for u in unread {
        let why = match &u.judged_by {
            Some(gate) => format!("{gate} is judged by it, and stood aside"),
            None => "no gate's --path reaches it".to_string(),
        };
        eprintln!("  {:width$}  {why}", u.file);
    }
    if unread.iter().any(|u| u.judged_by.is_some()) {
        eprintln!("\n  Standing aside is the design: a gate cannot measure a change to its own");
        eprintln!("  measure. Another gate's --path may still cover it — none here does.");
    }
    if unread.iter().any(|u| u.judged_by.is_none()) {
        eprintln!("\n  A path nothing reaches is wiring: widen a --path, or accept that this");
        eprintln!("  commit attests nothing about it.");
    }
}

// Named before the commit, not after: the trailer about to be written records what its reviewer saw, and this is every gate for which that is no longer what lands.
pub fn moved(gates: &[String]) {
    if gates.is_empty() {
        return;
    }
    eprintln!("\ngit-agent-verdict: CONTENT MOVED SINCE ITS VERDICT\n");
    eprintln!("  {}\n", gates.join("\n  "));
    eprintln!("  - fixing what a review named does not re-open its gate, and nothing recounts");
    eprintln!("  - a bound the reviewer enforces can be broken by the fix and land unchecked");
    eprintln!("  - for a fresh review: git agent-verdict reset \"<why>\"");
}

pub fn reset_done(count: u32, reason: &str) {
    eprintln!("git-agent-verdict: review state cleared (reset {count} for this HEAD): {reason}");
    eprintln!("  the reason is recorded and reaches the commit message");
}
