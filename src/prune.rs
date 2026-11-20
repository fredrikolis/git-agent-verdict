// Concern: clearing the reviewer sessions earlier runs left on this machine, for every agent | Non-concern: the sessions of a commit being written, which its diary discards | IO: (confirm) -> deletions

use crate::agent::{Agent, Leftover};
use crate::report;

// Spares a commit still being written.
const SETTLED: std::time::Duration = std::time::Duration::from_secs(24 * 60 * 60);

fn settled(leftover: &Leftover) -> bool {
    leftover
        .touched
        .and_then(|at| at.elapsed().ok())
        .is_some_and(|age| age > SETTLED)
}

pub fn run(confirm: bool) -> Result<bool, String> {
    let ours = &crate::brief::opened_here;
    let mut found: Vec<Leftover> = Vec::new();
    let mut unread = Vec::new();
    for agent in [Agent::Claude, Agent::Codex] {
        found.extend(agent.leftovers(ours));
        match agent.stranded(ours) {
            Ok(stranded) => found.extend(stranded),
            Err(why) => unread.push(why),
        }
    }
    found.retain(settled);
    let bytes = found
        .iter()
        .filter_map(|leftover| std::fs::metadata(&leftover.path).ok())
        .map(|meta| meta.len())
        .sum();
    let failed: Vec<String> = if confirm {
        found
            .iter()
            .filter_map(|leftover| leftover.forget().err())
            .collect()
    } else {
        Vec::new()
    };
    report::leftovers(&found, bytes, confirm, &failed, &unread);
    Ok(failed.is_empty())
}
