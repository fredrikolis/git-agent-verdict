// Concern: the invocation grammar and the pass/fail decision for one gate | Non-concern: the trailer grammar, or the wording of a rejection | IO: (argv, message file) -> exit status

mod git;
mod report;
mod trailer;

use std::process::ExitCode;

const USAGE: &str =
    "usage: git-agent-verdict <msg-file> <gate> [--per-file] --doc <path>... --path <pathspec>...";

pub struct Invocation {
    pub msg_file: String,
    pub gate: String,
    pub per_file: bool,
    pub docs: Vec<String>,
    pub paths: Vec<String>,
}

// Every list is a repeated singular flag: no variadic can absorb the token meant for its neighbour.
fn parse(args: impl Iterator<Item = String>) -> Result<Invocation, String> {
    let mut positional = Vec::new();
    let (mut per_file, mut docs, mut paths) = (false, Vec::new(), Vec::new());
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--per-file" => per_file = true,
            "--doc" => docs.push(args.next().ok_or("--doc needs a path")?),
            "--path" => paths.push(args.next().ok_or("--path needs a pathspec")?),
            flag if flag.starts_with('-') => return Err(format!("unknown flag '{flag}'")),
            value => positional.push(value.to_string()),
        }
    }
    let [msg_file, gate] = <[String; 2]>::try_from(positional).map_err(|got| {
        format!(
            "expected <msg-file> and <gate>, got {} argument(s)",
            got.len()
        )
    })?;
    if docs.is_empty() || paths.is_empty() {
        return Err("at least one --doc and one --path are required".to_string());
    }
    // Resolved once, here: the reviewer block promises absolute paths, and an unresolvable doc would silently exempt itself from the circular-rubric guard.
    let docs = docs
        .into_iter()
        .map(|d| {
            std::fs::canonicalize(&d)
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|e| format!("--doc {d}: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Invocation {
        msg_file,
        gate,
        per_file,
        docs,
        paths,
    })
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
fn staged_rubrics(inv: &Invocation) -> Result<Vec<String>, String> {
    let in_repo: Vec<String> = inv
        .docs
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

fn check(inv: &Invocation) -> Result<bool, String> {
    let rubrics = staged_rubrics(inv)?;
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
        report::missing(inv, detail);
        return Ok(false);
    }

    if inv.per_file {
        let gaps = per_file_gaps(inv, &verdicts)?;
        if !gaps.is_empty() {
            report::missing(inv, &format!("no trailer names: {}", gaps.join(", ")));
            return Ok(false);
        }
    }

    let major = verdicts.iter().map(|v| v.major).sum();
    let moderate = verdicts.iter().map(|v| v.moderate).sum();
    if verdicts.iter().any(trailer::Verdict::blocks) {
        report::blocked(&inv.gate, major, moderate);
        return Ok(false);
    }
    let minor = verdicts.iter().map(|v| v.minor).sum();
    report::attested(&inv.gate, verdicts.len(), minor);
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
    let inv = match parse(args.into_iter()) {
        Ok(inv) => inv,
        Err(detail) => {
            eprintln!("git-agent-verdict: {detail}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    match check(&inv) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(detail) => {
            eprintln!("git-agent-verdict: {}: {detail}", inv.gate);
            ExitCode::from(2)
        }
    }
}
