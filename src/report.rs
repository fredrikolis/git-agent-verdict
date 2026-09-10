// Concern: everything the tool prints — the skip line, each rejection, and what a review narrates as it runs | Non-concern: deciding whether a gate passed | IO: (gate, reason) -> stderr

use crate::cli::Invocation;
use crate::git;
use crate::state;
use crate::trailer::{self, Verdict};

pub fn coauthor_stripped(line: &str) {
    eprintln!(
        "git-agent-verdict: removed \"{line}\". This tool does not allow AI agent commit trailers. \
         Amending this commit to restore it can break your organization's audit tooling."
    );
}

pub fn skipped(gate: &str, paths: &[String]) {
    eprintln!(
        "git-agent-verdict: {gate}: skipped — no staged file matches {}",
        paths.join(", ")
    );
}

// git ran this hook in the tree being committed, so the fallback is never a shell's guess.
fn here() -> String {
    git::toplevel().unwrap_or_else(|_| "<abs path to the repo root>".to_string())
}

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

pub fn untraceable(gate: &str, token: &str) {
    eprintln!("\ngit-agent-verdict: error: {gate}: unknown token\n");
    eprintln!("  token={token} matches no review recorded for this HEAD");
    flow();
}

pub fn mismatch(gate: &str, detail: &str) {
    eprintln!("\ngit-agent-verdict: error: {gate}: trailer contradicts the review\n  {detail}");
}

// A commit reaching two gates refuses twice; only the first gate the hook declares says the flow.
fn speaks_for_the_hook(gate: &str) -> bool {
    crate::declarations::read()
        .ok()
        .and_then(|hook| hook.gates.first().map(|first| first.gate == gate))
        .unwrap_or(true)
}

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

pub fn flow() {
    let here = here();
    eprintln!(
        "\nThis repository mandates `git agent-verdict` for all commits. To commit:\n\n  \
         git agent-verdict attest --repo {here} --intent \"<intent: one line, at most {} characters>\"\n\n\
         That reviews this repository's {}, in declaration order, halting at the first MAJOR. Each \
         gate reviews the staged change under its own paths, and is skipped where nothing is \
         staged under them. \
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

// Outside the repo: the diary is dropped the moment HEAD moves, right when an author wants to re-read it.
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

// The verdict is the log's first line: callers read that line back without opening the file.
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

pub fn maintenance(files: &[String]) {
    eprintln!(
        "git-agent-verdict: error: {} cannot be attested: these files define the gates and the criteria this tool applies.",
        files.join(", ")
    );
    eprintln!("Review manually and commit with --no-verify.");
}

// A reviewer opens the working tree, so drift means it read text the commit will not actually carry.
pub fn drifted(files: &[String]) {
    eprintln!(
        "git-agent-verdict: error: staged at one version and edited since: {}.",
        files.join(", ")
    );
    eprintln!(
        "A round reviews the staged change. The reviewer also opens the working tree, which here \
         holds text the commit will not carry, and the usual cause is a fix that was never staged.\n\
         Stage those edits with `git add`, or, if the staged version is the one to review:\n\n  \
         git agent-verdict attest --repo {} {}",
        here(),
        crate::cli::STAGED_ONLY
    );
}

pub fn judging() {
    eprintln!("git-agent-verdict: validating the intent");
}

// A command, not a rendered digest: a format invented here is a format maintained here forever.
const LATEST: &str = r#"jq -rc 'select(.type=="assistant") | .timestamp[11:19] as $t | .message.content[] | if .type=="tool_use" then "\($t) \(.name) \(.input|tostring|gsub("\n";" "))" elif .type=="text" then "\($t) » \(.text|gsub("\n";" "))" else empty end'"#;

// Said before the reviewer runs, not after: a killed run never reaches an "after".
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

pub fn still_reviewing(elapsed: u64, ceiling: u64) {
    eprintln!("git-agent-verdict: still reviewing — {elapsed}s of {ceiling}s");
}

// Resuming the session that was already reading, not paying for a second one from the top.
pub fn resuming(gate: &str, session: &str, quiet_for: Option<u64>) {
    match quiet_for {
        Some(seconds) => eprintln!(
            "git-agent-verdict: {gate}: resuming session {session}, last wrote {seconds}s ago"
        ),
        None => eprintln!("git-agent-verdict: {gate}: resuming session {session}"),
    }
}

pub fn auditing(hook: &str) {
    eprintln!("git-agent-verdict: auditing the tree against every gate {hook} declares");
    eprintln!("  one review per gate. Nothing recorded, nothing committed.");
}

pub fn audited(at: &std::path::Path, gate: &str, verdicts: &[Verdict], findings: &str) {
    println!("{gate}: {}", summarize(verdicts));
    match logged(at, gate, verdicts, findings) {
        Some(path) => eprintln!("  the full report: {}", path.display()),
        None => eprintln!("\n{findings}"),
    }
}

pub fn gate_failed(gate: &str, detail: &str) {
    eprintln!("git-agent-verdict: error: {gate}: no verdict — {detail}");
}

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

pub enum Standing {
    Passed(String),
    Blocked(String),
    Pending,
    Skipped(String),
}

// The gate count can change between runs: a fix can touch a file another gate's pathspec reaches.
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
    println!("{gate}: {}", summarize(verdicts));
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

pub fn committed(trailers: &[String], out: &str) {
    print!("{out}");
    if trailers.is_empty() {
        eprintln!("  no verdict: no declared gate matched this commit; see the preceding output");
        return;
    }
    for line in trailers {
        eprintln!("  {line}");
    }
}

pub fn reset_done(count: u32, reason: &str) {
    eprintln!("git-agent-verdict: {count} verdict(s) dropped: {reason}");
    eprintln!("  the reason is recorded in the commit message");
}

pub fn started(started: &crate::round::Started) {
    eprintln!(
        "git-agent-verdict: spawned attestation process (pid {})",
        started.pid
    );
    println!("{}", started.at.display());
    println!(
        "This round reviews what is staged now.\n\
         Use `git agent-verdict await --repo {}` to wait for it.\n\
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
    // A landed round is read back, not acted on: its directory is keyed on a parent that has moved.
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

pub fn what_was_reviewed() {
    if let Some(at) = crate::round::last_at() {
        left(&at);
    }
}

fn left(at: &std::path::Path) {
    let Ok(entries) = std::fs::read_dir(at) else {
        return;
    };
    // Sidecars carry a session id and transcript path, not findings, so they're excluded.
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
