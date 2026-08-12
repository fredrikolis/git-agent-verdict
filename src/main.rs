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
