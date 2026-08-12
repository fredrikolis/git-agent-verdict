// Concern: the invocation grammar — every mode this binary answers and the flags each one accepts | Non-concern: what a mode decides, or anything it prints | IO: (argv) -> Mode

pub const USAGE: &str = concat!(
    "usage: git-agent-verdict <msg-file> <gate> [--simple] [--override-prompt <path>]\n",
    "                         --doc <path>... --path <pathspec>...\n",
    "       git-agent-verdict attest --intent <one line>\n",
    "       git-agent-verdict reset <reason>\n",
    "       git-agent-verdict --rubric-guard --doc <path>...\n",
    "       git-agent-verdict --reviewer-prompt <gate>\n",
    "       git-agent-verdict --require-version <version>"
);

// Wide enough for one real change's aim, and narrow enough that two aims will not fit: the reviewer refuses a brief that argues, so this bounds the change rather than the prose.
pub const INTENT_LIMIT: usize = 300;

// How a gate briefs its reviewer: which template it reads. Held apart because --reviewer-prompt has one without a message, a pathspec or a decision.
#[derive(Default, Clone)]
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

// The preflight needs neither the message nor a gate, so it is a flag-only mode rather than a gate that ignores half its arguments.
pub enum Mode {
    Gate(Box<Invocation>),
    Attest(String),
    Reset(String),
    RubricGuard(Vec<String>),
    ReviewerPrompt(String),
    RequireVersion(String),
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

#[derive(Default)]
struct Parsed {
    positional: Vec<String>,
    guard: bool,
    reviewer_prompt: Option<String>,
    require_version: Option<String>,
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
            "--rubric-guard" => p.guard = true,
            "--reviewer-prompt" => {
                p.reviewer_prompt = Some(args.next().ok_or("--reviewer-prompt needs a gate name")?)
            }
            "--require-version" => {
                p.require_version = Some(args.next().ok_or("--require-version needs a version")?)
            }
            "--intent" => p.intent = Some(args.next().ok_or("--intent needs a line of text")?),
            "--simple" => p.brief.simple = true,
            "--override-prompt" => {
                let path = args.next().ok_or("--override-prompt needs a path")?;
                p.brief.prompt = Some(canonical(&path)?);
            }
            "--doc" => p.docs.push(args.next().ok_or("--doc needs a path")?),
            "--path" => p.paths.push(args.next().ok_or("--path needs a pathspec")?),
            flag if flag.starts_with('-') => return Err(format!("unknown flag '{flag}'")),
            value => p.positional.push(value.to_string()),
        }
    }
    Ok(p)
}

fn only(what: &str, taken: bool) -> Result<(), String> {
    if taken {
        return Err(what.to_string());
    }
    Ok(())
}

fn attest(p: &Parsed) -> Result<Mode, String> {
    let intent = p.intent.clone().ok_or("attest needs --intent")?;
    // An aim that will not fit is usually two aims: the limit is a decomposition check as much as a brevity one.
    if intent.contains('\n') || intent.chars().count() > INTENT_LIMIT {
        let detail = format!(
            "--intent is one line of at most {INTENT_LIMIT} characters: state the aim flatly, as a spec would.\nAn aim that cannot be said that concisely is more than one change — commit them separately."
        );
        return Err(detail);
    }
    if intent.trim().is_empty() {
        return Err("--intent is empty".to_string());
    }
    only(
        "attest takes --intent only: what each gate reviews comes from the commit-msg hook",
        p.guard || p.brief.simple || !p.docs.is_empty() || !p.paths.is_empty(),
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
    Ok(Mode::Reset(reason))
}

pub fn parse(args: impl Iterator<Item = String>) -> Result<Mode, String> {
    let p = collect(args)?;
    match p.positional.first().map(String::as_str) {
        Some("attest") => return attest(&p),
        Some("reset") => return reset(&p),
        _ => {}
    }
    // Answered from the binary's own version alone, so it takes nothing else: a hook runs it before it asks the tool for anything, where there is no gate to speak of yet.
    if let Some(want) = p.require_version {
        let clean = !p.guard
            && !p.brief.simple
            && p.brief.prompt.is_none()
            && p.reviewer_prompt.is_none()
            && p.docs.is_empty()
            && p.paths.is_empty()
            && p.positional.is_empty();
        only("--require-version takes a version only", !clean)?;
        return Ok(Mode::RequireVersion(want));
    }
    if let Some(gate) = p.reviewer_prompt {
        let detail = "--reviewer-prompt takes a gate name only: how the gate is briefed comes from the commit-msg hook";
        let clean = !p.guard
            && !p.brief.simple
            && p.brief.prompt.is_none()
            && p.docs.is_empty()
            && p.paths.is_empty()
            && p.positional.is_empty();
        only(detail, !clean)?;
        return Ok(Mode::ReviewerPrompt(gate));
    }
    // A preflight guarding nothing is a hook that has silently stopped guarding, so an empty list is an error in both modes rather than a vacuous pass.
    if p.docs.is_empty() {
        return Err("at least one --doc is required".to_string());
    }
    if p.guard {
        let detail = "--rubric-guard reads the index alone: it demands no review, so no <msg-file>, <gate>, --path, --simple or --override-prompt";
        let clean = !p.brief.simple
            && p.brief.prompt.is_none()
            && p.paths.is_empty()
            && p.positional.is_empty();
        only(detail, !clean)?;
        return Ok(Mode::RubricGuard(canonical_docs(p.docs)?));
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
        gate,
        docs: canonical_docs(p.docs)?,
        paths: p.paths,
        brief: p.brief,
    })))
}
