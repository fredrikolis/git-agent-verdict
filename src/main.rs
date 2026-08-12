// Concern: dispatching one invocation to the mode that answers it, and the version floor | Non-concern: the grammar of any mode, what it decides, or what it prints | IO: (argv) -> exit status

mod attest;
mod cli;
mod declarations;
mod gate;
mod git;
mod report;
mod runner;
mod state;
mod trailer;

use cli::Mode;
use std::process::ExitCode;

pub const GUARD_LABEL: &str = "rubric-guard";

fn fields(version: &str, what: &str) -> Result<Vec<u32>, String> {
    version
        .split('.')
        .map(|f| {
            f.parse::<u32>()
                .map_err(|_| format!("{what} '{version}' is not a version like 0.2.0"))
        })
        .collect()
}

// Cargo's caret rule: the leading run through the first non-zero field, so 0.4 is a line of its own and 0.4.1 is not.
fn line_of(version: &[u32]) -> usize {
    version.iter().position(|field| *field != 0).unwrap_or(0)
}

// A pin, not a floor: too old cannot answer what the hook asks, and a later line answers something else. No shim — a hook meets the tool it declares against, or says so.
fn require_version(want: &str) -> Result<bool, String> {
    let have = env!("CARGO_PKG_VERSION");
    let (wanted, installed) = (
        fields(want, "--require-version")?,
        fields(have, "this binary's version")?,
    );
    let width = wanted.len().max(installed.len());
    // Padded, because [0, 2] and [0, 2, 0] are one version and compare unequal as vectors.
    let padded = |mut v: Vec<u32>| {
        v.resize(width, 0);
        v
    };
    let (wanted, installed) = (padded(wanted), padded(installed));
    let line = line_of(&wanted);
    if wanted[..=line] != installed[..=line] {
        report::incompatible(want, have);
        return Ok(false);
    }
    if installed < wanted {
        report::stale(want, have);
        return Ok(false);
    }
    Ok(true)
}

// Kept for reading a gate's brief without provoking one: nothing in the workflow forwards this any more, because the tool runs the review itself.
fn reviewer_prompt(want: &str) -> Result<bool, String> {
    let hook = declarations::read()?;
    let declaration = declarations::find(&hook, want)?;
    // Read against the index, not an empty list: what is in scope is the same question here as it is when a review runs.
    let files = git::staged_existing(&declaration.paths)?;
    println!("{}", report::prompt(declaration, None, &files)?);
    Ok(true)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("git-agent-verdict {}", env!("CARGO_PKG_VERSION"));
        return ExitCode::SUCCESS;
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{}", cli::USAGE);
        return ExitCode::SUCCESS;
    }
    let mode = match cli::parse(args.into_iter()) {
        Ok(mode) => mode,
        Err(detail) => {
            eprintln!("git-agent-verdict: {detail}\n{}", cli::USAGE);
            return ExitCode::from(2);
        }
    };
    let (label, outcome) = match &mode {
        Mode::Gate(inv) => (inv.gate.as_str(), gate::check(inv)),
        Mode::Attest(intent) => ("attest", attest::run(intent)),
        Mode::Reset(reason) => ("reset", attest::reset(reason)),
        Mode::RubricGuard(docs) => (GUARD_LABEL, gate::rubric_guard(docs)),
        Mode::ReviewerPrompt(gate) => ("reviewer-prompt", reviewer_prompt(gate)),
        Mode::RequireVersion(want) => ("require-version", require_version(want)),
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
