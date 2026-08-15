// Concern: the invocation grammar — every mode this binary answers and the flags each one accepts | Non-concern: what a mode decides, or anything it prints | IO: (argv) -> Mode

pub const USAGE: &str = concat!(
    "usage: git-agent-verdict <msg-file> <gate> [--simple] [--model <name>]\n",
    "                         [--override-prompt <path>]\n",
    "                         (--doc <path> | --rule <text>)... --path <pathspec>...\n",
    "       git-agent-verdict attest --repo <abs path> [--intent <one line>]\n",
    "       git-agent-verdict reset --repo <abs path> <reason>\n",
    "       git-agent-verdict --reviewer-prompt <gate>\n",
    "       git-agent-verdict --require-version <major.minor>\n",
    "       git-agent-verdict --repo-setup-guide"
);

// Undocumented on purpose, and in no usage line: the refusal below is the only place an agent meets it, which is the moment the reminder is worth anything.
pub const BACKGROUND: &str = "--confirm-running-in-background-shell-with-long-timeout";

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
    pub model: Option<String>,
    pub msg_file: String,
    pub gate: String,
    pub docs: Vec<String>,
    pub rules: Vec<String>,
    pub paths: Vec<String>,
    pub brief: Brief,
}

pub enum Mode {
    Gate(Box<Invocation>),
    // The repo comes first because nothing else means anything without it: the verb acts on the tree named here and never on the one the shell is standing in.
    Attest(String, Option<String>),
    Reset(String, String),
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

// A measure short enough to state in the hook, rather than a file the reviewer must open. It travels the declaration listing, which is tab-separated and line-based, so it is one line of its own.
fn rule(text: String) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("--rule is empty".to_string());
    }
    if text.contains('\n') || text.contains('\t') {
        return Err("--rule is one line, and carries no tab".to_string());
    }
    Ok(text)
}

fn canonical_docs(docs: &[String]) -> Result<Vec<String>, String> {
    docs.iter().map(|d| canonical("--doc", d)).collect()
}

// Dead by construction, not merely idle today: a pathspec that resolves to a file is a literal, and a gate built from nothing but its own rubrics meets its own measure or nothing. A glob resolves to no file and reaches whatever is added later, so it is never this.
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
    intent: Option<String>,
    repo: Option<String>,
    background: bool,
    brief: Brief,
    docs: Vec<String>,
    rules: Vec<String>,
    paths: Vec<String>,
    model: Option<String>,
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
            "--repo" => p.repo = Some(args.next().ok_or("--repo needs an absolute path")?),
            "--model" => {
                p.model = Some(args.next().ok_or("--model needs a model the agent knows")?)
            }
            BACKGROUND => p.background = true,
            "--simple" => p.brief.simple = true,
            "--override-prompt" => {
                let path = args.next().ok_or("--override-prompt needs a path")?;
                p.brief.prompt = Some(canonical("--override-prompt", &path)?);
            }
            "--doc" => p.docs.push(args.next().ok_or("--doc needs a path")?),
            "--rule" => p
                .rules
                .push(rule(args.next().ok_or("--rule needs a line of text")?)?),
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
        ("--repo", p.repo.is_some()),
        ("--model", p.model.is_some()),
        (BACKGROUND, p.background),
        ("--simple", p.brief.simple),
        ("--override-prompt", p.brief.prompt.is_some()),
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

// Named, never inferred: a shell an agent has held open for an hour is often not standing where the agent believes, and a verb that reads its target from that shell reviews whichever repo the mistake landed in. What is asserted here reaches the transcript, where it can be read back afterwards.
fn target(p: &Parsed) -> Result<String, String> {
    let Some(path) = p.repo.clone() else {
        // No value offered, deliberately: whatever this printed would be derived from the same shell the flag exists to distrust, and pasted straight back.
        return Err(
            "--repo <absolute path to the repo root> is required, and the shell's directory is not consulted"
                .to_string(),
        );
    };
    if !std::path::Path::new(&path).is_absolute() {
        return Err(format!(
            "--repo {path}: an absolute path. A relative one is the shell's directory again, under another name."
        ));
    }
    Ok(path)
}

// Optional once a review has recorded one: the diary holds the aim, it may not change without a MAJOR, and retyping it can only fail.
const FOREGROUND: &str = "a review reads every rubric in full and the whole staged diff, and often runs \
for ten minutes or more.\nA foreground shell will kill it partway, and you pay for the half that ran.\
\n\nStart it in a BACKGROUND shell with a long timeout, then say so:\n\n  \
git agent-verdict attest --repo <abs path to the repo root> \\\n    \
--intent \"<the aim, one flat line>\" \\\n    \
--confirm-running-in-background-shell-with-long-timeout\n\nThe flag asserts; it cannot check. \
It is here because this is worth reading once, and this is when.\n\nRun it directly — no wait loop. \
attest holds the repo while it runs, and a second one refuses at once naming what holds it.";

fn attest(p: &Parsed) -> Result<Mode, String> {
    if !p.background {
        return Err(FOREGROUND.to_string());
    }
    let repo = target(p)?;
    let Some(intent) = p.intent.clone() else {
        only(
            "attest takes --repo and --intent only: what each gate reviews comes from the commit-msg hook",
            p,
            &["--repo", "<positional>", BACKGROUND],
        )?;
        return Ok(Mode::Attest(repo, None));
    };
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
        "attest takes --repo and --intent only: what each gate reviews comes from the commit-msg hook",
        p,
        &["--intent", "--repo", "<positional>", BACKGROUND],
    )?;
    Ok(Mode::Attest(repo, Some(intent)))
}

// The reason is the whole point of the verb, so it is required rather than defaulted: an unexplained reset is the one this exists to make visible.
fn reset(p: &Parsed) -> Result<Mode, String> {
    let repo = target(p)?;
    let reason = p.positional[1..].join(" ");
    if reason.trim().is_empty() {
        return Err(
            "reset needs a reason, which is recorded and reaches the commit message".to_string(),
        );
    }
    only(
        "reset takes --repo and a reason only: it clears the diary and asks nothing of a gate",
        p,
        &["--repo", "<positional>"],
    )?;
    Ok(Mode::Reset(repo, reason))
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
    // A gate judging against nothing is a gate that has silently stopped judging, so an empty measure is an error rather than a vacuous pass.
    if p.docs.is_empty() && p.rules.is_empty() {
        return Err("a gate needs at least one --doc or --rule to judge against".to_string());
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
            "gate '{gate}': every --path names one of its own --doc files, so the only change it could ever review is a change to its own measure — which it cannot judge. It would skip every commit.\nWiden --path past the rubric, or let another gate's --path cover it."
        ));
    }
    Ok(Mode::Gate(Box::new(Invocation {
        model: p.model.clone(),
        msg_file,
        gate: gate_name(&gate)?,
        docs,
        rules: p.rules,
        paths: p.paths,
        brief: p.brief,
    })))
}
