// Concern: starting a survey of the repository as it stands, and what one sweep reviews | Non-concern: what a commit carries | IO: (hook) -> review

use crate::agent::Session;
use crate::declarations::{self, Declaration};
use crate::git;
use crate::report;
use crate::runner;
use crate::trailer::Verdict;

// Nothing recorded or committed: an audit just lists what the rubric now condemns, for the author to act on separately.
fn sweep(
    declaration: &Declaration,
    agent: &crate::agent::Agent,
    ceiling: std::time::Duration,
) -> Result<(Vec<Verdict>, String), String> {
    let system = crate::brief::system(declaration, crate::brief::Reach::Whole)?;
    let session = Session::opened();
    report::reviewing(
        &declaration.gate,
        session.id(),
        crate::agent::transcript_path(session.id()).as_deref(),
    );
    crate::signals::say(&format!(
        "while reviewing {}, session {}.",
        declaration.gate,
        session.id()
    ));
    let answered = agent.run(
        crate::agent::Role::Review,
        &system,
        &crate::brief::sweeping(),
        &session,
        &crate::agent::Terms {
            model: declaration.model.as_deref(),
            ceiling,
            read_only: declaration.read_only,
        },
    );
    // Dropped before the answer is judged: an audit keeps going after a failed gate, so a stale sentence would misname it.
    crate::signals::quiet();
    let answer = answered?;
    let verdicts = runner::verdicts(&answer, declaration.brief.simple)?;
    Ok((verdicts, runner::findings(&answer.text)))
}

fn reaches(declaration: &Declaration) -> Result<bool, String> {
    Ok(!git::tracked(&declaration.paths)?.is_empty())
}

pub fn run(ceiling: std::time::Duration) -> Result<bool, String> {
    let hook = declarations::read()?;
    runner::configured()?;
    let held = crate::lock::take()?;
    let started = crate::round::spawn(held, "audit", ceiling, move |round| {
        sweep_all(&hook, round, ceiling)
    })?;
    report::started(&started);
    Ok(true)
}

// One pass over every gate: a failed reviewer doesn't take the rest of the survey with it — failures are named at the end.
fn sweep_all(
    hook: &declarations::Hook,
    round: &crate::round::Round,
    ceiling: std::time::Duration,
) -> Result<crate::round::Outcome, String> {
    let agent = runner::configured()?;
    report::auditing(&hook.path);
    let mut blocked = false;
    let mut reviewed = 0;
    let mut failed: Vec<String> = Vec::new();
    for declaration in &hook.gates {
        round.at_gate(&declaration.gate);
        if !reaches(declaration)? {
            report::skipped(&declaration.gate, &declaration.paths);
            continue;
        }
        match sweep(declaration, &agent, ceiling) {
            Ok((verdicts, findings)) => {
                blocked |= verdicts.iter().any(Verdict::blocks);
                reviewed += 1;
                report::audited(round.dir(), &declaration.gate, &verdicts, &findings);
            }
            Err(said) => {
                report::gate_failed(&declaration.gate, &said);
                failed.push(declaration.gate.clone());
            }
        }
    }
    if reviewed == 0 && failed.is_empty() {
        return Err(format!("{} declares no gate matching this tree", hook.path));
    }
    report::audit_done(reviewed, blocked, &failed);
    // A survey that lost a gate isn't a survey, even though each failure was already reported above.
    if !failed.is_empty() {
        return Err(format!(
            "no verdict from {}: the audit is incomplete",
            failed.join(", ")
        ));
    }
    Ok(if blocked {
        crate::round::Outcome::Blocked
    } else {
        crate::round::Outcome::Clean
    })
}
