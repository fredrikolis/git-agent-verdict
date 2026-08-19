// Concern: the invocation grammar — every mode this binary answers and the flags each one accepts | Non-concern: what a mode decides, or anything it prints | IO: (argv) -> Mode

pub const USAGE: &str = concat!(
    "usage: git-agent-verdict <msg-file> <gate> [--simple] [--read-only] [--model <name>]\n",
    "                         [--override-prompt <path>]\n",
    "                         (--standard <name> | --doc <path> | --rule <text>|-)...\n",
    "                         --path <pathspec>...\n",
    "       git-agent-verdict attest --repo <abs path> [--intent <one line>]\n",
    "                                [--timeout <minutes, default 30; or 90s, 45m, 2h>]\n",
    "       git-agent-verdict audit  --repo <abs path> [--timeout <minutes>]\n",
    "       git-agent-verdict reset --repo <abs path> <reason>\n",
    "       git-agent-verdict --standards [<name>]\n",
    "       git-agent-verdict --reviewer-prompt <gate>\n",
    "       git-agent-verdict --require-version <major.minor>\n",
    "       git-agent-verdict --repo-setup-guide"
);

// Undocumented on purpose, and in no usage line: the refusal below is the only place an agent meets it, which is the moment the reminder is worth anything.
pub const BACKGROUND: &str = "--confirm-running-in-background-shell-with-long-timeout";

// Undocumented for the same reason as BACKGROUND, and asked for separately: the background shell is a fact about how the command is being run, this is a statement about what the caller means to spend. An agent reaching for `audit` because `attest` refused is exactly the mistake it stops.
pub const WHOLE: &str = "--confirm-reviewing-the-whole-repo-not-a-commit";

// Verbs a dev agent types, as against a declaration a hook carries: mistyping one is not a repo whose wiring has gone stale, so the setup guide would be noise.
pub fn agent_verb(args: &[String]) -> bool {
    matches!(
        args.first().map(String::as_str),
        Some("attest" | "audit" | "reset")
    )
}

// Wide enough for one real change's aim, and narrow enough that two aims will not fit: the reviewer refuses a brief that argues, so this bounds the change rather than the prose.
const INTENT_LIMIT: usize = 300;

// Above the longest review anyone has watched finish, and far enough above it that hitting this is evidence of a reviewer that has stopped rather than one that is thinking. A ceiling the tool owns: without one the only thing that ends a hung agent is whatever shell it was started in, which kills it with no elapsed time, no signal and nothing said.
const REVIEW_CEILING: std::time::Duration = std::time::Duration::from_secs(30 * 60);

// Minutes, because that is the unit a review is discussed in. The suffixes are for a test that cannot spend a minute proving a hang is caught.
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
    // A ceiling of nothing is every reviewer killed before it can answer, which reads as the agent failing rather than as the flag.
    let total = count
        .checked_mul(seconds)
        .filter(|total| *total > 0)
        .ok_or_else(|| format!("--timeout {text}: a ceiling above zero, and short of forever"))?;
    Ok(std::time::Duration::from_secs(total))
}

// How a gate briefs its reviewer: which template it reads. Held apart because --reviewer-prompt has one without a message, a pathspec or a decision.
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
    // The repo comes first because nothing else means anything without it: the verb acts on the tree named here and never on the one the shell is standing in. The ceiling comes last because it is the one field with an answer when the author gives none.
    Attest(String, Option<String>, std::time::Duration),
    Reset(String, String),
    // No intent, because there is no commit: an audit reviews the tree against the rubrics and lands nothing.
    Audit(String, std::time::Duration),
    ReviewerPrompt(String),
    RequireVersion(String),
    RepoSetupGuide,
    // No name lists them; a name prints that one whole. A gate declares a standard it cannot read, so there has to be a way to read it.
    Standards(Option<String>),
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

// A measure stated in the hook rather than in a file the reviewer must open. Multi-line, and `-` reads stdin, so a rubric a command prints arrives whole at any size: the declaration listing escapes what would otherwise split a gate across lines.
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
    list_standards: bool,
    read_only: bool,
    intent: Option<String>,
    repo: Option<String>,
    timeout: Option<String>,
    background: bool,
    whole: bool,
    brief: Brief,
    standards: Vec<String>,
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
            "--model" => {
                p.model = Some(args.next().ok_or("--model needs a model the agent knows")?)
            }
            BACKGROUND => p.background = true,
            WHOLE => p.whole = true,
            "--simple" => p.brief.simple = true,
            "--read-only" => p.read_only = true,
            "--override-prompt" => {
                let path = args.next().ok_or("--override-prompt needs a path")?;
                p.brief.prompt = Some(canonical("--override-prompt", &path)?);
            }
            // Checked here rather than at brief time: a gate declaring a name this build does not ship should fail the hook that declares it, not the review it was going to pay for.
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
            // `-` is the whole point of a heredoc: a rubric a command prints can be any size, and argv cannot.
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

// Each mode names what it takes; anything else given is a mistyped invocation rather than a mode, and saying so beats acting on half of it.
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
        (BACKGROUND, p.background),
        (WHOLE, p.whole),
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

// One text, and the verb it is printed for fills it in. Hardcoded to attest it told an audit's caller to run attest, which is not the same operation: audit reviews every file each gate reaches and lands nothing, attest reviews the staged diff and commits. A remedy copied verbatim has to be the command the reader ran.
fn foreground(verb: &str, reads: &str, confirmations: &str) -> String {
    format!(
        "a review reads every rubric in full and {reads}, and often runs for ten minutes or more.\n\
         A foreground shell will kill it partway.\n\n\
         Start it in a BACKGROUND shell with a long timeout, then say so:\n\n  \
         git agent-verdict {verb} --repo <abs path to the repo root> \\\n{confirmations}\n\n\
         The flag asserts; it cannot check. It is here because this is worth reading once, and this \
         is when.\n\nRun it directly — no wait loop. {verb} holds the repo while it runs, and a \
         second one refuses at once naming what holds it.\n\nLet the shell capture what it prints; \
         do not redirect it to a file. Under a redirect a run that is killed leaves an empty capture \
         and a truncated log, and the reviewer's own error — the one worth reading — is in neither."
    )
}

fn attest_foreground() -> String {
    let mut said = foreground(
        "attest",
        "the whole staged diff",
        "    --intent \"<the aim, one flat line>\" \\\n    --confirm-running-in-background-shell-with-long-timeout",
    );
    said.push_str(
        "\n\nA killed run is usually not lost work: the reviewer's session is named before it \
         starts, and the next attest takes up the round where it stopped — where that reviewer had \
         got far enough to leave a transcript behind.",
    );
    said
}

fn audit_foreground() -> String {
    foreground(
        "audit",
        "every file each gate reaches",
        "    --confirm-reviewing-the-whole-repo-not-a-commit \\\n    --confirm-running-in-background-shell-with-long-timeout",
    )
}

fn attest(p: &Parsed) -> Result<Mode, String> {
    if !p.background {
        return Err(attest_foreground());
    }
    let repo = target(p)?;
    let ceiling = match &p.timeout {
        Some(text) => ceiling(text)?,
        None => REVIEW_CEILING,
    };
    let Some(intent) = p.intent.clone() else {
        only(
            "attest takes --repo, --intent and --timeout only: what each gate reviews comes from the commit-msg hook",
            p,
            &["--repo", "--timeout", "<positional>", BACKGROUND],
        )?;
        return Ok(Mode::Attest(repo, None, ceiling));
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
        "attest takes --repo, --intent and --timeout only: what each gate reviews comes from the commit-msg hook",
        p,
        &[
            "--intent",
            "--repo",
            "--timeout",
            "<positional>",
            BACKGROUND,
        ],
    )?;
    Ok(Mode::Attest(repo, Some(intent), ceiling))
}

// Said in full at the one moment it is worth reading: an agent that reached for this verb because attest refused it needs the difference between them, not a flag name.
const NOT_A_COMMIT: &str = "audit reviews every file each gate reaches, not the staged diff. \
One full review per gate, and it lands nothing.\n\nUse it after a rubric changed, to find what the \
new wording condemns in code nobody is touching. Normal development is attested from the diff: that \
is what `attest` is for, and it is what the hook demands at commit time.\n\nIf that is what you mean, \
say so:\n\n  git agent-verdict audit --repo <abs path to the repo root> \\\n    \
--confirm-reviewing-the-whole-repo-not-a-commit \\\n    \
--confirm-running-in-background-shell-with-long-timeout";

fn audit(p: &Parsed) -> Result<Mode, String> {
    // Asked before the background shell is: what this verb does differently is the thing a caller reaching for it by mistake needs first, and a guard that teaches the shell first teaches it about a run it should not be making.
    if !p.whole {
        return Err(NOT_A_COMMIT.to_string());
    }
    if !p.background {
        return Err(audit_foreground());
    }
    let repo = target(p)?;
    let ceiling = match &p.timeout {
        Some(text) => ceiling(text)?,
        None => REVIEW_CEILING,
    };
    only(
        "audit takes --repo and --timeout only: what each gate reviews comes from the commit-msg hook, and there is no intent because there is no commit",
        p,
        &["--repo", "--timeout", "<positional>", BACKGROUND, WHOLE],
    )?;
    Ok(Mode::Audit(repo, ceiling))
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
        Some("audit") => return audit(&p),
        Some("reset") => return reset(&p),
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
            "gate '{gate}': every --path names one of its own --doc files, so the only change it could ever review is a change to its own measure — which it cannot judge. It would skip every commit.\nWiden --path past the rubric, or let another gate's --path cover it."
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
