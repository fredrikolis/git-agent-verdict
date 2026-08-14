// Concern: which agent reviews, and what the tool reads back out of its answer | Non-concern: how that agent is invoked, or what it was asked | IO: (answer) -> verdicts

use crate::agent::{Agent, Answer};
use crate::git;
use crate::trailer::{Counts, Verdict};

const RUNNER_KEY: &str = "agent-verdict.runner";

pub const MARKER: &str = "VERDICT:";
pub const REFUSED: &str = "refused";

// Host configuration, not a repo's: a repo that declared its reviewer would pick one for every maintainer, and they do not share a machine, a budget or a preferred agent.
pub fn configured() -> Result<Agent, String> {
    let named = git::config(RUNNER_KEY).ok_or_else(|| {
        format!(
            "no reviewer configured, and there is no default — unset, this refuses rather than spending on an agent nobody chose:\n  \
             git config --global {RUNNER_KEY} claude"
        )
    })?;
    Agent::named(&named)
}

// The one thing the author supplies, judged before a review is paid for. A reviewer handed the case for a change grades the case instead of the change.
pub fn judge(answer: &Answer, intent: &str) -> Result<(), String> {
    for line in answer.text.lines() {
        let Some(rest) = line.trim().strip_prefix(MARKER) else {
            continue;
        };
        let rest = rest.trim();
        if rest.starts_with(REFUSED) {
            let said = rest
                .trim_start_matches(REFUSED)
                .trim_start_matches(['—', '-', ':', ' ']);
            // What it said, beside the line it judged: the author has to see both to know which words to take out.
            let mut detail = format!("the intent was refused — {said}\n\n  {intent}\n");
            let rest_of = findings(&answer.text);
            if !rest_of.is_empty() {
                detail.push_str(&format!("\n{rest_of}\n"));
            }
            detail.push_str("\nState the aim and nothing else: what the change does, flatly.");
            return Err(detail);
        }
        if rest.starts_with("accepted") {
            return Ok(());
        }
    }
    Err(format!(
        "the intent judge answered with no `{MARKER} accepted` or `{MARKER} {REFUSED}` line"
    ))
}

fn counts_from(fields: &str, simple: bool) -> Result<Counts, String> {
    let mut found: [Option<u32>; 3] = [None; 3];
    for field in fields.split_whitespace() {
        let Some((name, raw)) = field.split_once('=') else {
            continue;
        };
        let slot = match name {
            "major" => 0,
            "moderate" => 1,
            "minor" => 2,
            _ => continue,
        };
        // Named but unreadable is not the same as absent: read as absent it would be reported as a missing field, sending the author after the wrong fault.
        found[slot] = Some(raw.parse().map_err(|_| {
            format!("the reviewer's {MARKER} line has {name}={raw}, which is not a number")
        })?);
    }
    // An advisory gate is never offered a MAJOR rung, so its reviewer is not asked for the count and the zero is recorded here. Reporting one anyway answers a brief it was not given.
    if simple && found[0].is_some_and(|major| major > 0) {
        return Err(
            "this gate is advisory and has no MAJOR rung, but its reviewer reported major>0"
                .to_string(),
        );
    }
    if !simple && found[0].is_none() {
        return Err(format!(
            "the reviewer's {MARKER} line needs major=, moderate= and minor="
        ));
    }
    match (found[1], found[2]) {
        (Some(moderate), Some(minor)) => Ok(Counts {
            major: found[0].unwrap_or(0),
            moderate,
            minor,
        }),
        _ if simple => Err(format!(
            "the reviewer's {MARKER} line needs moderate= and minor="
        )),
        _ => Err(format!(
            "the reviewer's {MARKER} line needs major=, moderate= and minor="
        )),
    }
}

// Everything the reviewer said that was not its verdict line. The counts say how much was found; only this says what, and an author told to address a finding it cannot read has been told nothing.
pub fn findings(output: &str) -> String {
    output
        .lines()
        .filter(|l| !l.trim().starts_with(MARKER))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// The reviewer's numbers are read here and never retyped by the author. Who reviewed, and on what session, come from the agent: the model would be guessing at one and cannot know the other.
pub fn verdicts(answer: &Answer, simple: bool) -> Result<Vec<Verdict>, String> {
    let mut verdicts = Vec::new();
    for line in answer.text.lines() {
        let Some(rest) = line.trim().strip_prefix(MARKER) else {
            continue;
        };
        verdicts.push(Verdict {
            reviewer: answer.reviewer.clone(),
            counts: counts_from(rest, simple)?,
            token: String::new(),
            resets: 0,
            session: answer.session.clone(),
        });
    }
    if verdicts.is_empty() {
        return Err(format!(
            "the reviewer closed with no `{MARKER}` line, so it reported nothing this tool can record"
        ));
    }
    // One review, one verdict: the rest is recorded under a single token and rendered as a trailer apiece, which the gate reads as trailers contradicting the review they name.
    if verdicts.len() > 1 {
        return Err(format!(
            "the reviewer closed with {} `{MARKER}` lines; the brief asks for one, and which of them is the review is not this tool's to guess",
            verdicts.len()
        ));
    }
    Ok(verdicts)
}
