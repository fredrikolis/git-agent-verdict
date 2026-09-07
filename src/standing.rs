// Concern: where each gate stands for the commit being written — which applies, which is settled, which is next | Non-concern: reviewing one, or landing them | IO: (hook, diary) -> gate

use crate::declarations::{Declaration, Hook};
use crate::git;
use crate::report;
use crate::state;
use crate::trailer;

pub fn applies(declaration: &Declaration) -> Result<bool, String> {
    Ok(!git::staged(&declaration.paths)?.is_empty())
}

pub fn latest<'a>(gate: &str, steps: &'a [state::Step]) -> Option<&'a state::Step> {
    steps.iter().rfind(|s| s.gate == gate)
}

// MAJOR alone re-opens a gate: re-reviewing for a MODERATE fix would resample advice, forever.
pub fn settled(declaration: &Declaration, steps: &[state::Step]) -> bool {
    latest(&declaration.gate, steps).is_some_and(|step| !step.blocked)
}

pub fn next<'a>(hook: &'a Hook, steps: &[state::Step]) -> Result<Option<&'a Declaration>, String> {
    for declaration in &hook.gates {
        if !applies(declaration)? || settled(declaration, steps) {
            continue;
        }
        return Ok(Some(declaration));
    }
    Ok(None)
}

pub fn survey(
    hook: &Hook,
    steps: &[state::Step],
) -> Result<Vec<(String, report::Standing)>, String> {
    let mut board = Vec::new();
    for declaration in &hook.gates {
        let gate = declaration.gate.clone();
        let standing = if !applies(declaration)? {
            report::Standing::Skipped(declaration.paths.join(", "))
        } else if let Some(step) = latest(&gate, steps) {
            let counts = state::lookup(&step.token)?
                .map(|record| trailer::total(&record.verdicts).render())
                .unwrap_or_default();
            if step.blocked {
                report::Standing::Blocked(counts)
            } else {
                report::Standing::Passed(counts)
            }
        } else {
            report::Standing::Pending
        };
        board.push((gate, standing));
    }
    Ok(board)
}
