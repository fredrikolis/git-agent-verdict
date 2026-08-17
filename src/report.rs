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

// Named here rather than left as a placeholder, and safe to paste: git ran this hook in the tree being committed, so the root under it is not a shell's guess about where it is.
fn here() -> String {
    git::toplevel().unwrap_or_else(|_| "<abs path to the repo root>".to_string())
}

// The remedy is one command: the tool runs the review itself, so nothing here asks the author to brief anyone.
pub fn missing(inv: &Invocation, detail: &str) {
    eprintln!(
        "\ngit-agent-verdict: error: {}: no reviewable trailer\n",
        inv.gate
    );
    eprintln!("  missing: {detail}");
    eprintln!("  wanted:  {}\n", shape(inv));
    eprintln!(
        "  git agent-verdict attest --repo {} --intent \"<the aim, one flat line>\"\n",
        here()
    );
    eprintln!("  - runs each gate in turn, records what the reviewer reported");
    eprintln!("  - commits once every gate is attested");
    eprintln!("  - composes the message from --intent; this message file is discarded");
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
    eprintln!("\ngit-agent-verdict: error: {gate}: declared blocker");
    eprintln!("  major={major} (must be 0)");
    eprintln!("  - fix what the review named; the remedy is yours");
    eprintln!("  - the gate reopens only on a fresh attest of {gate}");
}

// A trailer whose token names no entry is the one thing a well-formed forgery looks like, so it says what it is rather than what is malformed.
pub fn untraceable(gate: &str, token: &str) {
    eprintln!("\ngit-agent-verdict: error: {gate}: unknown token\n");
    eprintln!("  token={token} matches no review recorded for this HEAD");
    eprintln!(
        "  - run: git agent-verdict attest --repo {} --intent \"…\"",
        here()
    );
    eprintln!("    it reviews what is left, then commits with trailers it can trace");
}

pub fn mismatch(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: error: {gate}: trailer contradicts the review\n  {detail}");
}

// The install command is in the line: the reader is a hook's stderr, and an agent told only that the binary is wrong will otherwise invent one.
pub fn stale(want: &str, have: &str) {
    eprintln!(
        "git-agent-verdict: error: {have} is older than the required {want}: cargo install git-agent-verdict --version '^{want}'"
    );
}

// Named apart from stale because the remedy is the opposite one: the binary is ahead, and what a hook declares was written against a grammar this release no longer speaks.
pub fn incompatible(want: &str, have: &str) {
    eprintln!(
        "git-agent-verdict: error: {have} is not the {want} line this hook declares its gates against: cargo install git-agent-verdict --version '^{want}'"
    );
}

pub fn malformed(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: error: {gate}: malformed trailer\n  {detail}");
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

// Named inline: this is a routine class of commit, and the reader needs which files and what to do, not the argument for it.
pub fn maintenance(files: &[String]) {
    eprintln!(
        "git-agent-verdict: error: {} can never be attested — part of the rubric and infra this tool scores by.",
        files.join(", ")
    );
    eprintln!("Review manually and commit with --no-verify.");
}

// The verdict says the staged content was reviewed. A reviewer opens files to read them in context, so where the worktree and the index disagree it reviewed something the commit will not carry, and the trailer would say otherwise.
pub fn drifted(files: &[String]) {
    eprintln!(
        "git-agent-verdict: error: the index and the working tree disagree on {}.",
        files.join(", ")
    );
    eprintln!("The reviewer opens the files; the commit carries the index. Stage or restore them.");
}

pub fn judging() {
    eprintln!("git-agent-verdict: judging the intent…");
}

// A claim with nothing running under it is this tool's one record that the last run did not end on its own terms, and clearing it in silence throws that away. What it reports is when the repo was claimed, not how long the run lasted: the claim records a start and nothing else, and it is read whenever the next run happens along — an hour later, that difference is an hour of running the tool would be inventing. What bounds the end is the reviewer's own last write, which `resuming` reports.
pub fn abandoned(pid: &str, ago: u64) {
    eprintln!(
        "git-agent-verdict: the previous attest (pid {pid}) did not finish — it claimed this repo {ago}s ago. Taking its claim."
    );
}

// Said before the reviewer is spawned rather than after it answers, because a run that is killed never reaches the after. Without this line the shell holding a dead run has nothing saying which gate was in play or which session to go and read; with it, the two facts survive the kill.
pub fn reviewing(gate: &str, session: &str) {
    eprintln!(
        "git-agent-verdict: {gate}: reviewing — session {session}, pid {}",
        std::process::id()
    );
}

// The elapsed time, which is the number that tells a kill apart from a hang: thirteen seconds and ten minutes read exactly alike in a shell that reports only that something died.
pub fn still_reviewing(elapsed: u64, ceiling: u64) {
    eprintln!("git-agent-verdict: still reviewing — {elapsed}s of {ceiling}s");
}

// Said out loud because it is evidence the author does not otherwise have: a marker left behind means the last run died mid-review, which nothing else in this tool would ever mention. The reviewer picked up here is the one that was already reading, not a second one paid for from the top.
pub fn resuming(gate: &str, session: &str, quiet_for: Option<u64>) {
    eprintln!("git-agent-verdict: {gate}: the last run was cut short mid-review — resuming its reviewer, session {session}");
    // The one measured fact about the end: everything else here is inferred from a marker that was written before the round began.
    if let Some(seconds) = quiet_for {
        eprintln!("git-agent-verdict: {gate}: that reviewer last wrote {seconds}s ago");
    }
}

// Where each gate stands once a run is over, and why it is not in play when it is not. Nothing is under review by then, so there is no running state to show.
pub enum Standing {
    Passed(String),
    Blocked(String),
    Waiting,
    Skipped(String),
}

// The whole board, every round: the count in play moves when a fix touches a file another gate's pathspec reaches, so a bare fraction would shrink and grow with nothing saying why.
pub fn gates(standings: &[(String, Standing)]) {
    let width = standings.iter().map(|(g, _)| g.len()).max().unwrap_or(0);
    eprintln!("\nagent-verdict gates mandated by repo:");
    for (gate, standing) in standings {
        let said = match standing {
            Standing::Passed(counts) => format!("PASSED - {counts}"),
            Standing::Blocked(counts) => format!("BLOCKED - {counts}, fix and attest again"),
            Standing::Waiting => "PENDING".to_string(),
            Standing::Skipped(paths) => format!("SKIPPED - nothing staged matches {paths}"),
        };
        eprintln!("  {gate:width$}  [{said}]");
    }
    eprintln!();
}

pub fn reviewed(
    gate: &str,
    verdicts: &[Verdict],
    blocked: bool,
    next: Option<&str>,
    findings: &str,
    standings: &[(String, Standing)],
) {
    // The verdict on stdout, the report on disk: a review runs to hundreds of lines, and an author reading the tail of a stream misses the findings above it.
    println!("{gate}: {}", summarize(verdicts));
    if !findings.is_empty() {
        match logged(gate, findings) {
            Some(path) => eprintln!("\nsee the full report: {}", path.display()),
            None => eprintln!("\n{findings}"),
        }
    }
    gates(standings);
    if blocked {
        eprintln!("git-agent-verdict: error: MAJOR — gate not passed. Fix what the review named, then attest again.");
        return;
    }
    match next {
        Some(gate) => {
            eprintln!("next: address the findings, then attest again for {gate}");
        }
        None => eprintln!("next: attest again — every gate is through, so that run commits"),
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

pub fn reset_done(count: u32, reason: &str) {
    eprintln!("git-agent-verdict: review state cleared (reset {count} for this HEAD): {reason}");
    eprintln!("  the reason is recorded and reaches the commit message");
}
