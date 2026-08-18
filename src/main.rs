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
mod runner;
mod setup;
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

// Both halves as a reviewer gets them, without provoking a review: the standing instructions it is given once, and the line that opens a round.
fn reviewer_prompt(want: &str) -> Result<bool, String> {
    let hook = declarations::read()?;
    let declaration = declarations::find(&hook, want)?;
    println!("{}", brief::system(declaration, brief::Reach::Diff)?);
    println!("──── and on stdin, opening a round ────\n");
    println!(
        "{}",
        brief::opening("<the aim of the change, one flat line>")
    );
    Ok(true)
}

// A hook declares its paths against the root, because that is where git runs it. An agent's attest stands anywhere, and `--path .` from a subdirectory reviews a fraction of the change in silence.
fn at_repo_root() {
    if let Ok(root) = git::toplevel() {
        let _ = std::env::set_current_dir(root);
    }
}

// The named tree, and nothing near it: a path that is not a repo root is the near miss worth refusing, since a submodule taken for its parent reviews the wrong thing and looks like success while doing it.
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
    at_repo_root();
    // The sole argument, never one of several: scanned across the whole line, a stray --version in a gate's declaration exits 0 and the gate passes having checked nothing.
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
    // A declaration that no longer parses is the repo's wiring gone stale, and the whole guide is the answer — not a pointer to it, read at the next commit by whoever is not fixing this one.
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
    // Enumeration must not act: under `set -e` a mode that refuses here kills the hook, and every gate below it leaves the listing — a guard's refusal read back as a hook declaring nothing.
    if declarations::listing_requested() {
        match &mode {
            Mode::Gate(inv) => {
                declarations::emit_gate(inv);
                return ExitCode::SUCCESS;
            }
            // The pin is the one thing enumeration must honour, and killing the listing is how it says so: a hook written against another line declares flags this binary may read differently, and reading them anyway buys a review against a declaration nobody has established this release can parse.
            Mode::RequireVersion(_) => {}
            _ => return ExitCode::SUCCESS,
        }
    }
    let (label, outcome) = match &mode {
        Mode::Gate(inv) => (inv.gate.as_str(), gate::check(inv)),
        // Held across the whole run, and dropped with it: the diary is read, added to and written back, so two runs at once review the same gate, pay for it twice, and the second to finish drops the first's verdict.
        Mode::Attest(repo, intent, ceiling) => (
            "attest",
            enter(repo).and_then(|()| {
                let _held = lock::take()?;
                attest::run(intent.as_deref(), *ceiling)
            }),
        ),
        // Held like attest's: one review at a time in a repo, whichever verb is paying for it.
        Mode::Audit(repo, ceiling) => (
            "audit",
            enter(repo).and_then(|()| {
                let _held = lock::take()?;
                audit::run(*ceiling)
            }),
        ),
        Mode::Reset(repo, reason) => (
            "reset",
            enter(repo).and_then(|()| {
                let _held = lock::take()?;
                attest::reset(reason)
            }),
        ),
        Mode::ReviewerPrompt(gate) => ("reviewer-prompt", reviewer_prompt(gate)),
        Mode::RequireVersion(want) => ("require-version", require_version(want)),
        // The whole text on stdout, because a gate declaring one is judged by every word of it and an author deciding whether to declare it has no other way to read it.
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
