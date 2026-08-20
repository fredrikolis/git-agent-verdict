// Concern: everything the tool prints — the skip line, each rejection, and what a review narrates as it runs | Non-concern: deciding whether a gate passed | IO: (gate, reason) -> stderr

use crate::cli::Invocation;
use crate::git;
use crate::state;
use crate::trailer::{self, Verdict};

pub fn skipped(gate: &str, paths: &[String]) {
    eprintln!(
        "git-agent-verdict: {gate}: skipped — no staged file matches {}",
        paths.join(", ")
    );
}

// Named here rather than left as a placeholder, and safe to paste: git ran this hook in the tree being committed, so the root under it is not a shell's guess about where it is.
fn here() -> String {
    git::toplevel().unwrap_or_else(|_| "<abs path to the repo root>".to_string())
}

// The trailer's grammar is not the caller's problem: it carries a token no one can write by hand, so showing the shape of a line only this tool can produce teaches nothing. The remedy is the tool.
pub fn missing(inv: &Invocation, _detail: &str) {
    eprintln!(
        "\ngit-agent-verdict: error: {}: no reviewable trailer",
        inv.gate
    );
    if speaks_for_the_hook(&inv.gate) {
        flow();
    }
}

pub fn repo_root() -> String {
    here()
}

fn summarize(verdicts: &[Verdict]) -> String {
    trailer::total(verdicts).render()
}

pub fn stale(want: &str, have: &str) {
    eprintln!(
        "git-agent-verdict: error: {have} is older than the required {want}: cargo install git-agent-verdict --version '^{want}'"
    );
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
    eprintln!("  - address the MAJOR findings this review reported");
    eprintln!("  - the gate passes only after a subsequent attest of {gate}");
}

// A trailer whose token names no entry is the one thing a well-formed forgery looks like, so it says what it is rather than what is malformed.
pub fn untraceable(gate: &str, token: &str) {
    eprintln!("\ngit-agent-verdict: error: {gate}: unknown token\n");
    eprintln!("  token={token} matches no review recorded for this HEAD");
    flow();
}

pub fn mismatch(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: error: {gate}: trailer contradicts the review\n  {detail}");
}

// The hook runs this binary once per gate, so a commit reaching two of them refuses twice, and the standing instruction is the same both times. The first gate the hook declares is the one that says it: a fact both processes read from the same hook, with nothing recorded between them.
fn speaks_for_the_hook(gate: &str) -> bool {
    crate::declarations::read()
        .ok()
        .and_then(|hook| hook.gates.first().map(|first| first.gate == gate))
        .unwrap_or(true)
}

// Backticked and counted, because a bare list of names reads as prose: an agent has to be told these are the gates, not words in a sentence.
fn gate_names() -> String {
    crate::declarations::read()
        .map(|hook| {
            let named: Vec<String> = hook.gates.iter().map(|g| format!("`{}`", g.gate)).collect();
            match named.len() {
                0 => "the gates it declares".to_string(),
                1 => format!("1 gate, {}", named[0]),
                n => format!("{n} gates, {}", named.join(" then ")),
            }
        })
        .unwrap_or_else(|_| "the gates it declares".to_string())
}

// One paragraph, printed where a caller meets this tool for the first time: what the repo demands, the command that satisfies it, and how the loop ends.
pub fn flow() {
    let here = here();
    eprintln!(
        "\nThis repository mandates `git agent-verdict` for all commits. To commit:\n\n  \
         git agent-verdict attest --repo {here} --intent \"<intent: one line, at most {} characters>\"\n\n\
         That reviews this repository's {}, in declaration order, halting at the first MAJOR. \
         Every MAJOR and MODERATE finding is a required fix; MINOR is at your discretion. Once \
         they are fixed, run attest again with no --intent (a MAJOR requires the re-review; a \
         MODERATE does not), until every gate has passed. Then:\n\n  \
         git agent-verdict commit --repo {here}",
        crate::cli::INTENT_LIMIT,
        gate_names()
    );
}

pub fn incompatible(want: &str, have: &str) {
    eprintln!(
        "git-agent-verdict: error: {have} is not the {want} line this hook declares its gates against: cargo install git-agent-verdict --version '^{want}'"
    );
}

pub fn malformed(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: error: {gate}: malformed trailer\n  {detail}");
}

// Outside the repo and outside the diary, which is dropped the moment HEAD moves — exactly when an author wants to re-read what the review said about the commit that just landed. One directory per repo, one per commit inside it, so everything reviewed since the last reset is listed together.
pub fn verdicts_dir() -> Result<std::path::PathBuf, String> {
    let root = git::toplevel()?;
    let name = std::path::Path::new(&root)
        .file_name()
        .map_or_else(|| "repo".to_string(), |n| n.to_string_lossy().into_owned());
    let slug = format!("{name}-{}", &state::fingerprint(&root)[..8]);
    let home = std::env::var("HOME").map_err(|_| "no HOME to write the review log under")?;
    Ok(std::path::Path::new(&home)
        .join(".agent-verdicts")
        .join(slug))
}

// The verdict is the log's first line, so the file says whose findings these are and what they came to. Numbered in the order the gates ran, which is the order an author reads them back in.
pub fn logged(
    at: &std::path::Path,
    gate: &str,
    verdicts: &[Verdict],
    findings: &str,
) -> Option<std::path::PathBuf> {
    let path = at.join(format!("{}-{gate}.log", next_log(at)));
    std::fs::write(
        &path,
        format!("{gate}: {}\n{findings}", summarize(verdicts)),
    )
    .ok()?;
    Some(path)
}

// One number per gate, counted from the findings files: the sidecar carrying that gate's narration takes the same one.
pub fn next_log(at: &std::path::Path) -> usize {
    std::fs::read_dir(at)
        .map(|entries| {
            entries
                .flatten()
                .filter(|e| numbered(&e.path()).is_some() && !stdout_log(&e.path()))
                .count()
                + 1
        })
        .unwrap_or(1)
}

fn stdout_log(path: &std::path::Path) -> bool {
    path.to_string_lossy().ends_with(".stdout.log")
}

fn numbered(path: &std::path::Path) -> Option<usize> {
    path.file_name()?.to_str()?.split_once('-')?.0.parse().ok()
}

// Named inline: this is a routine class of commit, and the reader needs which files and what to do, not the argument for it.
pub fn maintenance(files: &[String]) {
    eprintln!(
        "git-agent-verdict: error: {} cannot be attested: these files define the gates and the criteria this tool applies.",
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
    eprintln!("The reviewer reads the working tree; the commit records the index. Stage or restore the listed files.");
}

pub fn judging() {
    eprintln!("git-agent-verdict: validating the intent");
}

// The agent's own transcript, and the one command that reads it: a caller asking whether the review is still going gets an answer for one line of output, and pays nothing while it does not ask. Handed the command rather than a digest rendered here — a format invented here is a format maintained here for ever, and the transcript belongs to the agent.
const LATEST: &str = r#"jq -rc 'select(.type=="assistant") | .timestamp[11:19] as $t | .message.content[] | if .type=="tool_use" then "\($t) \(.name) \(.input|tostring|gsub("\n";" "))" elif .type=="text" then "\($t) » \(.text|gsub("\n";" "))" else empty end'"#;

// Said before the reviewer is spawned rather than after it answers, because a run that is killed never reaches the after: which gate was in play, which session to read, and the command that reads it all survive the kill.
pub fn reviewing(gate: &str, session: &str, transcript: Option<&std::path::Path>) {
    eprintln!(
        "git-agent-verdict: {gate}: reviewing — session {session}, pid {}",
        std::process::id()
    );
    let Some(path) = transcript else {
        return;
    };
    println!("progress log: {}", path.display());
    println!(
        "  latest activity: {LATEST} {} | cut -c1-110 | tail -5",
        path.display()
    );
}

// The elapsed time, which is the number that tells a kill apart from a hang: thirteen seconds and ten minutes read exactly alike in a shell that reports only that something died.
pub fn still_reviewing(elapsed: u64, ceiling: u64) {
    eprintln!("git-agent-verdict: still reviewing — {elapsed}s of {ceiling}s");
}

// Said out loud because it is evidence the author does not otherwise have: a marker left behind means the last run died mid-review, which nothing else in this tool would ever mention. The reviewer picked up here is the one that was already reading, not a second one paid for from the top.
pub fn resuming(gate: &str, session: &str, quiet_for: Option<u64>) {
    match quiet_for {
        Some(seconds) => eprintln!(
            "git-agent-verdict: {gate}: resuming session {session}, last wrote {seconds}s ago"
        ),
        None => eprintln!("git-agent-verdict: {gate}: resuming session {session}"),
    }
}

// Named at the top because the cost is the surprise: one full review per gate, against a tree nobody is changing.
pub fn auditing(hook: &str) {
    eprintln!("git-agent-verdict: auditing the tree against every gate {hook} declares");
    eprintln!("  one review per gate. Nothing recorded, nothing committed.");
}

// The same shape a review reports at a gate, minus what only a commit has: no token, no trailer, and no next gate to attest, because an audit is not driving anything to a commit.
pub fn audited(at: &std::path::Path, gate: &str, verdicts: &[Verdict], findings: &str) {
    println!("{gate}: {}", summarize(verdicts));
    match logged(at, gate, verdicts, findings) {
        Some(path) => eprintln!("  the full report: {}", path.display()),
        None => eprintln!("\n{findings}"),
    }
}

// Said where it happened and not only at the end: the gates after it are still to run, and a failure scrolled past ten minutes ago is one the reader has to go looking for.
pub fn gate_failed(gate: &str, detail: &str) {
    eprintln!("git-agent-verdict: error: {gate}: no verdict — {detail}");
}

// What to do with it, which is not "attest again": the findings are about code no commit is touching, so acting on them makes changes that are then attested in the ordinary way.
pub fn audit_done(gates: usize, blocked: bool, failed: &[String]) {
    let severity = if blocked {
        "including MAJOR"
    } else {
        "no MAJOR"
    };
    eprintln!("\ngit-agent-verdict: audited {gates} gate(s), {severity}");
    if !failed.is_empty() {
        eprintln!("  no verdict from: {}", failed.join(", "));
    }
    eprintln!("next: address the reported findings, in commits attested from their own diffs");
}

// Where each gate stands once a run is over, and why it is not in play when it is not. Nothing is under review by then, so there is no running state to show.
pub enum Standing {
    Passed(String),
    Blocked(String),
    Pending,
    Skipped(String),
}

// The whole board, every round: the count in play moves when a fix touches a file another gate's pathspec reaches, so a bare fraction would shrink and grow with nothing saying why.
pub fn gates(standings: &[(String, Standing)]) {
    let width = standings.iter().map(|(g, _)| g.len()).max().unwrap_or(0);
    eprintln!("\nagent-verdict gates declared by the commit-msg hook:");
    for (gate, standing) in standings {
        let said = match standing {
            Standing::Passed(counts) => format!("PASSED — {counts}"),
            Standing::Blocked(counts) => {
                format!("BLOCKED — {counts}")
            }
            Standing::Pending => "PENDING".to_string(),
            Standing::Skipped(paths) => format!("SKIPPED — no staged file matches {paths}"),
        };
        eprintln!("  {gate:width$}  [{said}]");
    }
    eprintln!();
}

pub fn reviewed(
    at: &std::path::Path,
    gate: &str,
    verdicts: &[Verdict],
    blocked: bool,
    next: Option<&str>,
    findings: &str,
    standings: &[(String, Standing)],
) {
    // The verdict on stdout, the report on disk: a review runs to hundreds of lines, and an author reading the tail of a stream misses the findings above it.
    println!("{gate}: {}", summarize(verdicts));
    // Written whether or not it found anything: a gate that reviewed and said nothing is still a gate that reviewed, and a listing missing it reads as one that never ran.
    match logged(at, gate, verdicts, findings) {
        Some(path) => eprintln!("\nfull report: {}", path.display()),
        None => eprintln!("\n{findings}"),
    }
    gates(standings);
    if blocked {
        eprintln!(
            "git-agent-verdict: error: MAJOR — gate not passed. Address the reported findings, then:\n  git agent-verdict attest --repo {}",
            here()
        );
        return;
    }
    match next {
        Some(gate) => {
            eprintln!("next: {gate}, in this same run");
        }
        None => eprintln!(
            "next: git agent-verdict commit --repo {} — all gates passed",
            here()
        ),
    }
}

// Said on stdout, beside the verdicts: an agent reading that channel would otherwise have to infer a landed commit from git's own output, and the run that follows a guess is a second commit.
pub fn committed(trailers: &[String], out: &str) {
    print!("{out}");
    if trailers.is_empty() {
        eprintln!("  no verdict: no declared gate matched this commit; see the preceding output");
        return;
    }
    // The counts reach the message from the diary rather than from whoever read the review, and nothing in between could have retyped them.
    for line in trailers {
        eprintln!("  {line}");
    }
}

pub fn reset_done(count: u32, reason: &str) {
    eprintln!("git-agent-verdict: {count} verdict(s) dropped: {reason}");
    eprintln!("  the reason is recorded in the commit message");
}

// Where the round went, said the moment it is running rather than when it answers: a caller that is killed a second later has already been told where to look.
pub fn started(started: &crate::round::Started) {
    eprintln!(
        "git-agent-verdict: spawned attestation process (pid {})",
        started.pid
    );
    println!("{}", started.at.display());
    println!(
        "Use `git agent-verdict await --repo {}` to wait for it.\n\
         Do not poll with pgrep, sleep or any combination of them: those guards match their own \
         shell and can stall for hours. If your harness interrupts the await, run it again.",
        here()
    );
}

pub fn awaited(at: &std::path::Path, said: Option<&str>) {
    match said {
        Some(said) => eprintln!("git-agent-verdict: {said}"),
        None => eprintln!("git-agent-verdict: no verdict recorded"),
    }
    left(at);
    // The command that follows from what it decided, because every run of this tool names the next one and a caller left holding a verdict has to work it out otherwise. A round whose commit has already landed is read back, not acted on: its directory is keyed on a parent that has moved.
    let landed = verdicts_dir()
        .map(|dir| dir.join(git::head_sha()) != at)
        .unwrap_or(false);
    if landed {
        return;
    }
    match said {
        Some("PASSED") => println!("git agent-verdict commit --repo {}", here()),
        Some("BLOCKED") => println!(
            "address the reported findings, then: git agent-verdict attest --repo {}",
            here()
        ),
        _ => {}
    }
}

// What the reviews wrote, for any verb that has just told the caller to act on it: the same listing await prints, because the question "what do I fix" has one answer and it should not depend on which verb asked.
pub fn what_was_reviewed() {
    if let Some(at) = crate::round::last_at() {
        left(&at);
    }
}

// Every gate reviewed since the last reset, in the order they ran, each line carrying its own verdict. The findings are in the files: a reader that needs one opens it, and a reader scanning for which gate to open should not have to page past sixty lines to reach the next.
fn left(at: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    // The sidecars are not listed: they carry a session id, a transcript path and whatever a crash printed, and none of that is what a gate asked to be fixed.
    let mut wrote: Vec<(usize, std::path::PathBuf)> = entries
        .flatten()
        .filter(|e| !stdout_log(&e.path()))
        .filter_map(|e| Some((numbered(&e.path())?, e.path())))
        .collect();
    if wrote.is_empty() {
        return;
    }
    wrote.sort();
    println!("{}/", at.display());
    for (_, path) in &wrote {
        let named = path.file_name().unwrap_or_default().to_string_lossy();
        let says = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| text.lines().next().map(str::to_string))
            .unwrap_or_default();
        println!("  {named}  # {says}");
    }
}

pub fn aborted(pid: u32, at: Option<&std::path::Path>, standings: &[(String, Standing)]) {
    eprintln!("git-agent-verdict: killed {pid}");
    gates(standings);
    if let Some(at) = at {
        left(at);
    }
}

pub fn nothing_to_abort() {
    eprintln!("git-agent-verdict: no review is running");
}

// Not an error: the caller asked for a review, there is none left to run, and the state is exactly what it should be.
pub fn all_passed() {
    eprintln!(
        "git-agent-verdict: all gates passed. Every outstanding MODERATE finding is a required fix (no re-review required). MINOR is at your discretion. Then:\n  git agent-verdict commit --repo {}",
        here()
    );
}

pub fn not_passed(gate: &str) -> String {
    format!(
        "{gate}: no passing verdict recorded\n  git agent-verdict attest --repo {}",
        here()
    )
}
