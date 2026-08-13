// Concern: the invocation grammar — every mode this binary answers and the flags each one accepts | Non-concern: what a mode decides, or anything it prints | IO: (argv) -> Mode

pub const USAGE: &str = concat!(
    "usage: git-agent-verdict <msg-file> <gate> [--simple] [--override-prompt <path>]\n",
    "                         --doc <path>... --path <pathspec>...\n",
    "       git-agent-verdict attest --intent <one line>\n",
    "       git-agent-verdict reset <reason>\n",
    "       git-agent-verdict --reviewer-prompt <gate>\n",
    "       git-agent-verdict --require-version <major.minor>\n",
    "       git-agent-verdict --repo-setup-guide"
);

// Verbs a dev agent types, as against a declaration a hook carries: mistyping one is not a repo whose wiring has gone stale, so the setup guide would be noise.
pub fn agent_verb(args: &[String]) -> bool {
    matches!(args.first().map(String::as_str), Some("attest" | "reset"))
}

// Wide enough for one real change's aim, and narrow enough that two aims will not fit: the reviewer refuses a brief that argues, so this bounds the change rather than the prose.
const INTENT_LIMIT: usize = 300;

// How a gate briefs its reviewer: which template it reads. Held apart because --reviewer-prompt has one without a message, a pathspec or a decision.
#[derive(Default)]
pub struct Brief {
    pub simple: bool,
    pub prompt: Option<String>,
}

pub struct Invocation {
    pub msg_file: String,
    pub gate: String,
    pub docs: Vec<String>,
    pub paths: Vec<String>,
    pub brief: Brief,
}

pub enum Mode {
    Gate(Box<Invocation>),
    Attest(String),
    Reset(String),
    ReviewerPrompt(String),
    RequireVersion(String),
    RepoSetupGuide,
}

// Resolved once, here: the reviewer block promises absolute paths, and a path that does not resolve exempts itself in silence — a doc from its gate, an override from the template it replaces.
fn canonical(flag: &str, path: &str) -> Result<String, String> {
    std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .map_err(|e| format!("{flag} {path}: {e}"))
}

// A trailer key is one word. Git parses no key carrying a space, so a gate named with one earns a trailer its own gate can never read back, and the remedy it prints is the line it just refused.
fn gate_name(name: &str) -> Result<String, String> {
    let usable = |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.');
    if name.is_empty() || !name.chars().all(usable) {
        return Err(format!(
            "gate '{name}': a gate name is letters, digits, '-', '_' or '.'\nIt becomes the trailer key Reviewed-{name}, and git parses a trailer key as one word."
        ));
    }
    Ok(name.to_string())
}

fn canonical_docs(docs: &[String]) -> Result<Vec<String>, String> {
    docs.iter().map(|d| canonical("--doc", d)).collect()
}

#[derive(Default)]
struct Parsed {
    positional: Vec<String>,
    reviewer_prompt: Option<String>,
    require_version: Option<String>,
    setup_guide: bool,
    intent: Option<String>,
    brief: Brief,
    docs: Vec<String>,
    paths: Vec<String>,
}

// Every list is a repeated singular flag: no variadic can absorb the token meant for its neighbour.
fn collect(args: impl Iterator<Item = String>) -> Result<Parsed, String> {
    let mut p = Parsed::default();
    let mut args = args;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--repo-setup-guide" => p.setup_guide = true,
            "--reviewer-prompt" => {
                p.reviewer_prompt = Some(args.next().ok_or("--reviewer-prompt needs a gate name")?);
            }
            "--require-version" => {
                p.require_version = Some(args.next().ok_or("--require-version needs a version")?);
            }
            "--intent" => p.intent = Some(args.next().ok_or("--intent needs a line of text")?),
            "--simple" => p.brief.simple = true,
            "--override-prompt" => {
                let path = args.next().ok_or("--override-prompt needs a path")?;
                p.brief.prompt = Some(canonical("--override-prompt", &path)?);
            }
            "--doc" => p.docs.push(args.next().ok_or("--doc needs a path")?),
            "--path" => p.paths.push(args.next().ok_or("--path needs a pathspec")?),
            flag if flag.starts_with('-') => return Err(format!("unknown flag '{flag}'")),
            value => p.positional.push(value.to_string()),
        }
    }
    Ok(p)
}

// Each mode names what it takes; anything else given is a mistyped invocation rather than a mode, and saying so beats acting on half of it.
fn only(detail: &str, p: &Parsed, takes: &[&str]) -> Result<(), String> {
    let given = [
        ("--repo-setup-guide", p.setup_guide),
        ("--reviewer-prompt", p.reviewer_prompt.is_some()),
        ("--require-version", p.require_version.is_some()),
        ("--intent", p.intent.is_some()),
        ("--simple", p.brief.simple),
        ("--override-prompt", p.brief.prompt.is_some()),
        ("--doc", !p.docs.is_empty()),
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

fn attest(p: &Parsed) -> Result<Mode, String> {
    let intent = p.intent.clone().ok_or("attest needs --intent")?;
    // An aim that will not fit is usually two aims: the limit is a decomposition check as much as a brevity one.
    if intent.contains('\n') || intent.chars().count() > INTENT_LIMIT {
        let detail = format!(
            "--intent: one line, at most {INTENT_LIMIT} characters, stating the aim as a spec would.\nAn aim that will not fit is more than one change — commit them separately."
        );
        return Err(detail);
    }
    if intent.trim().is_empty() {
        return Err("--intent is empty".to_string());
    }
    only(
        "attest takes --intent only: what each gate reviews comes from the commit-msg hook",
        p,
        &["--intent", "<positional>"],
    )?;
    Ok(Mode::Attest(intent))
}

// The reason is the whole point of the verb, so it is required rather than defaulted: an unexplained reset is the one this exists to make visible.
fn reset(p: &Parsed) -> Result<Mode, String> {
    let reason = p.positional[1..].join(" ");
    if reason.trim().is_empty() {
        return Err(
            "reset needs a reason, which is recorded and reaches the commit message".to_string(),
        );
    }
    only(
        "reset takes a reason only: it clears the diary and asks nothing of a gate",
        p,
        &["<positional>"],
    )?;
    Ok(Mode::Reset(reason))
}

pub fn parse(args: impl Iterator<Item = String>) -> Result<Mode, String> {
    let p = collect(args)?;
    match p.positional.first().map(String::as_str) {
        Some("attest") => return attest(&p),
        Some("reset") => return reset(&p),
        _ => {}
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
    // A gate judging against nothing is a gate that has silently stopped judging, so an empty list is an error rather than a vacuous pass.
    if p.docs.is_empty() {
        return Err("at least one --doc is required".to_string());
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
    Ok(Mode::Gate(Box::new(Invocation {
        msg_file,
        gate: gate_name(&gate)?,
        docs: canonical_docs(&p.docs)?,
        paths: p.paths,
        brief: p.brief,
    })))
}
