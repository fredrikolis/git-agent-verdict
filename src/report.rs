// Concern: everything the tool prints — the skip line, each rejection, and what `attest` narrates as it runs | Non-concern: deciding whether a gate passed | IO: (gate, reason) -> stderr

use crate::cli::Invocation;
use crate::declarations::Declaration;
use crate::git;
use crate::runner::{MARKER, REFUSED};
use crate::state;
use crate::trailer::{key_for, Counts, Verdict};

const TEMPLATE: &str = include_str!("prompt.md");
const TEMPLATE_SIMPLE: &str = include_str!("prompt-simple.md");
const PLACEHOLDER: &str = "<the aim of the change, stated flatly, as a spec would state it>";

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

fn shape_of(simple: bool) -> &'static str {
    if simple {
        "findings=<n>"
    } else {
        "major=<n> moderate=<n> minor=<n>"
    }
}

// Every field the runner must report, in the line it must report them on: what is not stated here cannot be demanded of it.
fn asked_of(simple: bool) -> String {
    format!(
        "reviewer=<who reviewed> session=<this review\'s id> {}",
        shape_of(simple)
    )
}

// The machine-read line is written here and nowhere else, so what the reviewer is asked for is the shape the runner parses.
fn verdict_spec(declaration: &Declaration, files: &[String]) -> String {
    let shape = asked_of(declaration.brief.simple);
    let refusal = format!(
        "\n\nIf you are refusing the brief, close with this instead, and review nothing:\n\n  {MARKER} {REFUSED}"
    );
    let listed = files
        .iter()
        .map(|f| format!("  {f}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "The files under review, and no others:\n{listed}\n\nClose with exactly this line, and nothing after it:\n\n  {MARKER} {shape}{refusal}"
    )
}

pub fn prompt(
    declaration: &Declaration,
    intent: Option<&str>,
    files: &[String],
) -> Result<String, String> {
    let template = match &declaration.brief.prompt {
        Some(path) => {
            std::fs::read_to_string(path).map_err(|e| format!("--override-prompt {path}: {e}"))?
        }
        None if declaration.brief.simple => built_in(TEMPLATE_SIMPLE),
        None => built_in(TEMPLATE),
    };
    let docs = declaration
        .docs
        .iter()
        .map(|d| format!("  {d}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(template
        .replace("{{gate}}", &declaration.gate)
        .replace("{{docs}}", &docs)
        .replace("{{intent}}", intent.unwrap_or(PLACEHOLDER))
        .replace("{{verdict}}", &verdict_spec(declaration, files)))
}

fn shape(inv: &Invocation) -> String {
    let key = key_for(&inv.gate);
    let counts = shape_of(inv.brief.simple);
    format!("  {key}: reviewer=<id> {counts} token=<issued>")
}

// The remedy is one command: the reviewer is never briefed by hand any more, so nothing here is forwarded anywhere.
pub fn missing(inv: &Invocation, detail: &str) {
    eprintln!("\ngit-agent-verdict: {}: REVIEW GATE FAILED\n", inv.gate);
    eprintln!("MISSING — {detail}\n");
    eprintln!("{}\n", shape(inv));
    eprintln!("Earned by a review this tool runs for you:\n");
    eprintln!(
        "  git agent-verdict attest --intent \"<the aim of the change, in one flat line>\"\n"
    );
    eprintln!("It runs the next gate, records what the reviewer reported, and hands back the");
    eprintln!("trailer to paste. Trailers must be the LAST paragraph of the message.");
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

pub fn summarize(verdicts: &[Verdict]) -> String {
    let (mut major, mut moderate, mut minor, mut findings) = (0, 0, 0, 0);
    for verdict in verdicts {
        match verdict.counts {
            Counts::Graded {
                major: a,
                moderate: b,
                minor: c,
            } => {
                major += a;
                moderate += b;
                minor += c;
            }
            Counts::Advisory { findings: n } => findings += n,
        }
    }
    match verdicts.first().map(|v| v.counts) {
        Some(Counts::Advisory { .. }) => format!("findings={findings}"),
        _ => format!("major={major} moderate={moderate} minor={minor}"),
    }
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
    eprintln!("\nThe review named what is wrong. How it gets fixed is yours to decide; this gate");
    eprintln!("reopens only when the same gate is attested again.");
}

// A trailer whose token names no entry is the one thing a well-formed forgery looks like, so it says what it is rather than what is malformed.
pub fn untraceable(gate: &str, token: &str) {
    eprintln!("\ngit-agent-verdict: {gate}: UNKNOWN TOKEN\n");
    eprintln!("  token={token} matches no review recorded for this HEAD.");
    eprintln!("\nRun `git agent-verdict attest --intent \"…\"` and paste the trailer it returns.");
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
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_string());
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
        eprintln!(
            "MAJOR — this gate is not passed. Fix what the review named, then run attest again."
        );
        return;
    }
    match next {
        Some(gate) => {
            eprintln!("Address what it found, then run attest again for the {gate} gate.")
        }
        None => eprintln!("Address what it found, then run attest again for the trailers."),
    }
}

// The counts reach the message from the diary rather than from whoever read the review, and nothing in between could have retyped them.
pub fn committed(trailers: &[String], out: &str) {
    eprintln!("\ngit-agent-verdict: every gate attested — committed.\n");
    for line in trailers {
        eprintln!("  {line}");
    }
    print!("{out}");
}

pub fn reset_done(count: u32, reason: &str) {
    eprintln!("git-agent-verdict: review state cleared (reset {count} for this HEAD): {reason}");
    eprintln!("The reason is recorded and travels into the commit message.");
}
