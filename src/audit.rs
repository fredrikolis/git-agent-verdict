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
    report::reviewing(&declaration.gate, session.id());
    let answer = agent.run(
        crate::agent::Role::Review,
        &system,
        &crate::brief::sweeping(),
        &session,
        declaration.model.as_deref(),
        ceiling,
    )?;
    let verdicts = runner::verdicts(&answer, declaration.brief.simple)?;
    Ok((verdicts, runner::findings(&answer.text)))
}

// Every gate, not the next one: an audit is a survey, and stopping at the first gate with something to say would leave the rest of the rubric unread. One gate's reviewer failing is still fatal — a survey missing a gate is not a survey, and reporting it as clean would be a lie about the gate that never ran.
pub fn run(ceiling: std::time::Duration) -> Result<bool, String> {
    let hook = declarations::read()?;
    let agent = runner::configured()?;
    report::auditing(&hook.path);
    let mut blocked = false;
    let mut reviewed = 0;
    for declaration in &hook.gates {
        if !reaches(declaration)? {
            report::skipped(&declaration.gate, &declaration.paths);
            continue;
        }
        let (verdicts, findings) = sweep(declaration, &agent, ceiling)?;
        blocked |= verdicts.iter().any(Verdict::blocks);
        reviewed += 1;
        report::audited(&declaration.gate, &verdicts, &findings);
    }
    if reviewed == 0 {
        return Err(format!("{} declared no gate this tree reaches", hook.path));
    }
    report::audit_done(reviewed, blocked);
    Ok(!blocked)
}
