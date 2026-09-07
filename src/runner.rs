// Concern: which agent reviews, and what the tool reads back out of its answer | Non-concern: how that agent is invoked, or what it was asked | IO: (answer) -> verdicts

use crate::agent::{Agent, Answer};
use crate::git;
use crate::trailer::{Counts, Verdict};

const RUNNER_KEY: &str = "agent-verdict.runner";

pub const MARKER: &str = "VERDICT:";
pub const REFUSED: &str = "refused";

// Host config, not the repo's: maintainers don't share a machine, a budget, or a preferred agent.
pub fn configured() -> Result<Agent, String> {
    let named = git::config(RUNNER_KEY).ok_or_else(|| {
        format!(
            "no reviewer configured, and there is no default:\n  \
             git config --global {RUNNER_KEY} claude"
        )
    })?;
    Agent::named(&named)
}

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
            let mut detail = format!("the intent was refused — {said}\n\n  {intent}\n");
            let rest_of = findings(&answer.text);
            if !rest_of.is_empty() {
                detail.push_str(&format!("\n{rest_of}\n"));
            }
            detail.push_str("\nState the intent only: what the change does.");
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
        found[slot] = Some(raw.parse().map_err(|_| {
            format!("the reviewer's {MARKER} line has {name}={raw}, which is not a number")
        })?);
    }
    // An advisory gate is never offered a MAJOR rung; reporting one anyway answers a brief it wasn't given.
    if simple && found[0].is_some_and(|major| major > 0) {
        return Err(
            "this gate is advisory and has no MAJOR severity, but its reviewer reported major>0"
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

pub fn findings(output: &str) -> String {
    output
        .lines()
        .filter(|l| !l.trim().starts_with(MARKER))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

// Reviewer and session come from the agent, not the answer text: the model can't know either reliably.
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
        // A turn-limit cutoff is a re-run; ignoring the brief is a brief to fix — same silence here.
        let stopped = match answer.stop_reason.as_str() {
            "" | "end_turn" => String::new(),
            reason => format!(" — it stopped on {reason}"),
        };
        return Err(format!(
            "the reviewer closed with no `{MARKER}` line{stopped}, so it reported nothing this tool can record"
        ));
    }
    // One review, one verdict: multiple would render as trailers contradicting the review they name.
    if verdicts.len() > 1 {
        return Err(format!(
            "the reviewer closed with {} `{MARKER}` lines; the brief asks for one, and which of them is the review is not this tool's to guess",
            verdicts.len()
        ));
    }
    Ok(verdicts)
}
