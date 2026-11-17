// Concern: clearing the reviewer sessions earlier runs left on this machine, for every agent | Non-concern: the sessions of a commit being written, which its diary discards | IO: (confirm) -> deletions

use crate::agent::{Agent, Leftover};
use crate::report;

// Spares a commit still being written.
const SETTLED: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

fn settled(leftover: &Leftover) -> bool {
    std::fs::metadata(&leftover.path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|at| at.elapsed().ok())
        .is_some_and(|age| age > SETTLED)
}

pub fn run(confirm: bool) -> Result<bool, String> {
    let found: Vec<Leftover> = [Agent::Claude, Agent::Codex]
        .into_iter()
        .flat_map(|agent| agent.leftovers(&crate::brief::opened_here))
        .filter(settled)
        .collect();
    let bytes = found
        .iter()
        .filter_map(|leftover| std::fs::metadata(&leftover.path).ok())
        .map(|meta| meta.len())
        .sum();
    if confirm {
        for leftover in &found {
            leftover.forget();
        }
    }
    let stuck = found
        .iter()
        .filter(|leftover| leftover.path.exists())
        .count();
    report::leftovers(&found, bytes, confirm, stuck);
    Ok(true)
}
