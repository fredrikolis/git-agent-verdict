// Concern: dispatching one invocation to the mode that answers it, and the version floor | Non-concern: the grammar of any mode, what it decides, or what it prints | IO: (argv) -> exit status

mod agent;
mod attest;
mod audit;
mod brief;
mod cli;
mod declarations;
mod gate;
mod git;
mod lock;
mod report;
mod round;
mod runner;
mod setup;
mod signals;
mod standing;
mod state;
mod trailer;

use cli::Mode;
use std::process::ExitCode;

fn fields(version: &str, what: &str) -> Result<Vec<u32>, String> {
    version
        .split('.')
        .map(|f| {
            f.parse::<u32>()
                .map_err(|_| format!("{what} '{version}' is not a version like 0.2.0"))
        })
        .collect()
}

// Cargo's caret rule: 0.4 is a line of its own, 0.4.1 is not.
fn line_of(version: &[u32]) -> usize {
    version.iter().position(|field| *field != 0).unwrap_or(0)
}

// A pin, not a floor: a later line answers something else too, so there's no shim — the hook meets the version it declares, or says so.
fn require_version(want: &str) -> Result<bool, String> {
    let have = env!("CARGO_PKG_VERSION");
    let (wanted, installed) = (
        fields(want, "--require-version")?,
        fields(have, "this binary's version")?,
    );
    let width = wanted.len().max(installed.len());
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

fn reviewer_prompt(want: &str) -> Result<bool, String> {
    let hook = declarations::read()?;
    let declaration = declarations::find(&hook, want)?;
    println!("{}", brief::system(declaration, brief::Reach::Diff)?);
    println!("──── and on stdin, opening a review ────\n");
    println!("{}", brief::opening("<the intent of the change, one line>"));
    Ok(true)
}

// A hook declares its paths against the root; `--path .` from a subdirectory would silently review only a fraction of the change.
fn at_repo_root() {
    if let Ok(root) = git::toplevel() {
        let _ = std::env::set_current_dir(root);
    }
}

// A path that isn't a repo root is worth refusing: a submodule taken for its parent reviews the wrong tree and looks like success.
fn enter(repo: &str) -> Result<(), String> {
    std::env::set_current_dir(repo).map_err(|e| format!("--repo {repo}: {e}"))?;
    let root = git::toplevel()?;
    let same = |p: &str| std::fs::canonicalize(p).ok();
    if same(repo) != same(&root) {
        return Err(format!(
            "--repo {repo} is not a repo root. The root it sits under is:\n  {root}"
        ));
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    signals::arm(signals::Posture::Caller);
    at_repo_root();
    // The sole argument, never one of several: scanned across the line, a stray --version would exit 0 having checked nothing.
    if let [only] = args.as_slice() {
        if only == "--version" || only == "-V" {
            println!("git-agent-verdict {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        if only == "--help" || only == "-h" {
            println!("{}", cli::USAGE);
            return ExitCode::SUCCESS;
        }
    }
    let mode = match cli::parse(args.clone().into_iter()) {
        Ok(mode) => mode,
        Err(detail) => {
            eprintln!("git-agent-verdict: error: {detail}\n{}", cli::USAGE);
            if !cli::agent_verb(&args) {
                eprintln!("\n{}", setup::guide());
            }
            return ExitCode::from(2);
        }
    };
    // Enumeration must not act: under `set -e`, a refusal here kills the hook and every gate below it leaves the listing.
    if declarations::listing_requested() {
        match &mode {
            Mode::Gate(inv) => {
                declarations::emit_gate(inv);
                return ExitCode::SUCCESS;
            }
            Mode::RequireVersion(_) => {}
            _ => return ExitCode::SUCCESS,
        }
    }
    let (label, outcome) = match &mode {
        Mode::Gate(inv) => (inv.gate.as_str(), gate::check(inv)),
        Mode::Attest(repo, intent, ceiling, staged_only) => (
            "attest",
            enter(repo).and_then(|()| attest::run(intent.as_deref(), *ceiling, *staged_only)),
        ),
        Mode::Audit(repo, ceiling) => ("audit", enter(repo).and_then(|()| audit::run(*ceiling))),
        Mode::Await(repo) => ("await", enter(repo).and_then(|()| round::wait())),
        Mode::Abort(repo) => ("abort", enter(repo).and_then(|()| attest::abandon())),
        Mode::Commit(repo) => ("commit", enter(repo).and_then(|()| attest::commit())),
        Mode::Reset(repo, reason) => (
            "reset",
            enter(repo).and_then(|()| {
                let _held = lock::take()?;
                attest::reset(reason)
            }),
        ),
        Mode::ReviewerPrompt(gate) => ("reviewer-prompt", reviewer_prompt(gate)),
        Mode::RequireVersion(want) => ("require-version", require_version(want)),
        Mode::Standards(name) => ("standards", {
            match name {
                Some(name) => println!("{}", brief::shipped(name).unwrap_or_default().trim_end()),
                None => {
                    println!("{}", brief::shipped_listing());
                    println!(
                        "\nDeclare one on a gate with --standard <name>. Read one in full with --standards <name>."
                    );
                }
            }
            Ok(true)
        }),
        Mode::RepoSetupGuide => ("repo-setup-guide", {
            println!("{}", setup::guide());
            Ok(true)
        }),
    };
    match outcome {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(detail) => {
            eprintln!("git-agent-verdict: error: {label}: {detail}");
            ExitCode::from(2)
        }
    }
}
