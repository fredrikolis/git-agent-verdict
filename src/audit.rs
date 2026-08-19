// Concern: reviewing the repository as it stands against every gate's rubric | Non-concern: what a commit carries, or what a gate refuses at commit time | IO: (hook) -> findings, status

use crate::agent::Session;
use crate::declarations::{self, Declaration};
use crate::git;
use crate::report;
use crate::runner;
use crate::trailer::Verdict;

// A gate reaching nothing tracked has no repository to review: its pathspec names files this tree does not carry.
fn reaches(declaration: &Declaration) -> Result<bool, String> {
    Ok(!git::tracked(&declaration.paths)?.is_empty())
}

// Nothing is recorded and nothing is committed. A verdict attests one commit, and there is no commit here — what an audit produces is the list of what the rubric now condemns, which the author acts on by making changes that are attested in the usual way.
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
    // Dropped before the answer is judged, and before a failure is carried out: an audit reports a failed gate and keeps going, so a sentence left armed here would name a review that is already over.
    crate::signals::quiet();
    let answer = answered?;
    let verdicts = runner::verdicts(&answer, declaration.brief.simple)?;
    Ok((verdicts, runner::findings(&answer.text)))
}

// Every gate in one pass, and it does not stop for anything it finds: an audit is a survey, not a procedure the author works through a step at a time. A gate whose reviewer fails does not take the rest with it either — the run that was going to cost a full review per gate should not throw away the ones that answered. What failed is named at the end, and fails the run there.
pub fn run(ceiling: std::time::Duration) -> Result<bool, String> {
    let hook = declarations::read()?;
    let agent = runner::configured()?;
    report::auditing(&hook.path);
    let mut blocked = false;
    let mut reviewed = 0;
    let mut failed: Vec<String> = Vec::new();
    for declaration in &hook.gates {
        if !reaches(declaration)? {
            report::skipped(&declaration.gate, &declaration.paths);
            continue;
        }
        match sweep(declaration, &agent, ceiling) {
            Ok((verdicts, findings)) => {
                blocked |= verdicts.iter().any(Verdict::blocks);
                reviewed += 1;
                report::audited(&declaration.gate, &verdicts, &findings);
            }
            Err(said) => {
                report::gate_failed(&declaration.gate, &said);
                failed.push(declaration.gate.clone());
            }
        }
    }
    if reviewed == 0 && failed.is_empty() {
        return Err(format!("{} declared no gate this tree reaches", hook.path));
    }
    report::audit_done(reviewed, blocked, &failed);
    // Reported as it happened and refused here: a survey that lost a gate is not a survey, and an exit saying otherwise would be a claim about the gate that never ran.
    if !failed.is_empty() {
        return Err(format!(
            "no verdict from {}: the audit is incomplete",
            failed.join(", ")
        ));
    }
    Ok(!blocked)
}
