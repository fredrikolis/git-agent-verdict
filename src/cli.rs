// Concern: the invocation grammar — every mode this binary answers and the flags each one accepts | Non-concern: what a mode decides, or anything it prints | IO: (argv) -> Mode

pub const USAGE: &str = concat!(
    "usage: git-agent-verdict <msg-file> <gate> [--simple] [--read-only] [--model <name>]\n",
    "                         [--override-prompt <path>]\n",
    "                         (--standard <name> | --doc <path> | --rule <text>|-)...\n",
    "                         --path <pathspec>...\n",
    "       git-agent-verdict attest --repo <abs path> [--intent <one line>]\n",
    "                                [--timeout <minutes, default 30; or 90s, 45m, 2h>]\n",
    "       git-agent-verdict audit  --repo <abs path> [--timeout <minutes>]\n",
    "       git-agent-verdict commit --repo <abs path>\n",
    "       git-agent-verdict await  --repo <abs path>\n",
    "       git-agent-verdict abort  --repo <abs path>\n",
    "       git-agent-verdict reset --repo <abs path> <reason>\n",
    "       git-agent-verdict --standards [<name>]\n",
    "       git-agent-verdict --reviewer-prompt <gate>\n",
    "       git-agent-verdict --require-version <major.minor>\n",
    "       git-agent-verdict --repo-setup-guide"
);

// Undocumented and unlisted: an agent meets it only in the refusal it answers, the one place it's worth anything.
pub const WHOLE: &str = "--confirm-reviewing-the-whole-repo-not-a-commit";
// Undocumented: named only in the refusal it answers, so asserting it presumes you just read why.
pub const STAGED_ONLY: &str = "--confirm-attesting-the-staged-version-not-the-working-tree";

pub fn agent_verb(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("attest" | "audit" | "await" | "abort" | "commit" | "reset")
    )
}

// Narrow enough two aims won't fit, wide enough for one: bounds the change, not the prose.
pub const INTENT_LIMIT: usize = 300;

// Past the longest review anyone's watched finish: beyond it a reviewer has stopped, not still thinking. Without this, only the shell ever kills a hung one.
const REVIEW_CEILING: std::time::Duration = std::time::Duration::from_secs(30 * 60);

fn ceiling(text: &str) -> Result<std::time::Duration, String> {
    let malformed = || {
        format!(
            "--timeout {text}: a whole number of minutes, or a number with a unit — 90s, 45m, 2h"
        )
    };
    let (digits, per) = match text.strip_suffix(['s', 'm', 'h']) {
        Some(digits) => (digits, text.chars().last().ok_or_else(malformed)?),
        None => (text, 'm'),
    };
    let count: u64 = digits.parse().map_err(|_| malformed())?;
    let seconds = match per {
        's' => 1,
        'm' => 60,
        _ => 60 * 60,
    };
    // Zero would kill every reviewer before it answers, reading as the agent failing rather than the flag.
    let total = count
        .checked_mul(seconds)
        .filter(|total| *total > 0)
        .ok_or_else(|| {
            format!("--timeout {text}: must be greater than zero and must not overflow")
        })?;
    Ok(std::time::Duration::from_secs(total))
}

// Held apart because --reviewer-prompt needs one without a message, a pathspec or a decision.
#[derive(Default)]
pub struct Brief {
    pub simple: bool,
    pub prompt: Option<String>,
}

pub struct Invocation {
    pub read_only: bool,
    pub model: Option<String>,
    pub msg_file: String,
    pub gate: String,
    pub standards: Vec<String>,
    pub docs: Vec<String>,
    pub rules: Vec<String>,
    pub paths: Vec<String>,
    pub brief: Brief,
}

pub enum Mode {
    Gate(Box<Invocation>),
    // repo first: the verb acts on the tree named here, never the shell's own directory.
    Attest(String, Option<String>, std::time::Duration, bool),
    Reset(String, String),
    Await(String),
    Abort(String),
    Commit(String),
    Audit(String, std::time::Duration),
    ReviewerPrompt(String),
    RequireVersion(String),
    RepoSetupGuide,
    Standards(Option<String>),
}

fn canonical(flag: &str, path: &str) -> Result<String, String> {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("{flag} {path}: {e}"))
}

// A trailer key is one word: git parses none with a space, so a gate named with one could never read its own trailer back.
fn gate_name(name: &str) -> Result<String, String> {
    let usable = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    if name.is_empty() || !name.chars().all(usable) {
        return Err(format!(
            "gate '{name}': a gate name is letters, digits, '-', '_' or '.'\nIt becomes the trailer key Reviewed-{name}, and git parses a trailer key as one word."
        ));
    }
    Ok(name.to_string())
}

// Multi-line and stdin-fed (`-`), so a rubric a command prints arrives whole regardless of size.
fn rule(text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("--rule is empty".to_string());
    }
    Ok(text)
}

// Read whole, because a heredoc arrives as a stream and a rubric is not complete until it ends.
fn from_stdin() -> Result<String, String> {
    use std::io::Read;
    let mut text = String::new();
    std::io::stdin()
        .read_to_string(&mut text)
        .map_err(|e| format!("--rule -: cannot read stdin: {e}"))?;
    rule(text)
}

fn canonical_docs(docs: &[String]) -> Result<Vec<String>, String> {
    docs.iter().map(|d| canonical("--doc", d)).collect()
}

// Dead by construction: a pathspec resolving to a file is a literal, met only by rubrics on itself. A glob might catch later additions.
fn inert(docs: &[String], paths: &[String]) -> bool {
    !docs.is_empty()
        && paths.iter().all(|p| {
            std::fs::canonicalize(p)
                .is_ok_and(|full| docs.contains(&full.to_string_lossy().into_owned()))
        })
}

#[derive(Default)]
struct Parsed {
    positional: Vec<String>,
    reviewer_prompt: Option<String>,
    require_version: Option<String>,
    setup_guide: bool,
    list_standards: bool,
    read_only: bool,
    intent: Option<String>,
    repo: Option<String>,
    timeout: Option<String>,
    whole: bool,
    staged_only: bool,
    brief: Brief,
    standards: Vec<String>,
    docs: Vec<String>,
    rules: Vec<String>,
    paths: Vec<String>,
    model: Option<String>,
}

fn collect(args: impl Iterator<Item = String>) -> Result<Parsed, String> {
    let mut p = Parsed::default();
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo-setup-guide" => p.setup_guide = true,
            "--standards" => p.list_standards = true,
            "--reviewer-prompt" => {
                p.reviewer_prompt = Some(args.next().ok_or("--reviewer-prompt needs a gate name")?);
            }
            "--require-version" => {
                p.require_version = Some(args.next().ok_or("--require-version needs a version")?);
            }
            "--intent" => p.intent = Some(args.next().ok_or("--intent needs a line of text")?),
            "--repo" => p.repo = Some(args.next().ok_or("--repo needs an absolute path")?),
            "--timeout" => {
                p.timeout = Some(args.next().ok_or("--timeout needs a number of minutes")?)
            }
            "--model" => p.model = Some(args.next().ok_or("--model needs a model name")?),
            WHOLE => p.whole = true,
            STAGED_ONLY => p.staged_only = true,
            "--simple" => p.brief.simple = true,
            "--read-only" => p.read_only = true,
            "--override-prompt" => {
                let path = args.next().ok_or("--override-prompt needs a path")?;
                p.brief.prompt = Some(canonical("--override-prompt", &path)?);
            }
            // Checked here, not at brief time: an unshipped --standard should fail the hook, not the paid-for review.
            "--standard" => {
                let name = args.next().ok_or_else(|| {
                    format!("--standard needs one of: {}", crate::brief::shipped_names())
                })?;
                if crate::brief::shipped(&name).is_none() {
                    return Err(crate::brief::unknown_standard(&name));
                }
                p.standards.push(name);
            }
            "--doc" => p.docs.push(args.next().ok_or("--doc needs a path")?),
            "--rule" => {
                let text = args.next().ok_or("--rule needs text, or - to read stdin")?;
                p.rules.push(if text == "-" {
                    from_stdin()?
                } else {
                    rule(text)?
                });
            }
            "--path" => p.paths.push(args.next().ok_or("--path needs a pathspec")?),
            flag if flag.starts_with('-') => return Err(format!("unknown flag '{flag}'")),
            value => p.positional.push(value.to_string()),
        }
    }
    Ok(p)
}

fn only(detail: &str, p: &Parsed, takes: &[&str]) -> Result<(), String> {
    let given = [
        ("--repo-setup-guide", p.setup_guide),
        ("--standards", p.list_standards),
        ("--reviewer-prompt", p.reviewer_prompt.is_some()),
        ("--require-version", p.require_version.is_some()),
        ("--intent", p.intent.is_some()),
        ("--repo", p.repo.is_some()),
        ("--timeout", p.timeout.is_some()),
        ("--model", p.model.is_some()),
        (WHOLE, p.whole),
        (STAGED_ONLY, p.staged_only),
        ("--simple", p.brief.simple),
        ("--read-only", p.read_only),
        ("--override-prompt", p.brief.prompt.is_some()),
        ("--standard", !p.standards.is_empty()),
        ("--doc", !p.docs.is_empty()),
        ("--rule", !p.rules.is_empty()),
        ("--path", !p.paths.is_empty()),
        ("<positional>", !p.positional.is_empty()),
    ];
    if given
        .iter()
        .any(|(flag, present)| *present && !takes.contains(flag))
    {
        return Err(detail.to_string());
    }
    Ok(())
}

// Named, never inferred: a shell an agent held open for an hour may not be where it believes. Asserted here, it reaches the transcript.
fn target(p: &Parsed) -> Result<String, String> {
    let Some(path) = p.repo.clone() else {
        // No value offered: it would just be derived from the same shell this flag exists to distrust.
        return Err(
            "--repo <absolute path to the repo root> is required, and the shell's directory is not consulted"
                .to_string(),
        );
    };
    if !std::path::Path::new(&path).is_absolute() {
        return Err(format!(
            "--repo {path}: must be absolute. A relative path resolves against the shell's directory."
        ));
    }
    Ok(path)
}

fn attest(p: &Parsed) -> Result<Mode, String> {
    let repo = target(p)?;
    let ceiling = match &p.timeout {
        Some(text) => ceiling(text)?,
        None => REVIEW_CEILING,
    };
    let Some(intent) = p.intent.clone() else {
        only(
            "attest takes --repo, --intent and --timeout only: what each gate reviews comes from the commit-msg hook",
            p,
            &["--repo", "--timeout", STAGED_ONLY, "<positional>"],
        )?;
        return Ok(Mode::Attest(repo, None, ceiling, p.staged_only));
    };
    if intent.contains('\n') || intent.chars().count() > INTENT_LIMIT {
        let detail = format!(
            "--intent: one line, at most {INTENT_LIMIT} characters, stating what the change does.\nAn intent that does not fit describes more than one change; commit them separately: unstage all but one with `git restore --staged <paths>`, attest what remains, then repeat."
        );
        return Err(detail);
    }
    if intent.trim().is_empty() {
        return Err("--intent is empty".to_string());
    }
    only(
        "attest takes --repo, --intent and --timeout only: what each gate reviews comes from the commit-msg hook",
        p,
        &["--intent", "--repo", "--timeout", STAGED_ONLY, "<positional>"],
    )?;
    Ok(Mode::Attest(repo, Some(intent), ceiling, p.staged_only))
}

// Said in full: an agent reaching for this verb because attest refused needs the distinction, not a flag name.
const NOT_A_COMMIT: &str = "audit reviews every file each gate reaches, not the staged diff. \
One full review per gate, and it lands nothing.\n\nUse it after a standard changes, to find what the \
new criteria reject in code no commit is modifying. Normal development is attested from the diff: that \
is what `attest` is for, and it is what the hook demands at commit time.\n\nTo confirm:\n\n  git agent-verdict audit --repo <abs path to the repo root> \\\n    \
--confirm-reviewing-the-whole-repo-not-a-commit";

fn audit(p: &Parsed) -> Result<Mode, String> {
    if !p.whole {
        return Err(NOT_A_COMMIT.to_string());
    }
    let repo = target(p)?;
    let ceiling = match &p.timeout {
        Some(text) => ceiling(text)?,
        None => REVIEW_CEILING,
    };
    only(
        "audit takes --repo and --timeout only: what each gate reviews comes from the commit-msg hook, and there is no intent because there is no commit",
        p,
        &["--repo", "--timeout", "<positional>", WHOLE],
    )?;
    Ok(Mode::Audit(repo, ceiling))
}

fn waiting(p: &Parsed) -> Result<Mode, String> {
    let repo = target(p)?;
    only(
        "await takes --repo only: it waits for the review running in this repository and reads nothing else",
        p,
        &["--repo", "<positional>"],
    )?;
    Ok(Mode::Await(repo))
}

fn landing(p: &Parsed) -> Result<Mode, String> {
    let repo = target(p)?;
    only(
        "commit takes --repo only: it commits exactly what the gates have attested",
        p,
        &["--repo", "<positional>"],
    )?;
    Ok(Mode::Commit(repo))
}

fn ending(p: &Parsed) -> Result<Mode, String> {
    let repo = target(p)?;
    only(
        "abort takes --repo only: it terminates the review running in this repository and retains every verdict",
        p,
        &["--repo", "<positional>"],
    )?;
    Ok(Mode::Abort(repo))
}

fn reset(p: &Parsed) -> Result<Mode, String> {
    let repo = target(p)?;
    let reason = p.positional[1..].join(" ");
    if reason.trim().is_empty() {
        return Err("reset needs a reason; it is recorded in the commit message".to_string());
    }
    only(
        "reset takes --repo and a reason only: it clears the recorded verdicts and asks nothing of a gate",
        p,
        &["--repo", "<positional>"],
    )?;
    Ok(Mode::Reset(repo, reason))
}

pub fn parse(args: impl Iterator<Item = String>) -> Result<Mode, String> {
    let p = collect(args)?;
    match p.positional.first().map(String::as_str) {
        Some("attest") => return attest(&p),
        Some("audit") => return audit(&p),
        Some("reset") => return reset(&p),
        Some("await") => return waiting(&p),
        Some("abort") => return ending(&p),
        Some("commit") => return landing(&p),
        _ => {}
    }
    // Answered from the binary alone, outside any repo: what this build carries is a fact about the binary, and a caller asking has not necessarily got a repo yet.
    if p.list_standards {
        only(
            "--standards takes a standard's name, or nothing at all",
            &p,
            &["--standards", "<positional>"],
        )?;
        let named = p.positional.first().cloned();
        if let Some(name) = &named {
            if crate::brief::shipped(name).is_none() {
                return Err(crate::brief::unknown_standard(name));
            }
        }
        return Ok(Mode::Standards(named));
    }
    // Answered from nothing at all: it is the one mode that works outside a repo, which is where someone wiring one up starts.
    if p.setup_guide {
        only(
            "--repo-setup-guide takes nothing else",
            &p,
            &["--repo-setup-guide"],
        )?;
        return Ok(Mode::RepoSetupGuide);
    }
    // Answered from the binary's own version alone, so it takes nothing else: a hook runs it before it asks the tool for anything, where there is no gate to speak of yet.
    if let Some(want) = p.require_version.clone() {
        only(
            "--require-version takes a version only",
            &p,
            &["--require-version"],
        )?;
        return Ok(Mode::RequireVersion(want));
    }
    if let Some(gate) = p.reviewer_prompt.clone() {
        let detail = "--reviewer-prompt takes a gate name only: how the gate is briefed comes from the commit-msg hook";
        only(detail, &p, &["--reviewer-prompt"])?;
        return Ok(Mode::ReviewerPrompt(gate));
    }
    // A gate judging against nothing is a gate that has silently stopped judging, so an empty measure is an error rather than a vacuous pass.
    if p.docs.is_empty() && p.rules.is_empty() && p.standards.is_empty() {
        return Err(format!(
            "a gate needs at least one --standard, --doc or --rule to judge against.\nThis build ships: {}\nList them with: git agent-verdict --standards",
            crate::brief::shipped_names()
        ));
    }
    let [msg_file, gate] = <[String; 2]>::try_from(p.positional).map_err(|got| {
        format!(
            "expected <msg-file> and <gate>, got {} argument(s)",
            got.len()
        )
    })?;
    if p.paths.is_empty() {
        return Err("at least one --path is required".to_string());
    }
    let docs = canonical_docs(&p.docs)?;
    if inert(&docs, &p.paths) {
        return Err(format!(
            "gate '{gate}': every --path names one of its own --doc files, so the only change it could review is a change to its own criteria, which it cannot judge. It would skip every commit.\nExtend --path beyond the criteria files, or let another gate's --path cover them."
        ));
    }
    Ok(Mode::Gate(Box::new(Invocation {
        read_only: p.read_only,
        model: p.model.clone(),
        msg_file,
        gate: gate_name(&gate)?,
        standards: p.standards,
        docs,
        rules: p.rules,
        paths: p.paths,
        brief: p.brief,
    })))
}
