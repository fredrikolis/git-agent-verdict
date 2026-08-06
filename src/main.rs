// Concern: the invocation grammar, and the decision for one gate or for the rubric preflight | Non-concern: the trailer grammar, or the wording of a rejection | IO: (argv, message file) -> exit status

mod git;
mod report;
mod trailer;

use std::process::ExitCode;

const USAGE: &str = concat!(
    "usage: git-agent-verdict <msg-file> <gate> [--per-file] [--simple] [--override-prompt <path>]\n",
    "                         --doc <path>... --path <pathspec>...\n",
    "       git-agent-verdict --rubric-guard --doc <path>...\n",
    "       git-agent-verdict --reviewer-prompt <gate>\n",
    "       git-agent-verdict --check-min-version <version>"
);

const GUARD_LABEL: &str = "rubric-guard";

// Set while the hook is re-run to enumerate itself: every gate prints its declaration and exits instead of validating, so the doc list is read from the hook rather than retyped beside it.
const LIST_ENV: &str = "GIT_AGENT_VERDICT_LIST";

// How a gate briefs its reviewer: which ladder it grades against, whose template it reads. Held apart because --reviewer-prompt has one without a message, a pathspec or a decision.
#[derive(Default)]
pub struct Brief {
    pub simple: bool,
    pub prompt: Option<String>,
}

pub struct Invocation {
    pub msg_file: String,
    pub gate: String,
    pub per_file: bool,
    pub docs: Vec<String>,
    pub paths: Vec<String>,
    pub brief: Brief,
}

// The preflight needs neither the message nor a gate, so it is a flag-only mode rather than a gate that ignores half its arguments.
enum Mode {
    Gate(Invocation),
    RubricGuard(Vec<String>),
    ReviewerPrompt(String),
    MinVersion(String),
}

// Same reason as a doc: the reviewer block promises absolute paths, and a mistyped override would otherwise fall back to the built-in template without saying so.
fn canonical(path: &str) -> Result<String, String> {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("--override-prompt {path}: {e}"))
}

// Resolved once, here: the reviewer block promises absolute paths, and an unresolvable doc would silently exempt itself from the rubric guards.
fn canonical_docs(docs: Vec<String>) -> Result<Vec<String>, String> {
    docs.into_iter()
        .map(|d| {
            std::fs::canonicalize(&d)
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|e| format!("--doc {d}: {e}"))
        })
        .collect()
}

// Every list is a repeated singular flag: no variadic can absorb the token meant for its neighbour.
fn parse(args: impl Iterator<Item = String>) -> Result<Mode, String> {
    let mut positional = Vec::new();
    let (mut guard, mut per_file) = (false, false);
    let mut reviewer_prompt: Option<String> = None;
    let mut min_version: Option<String> = None;
    let mut brief = Brief::default();
    let (mut docs, mut paths) = (Vec::new(), Vec::new());
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--rubric-guard" => guard = true,
            "--reviewer-prompt" => {
                reviewer_prompt = Some(args.next().ok_or("--reviewer-prompt needs a gate name")?)
            }
            "--check-min-version" => {
                min_version = Some(args.next().ok_or("--check-min-version needs a version")?)
            }
            "--simple" => brief.simple = true,
            "--override-prompt" => {
                let path = args.next().ok_or("--override-prompt needs a path")?;
                brief.prompt = Some(canonical(&path)?);
            }
            "--per-file" => per_file = true,
            "--doc" => docs.push(args.next().ok_or("--doc needs a path")?),
            "--path" => paths.push(args.next().ok_or("--path needs a pathspec")?),
            flag if flag.starts_with('-') => return Err(format!("unknown flag '{flag}'")),
            value => positional.push(value.to_string()),
        }
    }
    // Answered from the binary's own version alone, so it takes nothing else: a hook runs it before it asks the tool for anything, where there is no gate to speak of yet.
    if let Some(want) = min_version {
        let anything_else = guard
            || per_file
            || brief.simple
            || brief.prompt.is_some()
            || reviewer_prompt.is_some()
            || !docs.is_empty()
            || !paths.is_empty()
            || !positional.is_empty();
        if anything_else {
            return Err("--check-min-version takes a version only".to_string());
        }
        return Ok(Mode::MinVersion(want));
    }
    if let Some(gate) = reviewer_prompt {
        let gate_flags = per_file || brief.simple || brief.prompt.is_some();
        if guard || gate_flags || !docs.is_empty() || !paths.is_empty() || !positional.is_empty() {
            let detail = "--reviewer-prompt takes a gate name only: how the gate is briefed comes from the commit-msg hook";
            return Err(detail.to_string());
        }
        return Ok(Mode::ReviewerPrompt(gate));
    }
    // A preflight guarding nothing is a hook that has silently stopped guarding, so an empty list is an error in both modes rather than a vacuous pass.
    if docs.is_empty() {
        return Err("at least one --doc is required".to_string());
    }
    if guard {
        if per_file
            || brief.simple
            || brief.prompt.is_some()
            || !paths.is_empty()
            || !positional.is_empty()
        {
            let detail = "--rubric-guard reads the index alone: it demands no review, so no <msg-file>, <gate>, --path, --per-file, --simple or --override-prompt";
            return Err(detail.to_string());
        }
        return Ok(Mode::RubricGuard(canonical_docs(docs)?));
    }
    let [msg_file, gate] = <[String; 2]>::try_from(positional).map_err(|got| {
        format!(
            "expected <msg-file> and <gate>, got {} argument(s)",
            got.len()
        )
    })?;
    if paths.is_empty() {
        return Err("at least one --path is required".to_string());
    }
    Ok(Mode::Gate(Invocation {
        msg_file,
        gate,
        per_file,
        docs: canonical_docs(docs)?,
        paths,
        brief,
    }))
}

fn fields(version: &str, what: &str) -> Result<Vec<u32>, String> {
    version
        .split('.')
        .map(|f| {
            f.parse::<u32>()
                .map_err(|_| format!("{what} '{version}' is not a version like 0.2.0"))
        })
        .collect()
}

// A floor, not an equality: what must not arrive silently is a different reviewer brief, and that only happens when the floor is raised deliberately. An additive release passes.
fn min_version(want: &str) -> Result<bool, String> {
    let have = env!("CARGO_PKG_VERSION");
    let (floor, installed) = (
        fields(want, "--check-min-version")?,
        fields(have, "this binary's version")?,
    );
    let width = floor.len().max(installed.len());
    // Padded, because [0, 2] and [0, 2, 0] are one version and compare unequal as vectors.
    let padded = |mut v: Vec<u32>| {
        v.resize(width, 0);
        v
    };
    if padded(installed) < padded(floor) {
        report::stale(want, have);
        return Ok(false);
    }
    Ok(true)
}

fn per_file_gaps(inv: &Invocation, verdicts: &[trailer::Verdict]) -> Result<Vec<String>, String> {
    let attested: Vec<&str> = verdicts.iter().filter_map(|v| v.file.as_deref()).collect();
    Ok(git::staged_existing(&inv.paths)?
        .into_iter()
        .filter(|staged| !attested.contains(&staged.as_str()))
        .collect())
}

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
fn staged_rubrics(docs: &[String]) -> Result<Vec<String>, String> {
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
fn rubric_guard(docs: &[String]) -> Result<bool, String> {
    let rubrics = staged_rubrics(docs)?;
    if rubrics.is_empty() {
        return Ok(true);
    }
    report::preflight(&rubrics);
    Ok(false)
}

// Re-running the hook is what resolves `$KB/foo.md` and friends: the shell expands them, where reading the hook as text could not.
fn reviewer_prompt(want: &str) -> Result<bool, String> {
    let hook = git::hook_path()?;
    let out = std::process::Command::new(&hook)
        .arg("/dev/null")
        .env(LIST_ENV, "1")
        .output()
        .map_err(|e| format!("cannot run {hook}: {e}"))?;
    let listing = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut declared = Vec::new();
    for line in listing.lines() {
        let mut fields = line.split('\t');
        let Some(gate) = fields.next() else { continue };
        let mut brief = Brief::default();
        let mut docs = Vec::new();
        for field in fields {
            if let Some(doc) = field.strip_prefix("doc=") {
                docs.push(doc.to_string());
            } else if let Some(path) = field.strip_prefix("prompt=") {
                brief.prompt = Some(path.to_string());
            } else if field == "simple" {
                brief.simple = true;
            }
        }
        if docs.is_empty() {
            continue;
        }
        if gate == want {
            println!("{}", report::prompt(gate, &docs, &brief)?);
            return Ok(true);
        }
        declared.push(gate.to_string());
    }
    if declared.is_empty() {
        return Err(format!("{hook} declared no gates"));
    }
    Err(format!(
        "no gate '{want}' in {hook}; it declares: {}",
        declared.join(", ")
    ))
}

fn check(inv: &Invocation) -> Result<bool, String> {
    if std::env::var_os(LIST_ENV).is_some() {
        // Named fields, so a gate that declares no override still lists its docs unambiguously.
        let mut fields = vec![inv.gate.clone()];
        if inv.brief.simple {
            fields.push("simple".to_string());
        }
        if let Some(path) = &inv.brief.prompt {
            fields.push(format!("prompt={path}"));
        }
        fields.extend(inv.docs.iter().map(|d| format!("doc={d}")));
        println!("{}", fields.join("\t"));
        return Ok(true);
    }
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

    let raw = std::fs::read_to_string(&inv.msg_file)
        .map_err(|e| format!("cannot read {}: {e}", inv.msg_file))?;
    if auto_generated(&raw) {
        return Ok(true);
    }
    let raw = drop_agent_coauthor(&inv.msg_file, &raw)?;
    let block = git::trailers(&inv.msg_file)?;
    let verdicts = match trailer::parse_for(&inv.gate, &block) {
        Ok(verdicts) => verdicts,
        Err(detail) => {
            report::malformed(&inv.gate, &detail);
            return Ok(false);
        }
    };

    if verdicts.is_empty() {
        let detail = if trailer::present_but_unparsed(&inv.gate, &raw, &block) {
            "the trailer exists but is not in the message's trailing paragraph, so git does not see it"
        } else {
            "the message needs this trailer and has none"
        };
        report::missing(inv, detail)?;
        return Ok(false);
    }

    if inv.per_file {
        let gaps = per_file_gaps(inv, &verdicts)?;
        if !gaps.is_empty() {
            report::missing(inv, &format!("no trailer names: {}", gaps.join(", ")))?;
            return Ok(false);
        }
    }

    let major = verdicts.iter().map(|v| v.major).sum();
    // A simple gate demands the review and records it; what the review found is the author's to act on, so no count of it is a blocker.
    if !inv.brief.simple && verdicts.iter().any(trailer::Verdict::blocks) {
        report::blocked(&inv.gate, major);
        return Ok(false);
    }
    let moderate = verdicts.iter().map(|v| v.moderate).sum();
    let minor = verdicts.iter().map(|v| v.minor).sum();
    report::attested(&inv.gate, verdicts.len(), (major, moderate, minor));
    Ok(true)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("git-agent-verdict {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let mode = match parse(args.into_iter()) {
        Ok(mode) => mode,
        Err(detail) => {
            eprintln!("git-agent-verdict: {detail}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    let (label, outcome) = match &mode {
        Mode::Gate(inv) => (inv.gate.as_str(), check(inv)),
        Mode::RubricGuard(docs) => (GUARD_LABEL, rubric_guard(docs)),
        Mode::ReviewerPrompt(gate) => ("reviewer-prompt", reviewer_prompt(gate)),
        Mode::MinVersion(want) => ("check-min-version", min_version(want)),
    };
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(detail) => {
            eprintln!("git-agent-verdict: {label}: {detail}");
            ExitCode::from(2)
        }
    }
}
