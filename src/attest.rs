// Concern: driving a commit to a full set of verdicts — which gate runs next | Non-concern: the trailer's grammar, or how a gate is later checked | IO: (intent) -> reviews, commit

use crate::declarations::{self, Declaration, Hook};
use crate::gate;
use crate::git;
use crate::report;
use crate::state;
use crate::trailer::{self, Verdict};

// A gate with nothing staged is not part of this commit — exactly as the gate itself decides when the hook runs.
fn applies(declaration: &Declaration) -> Result<bool, String> {
    Ok(!git::staged(&declaration.paths)?.is_empty())
}

// The last word on a gate, which is the only one that counts: a re-review supersedes what it was asked to look at again.
fn latest<'a>(gate: &str, steps: &'a [state::Step]) -> Option<&'a state::Step> {
    steps.iter().rfind(|s| s.gate == gate)
}

// MAJOR alone re-opens a gate. Acting on a MODERATE moves content too, and re-reviewing for that resamples advice the author already has — a loop keyed on content never ends.
fn settled(declaration: &Declaration, steps: &[state::Step]) -> bool {
    latest(&declaration.gate, steps).is_some_and(|step| !step.blocked)
}

// Line order is review order, and a later gate must never be judged against content an earlier one is still changing: the position is held here so nothing has to sequence it by hand.
fn next<'a>(hook: &'a Hook, steps: &[state::Step]) -> Result<Option<&'a Declaration>, String> {
    for declaration in &hook.gates {
        if !applies(declaration)? || settled(declaration, steps) {
            continue;
        }
        return Ok(Some(declaration));
    }
    Ok(None)
}

// Every gate this hook declares, and where each one stands: what is left is not a number, because a fix can bring a file into a pathspec that reached nothing before.
fn survey(hook: &Hook, steps: &[state::Step]) -> Result<Vec<(String, report::Standing)>, String> {
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
            report::Standing::Waiting
        };
        board.push((gate, standing));
    }
    Ok(board)
}

fn session_of(step: &state::Step) -> Option<String> {
    let record = state::lookup(&step.token).ok()??;
    Some(record.verdicts.first()?.session.clone()).filter(|s| !s.is_empty())
}

// The session the last reviewer reported. A runner that can resume one reads what changed, rather than sampling a rubric afresh every round.
fn prior_session(declaration: &Declaration, steps: &[state::Step]) -> Option<String> {
    session_of(latest(&declaration.gate, steps)?)
}

// Three things can be asked of a reviewer, and they are not interchangeable: a gate nobody has reviewed, one whose findings the author has since acted on, and one whose round was cut short with nothing changed. Told the wrong one, a resumed reviewer reports on fixes nobody made.
enum Opening {
    First,
    Again,
    Interrupted,
}

struct Round {
    opening: Opening,
    session: crate::agent::Session,
    tries: u32,
}

// A round that opened and never closed is taken up rather than paid for again — but only while its transcript is really there, and only so many times: a session that cannot be resumed would otherwise be retried by every run from here on, and a gate nothing can enter is worse than a review paid for twice.
const TRIES: u32 = 3;

fn round_for(declaration: &Declaration, steps: &[state::Step]) -> Result<Round, String> {
    if let Some(held) = state::in_flight()? {
        if held.gate == declaration.gate
            && held.tries < TRIES
            && crate::agent::transcript(&held.session).is_some()
        {
            return Ok(Round {
                opening: Opening::Interrupted,
                session: crate::agent::Session::resumed(&held.session),
                tries: held.tries + 1,
            });
        }
    }
    Ok(match prior_session(declaration, steps) {
        Some(session) => Round {
            opening: Opening::Again,
            session: crate::agent::Session::resumed(&session),
            tries: 1,
        },
        None => Round {
            opening: Opening::First,
            session: crate::agent::Session::opened(),
            tries: 1,
        },
    })
}

// A resumed reviewer holds the aim, the criteria and the ladder; one starting fresh is told them again. The tool chose which, so there is nothing here to detect.
fn review(
    declaration: &Declaration,
    agent: &crate::agent::Agent,
    intent: &str,
    round: &Round,
    ceiling: std::time::Duration,
) -> Result<(Vec<Verdict>, String), String> {
    let system = crate::brief::system(declaration)?;
    let prompt = match round.opening {
        Opening::First => crate::brief::opening(intent),
        Opening::Again => crate::brief::continuing(),
        Opening::Interrupted => crate::brief::resuming(),
    };
    let answer = agent
        .run(
            crate::agent::Role::Review,
            &system,
            &prompt,
            &round.session,
            declaration.model.as_deref(),
            ceiling,
        )
        .map_err(|said| declared_model_fault(declaration, &said))?;
    let verdicts = crate::runner::verdicts(&answer, declaration.brief.simple)?;
    Ok((verdicts, crate::runner::findings(&answer.text)))
}

// A model the agent will not answer for is the hook's wiring, not this commit's, and no dev agent is going to resolve it by trying again or by choosing another. Naming the declaration is what turns a reviewer error into the maintenance it is.
fn declared_model_fault(declaration: &Declaration, said: &str) -> String {
    let Some(model) = &declaration.model else {
        return said.to_string();
    };
    let hook = git::hook_path().unwrap_or_else(|_| "the commit-msg hook".to_string());
    format!(
        "gate '{}' declares --model {model}, and the agent answered:\n  {said}\nThe declaration is in {hook}. Changing it is maintenance, committed with --no-verify.",
        declaration.gate
    )
}

fn trailers(hook: &Hook, steps: &[state::Step]) -> Result<Vec<String>, String> {
    let resets = state::resets()?;
    let mut lines = Vec::new();
    // One line per gate, from its last verdict, and only while its files are still in the commit.
    for declaration in &hook.gates {
        if !settled(declaration, steps) || !applies(declaration)? {
            continue;
        }
        let Some(step) = latest(&declaration.gate, steps) else {
            continue;
        };
        let Some(record) = state::lookup(&step.token)? else {
            continue;
        };
        for verdict in record.verdicts {
            let verdict = Verdict { resets, ..verdict };
            lines.push(trailer::render(&record.gate, &verdict));
        }
    }
    if lines.is_empty() {
        // The commit that just landed is the common way here: HEAD moved, the diary it was keyed on went with it, and there is nothing left to review.
        if git::staged(&[])?.is_empty() {
            return Err("nothing staged: nothing to review, nothing to commit".to_string());
        }
        return Err(format!(
            "{} declared no gate this commit reaches",
            hook.path
        ));
    }
    Ok(lines)
}

// The subject is the intent verbatim: what the change sets out to do is the one line both the reviewer and the record need, and it is already written by the time a review runs.
fn compose(intent: &str, trailers: &[String], resets: &[String]) -> String {
    let mut message = format!("{intent}\n");
    for reason in resets {
        message.push_str(&format!("\nReset: {reason}\n"));
    }
    if trailers.is_empty() {
        return message;
    }
    message.push('\n');
    message.push_str(&trailers.join("\n"));
    message.push('\n');
    message
}

// Nothing hands a token to anyone: the last run writes the trailers itself, and the hook it triggers verifies them exactly as it would verify a commit made by hand.
fn land(hook: &Hook, steps: &[state::Step], intent: &str) -> Result<bool, String> {
    let trailers = trailers(hook, steps)?;
    report::gates(&survey(hook, steps)?);
    let message = compose(intent, &trailers, &state::reasons()?);
    let out = git::commit(&message)?;
    report::committed(&trailers, &out);
    Ok(true)
}

pub fn run(asked: Option<&str>, ceiling: std::time::Duration) -> Result<bool, String> {
    let hook = declarations::read()?;
    // Every gate asked at once, before the first review rather than before each: a run that pays for one gate and then refuses at the next has spent the money either way.
    let mut drifting: Vec<String> = Vec::new();
    for declaration in &hook.gates {
        for file in git::unstaged(&declaration.paths)? {
            if !drifting.contains(&file) {
                drifting.push(file);
            }
        }
    }
    if !drifting.is_empty() {
        report::drifted(&drifting);
        return Ok(false);
    }
    // Asked before a review is paid for, and refused for the same reason the gate refuses it at commit time.
    let staged_machinery = gate::machinery_staged()?;
    if !staged_machinery.is_empty() {
        report::maintenance(&staged_machinery);
        return Ok(false);
    }
    let steps = state::progress()?;
    let recorded = state::intent()?;
    // Stated once, and it does not move: an aim restated is an aim that can drift, and what the first reviewer was briefed against is what the rest are judged by.
    let intent = match (asked, recorded.as_deref()) {
        (None, Some(held)) => held.to_string(),
        (Some(asked), None) => {
            let judge = crate::runner::configured()?;
            report::judging();
            let answer = judge.run(
                crate::agent::Role::JudgeIntent,
                &crate::brief::judge_system(),
                &crate::brief::judge_prompt(asked),
                // Its own session, opened and finished within this one question: there is nothing here worth resuming, and nothing a later round would want from it.
                &crate::agent::Session::opened(),
                None,
                ceiling,
            )?;
            crate::runner::judge(&answer, asked)?;
            state::set_intent(asked)?;
            asked.to_string()
        }
        (Some(_), Some(held)) => {
            return Err(format!(
                "this commit already states its aim, and it does not move:\n  {held}\nattest takes no --intent after the first run."
            ))
        }
        (None, None) => {
            return Err("attest needs --intent: no aim is recorded for this commit yet".to_string())
        }
    };
    let intent = intent.as_str();
    let Some(declaration) = next(&hook, &steps)? else {
        return land(&hook, &steps, intent);
    };
    let agent = crate::runner::configured()?;
    let round = round_for(declaration, &steps)?;
    if matches!(round.opening, Opening::Interrupted) {
        report::resuming(
            &declaration.gate,
            round.session.id(),
            crate::agent::last_wrote(round.session.id()),
        );
    }
    // Written down before the reviewer is spawned, which is the whole point of choosing the session here: after this line, a run that dies leaves something that names what it was doing and what to take up.
    state::open_round(&declaration.gate, round.session.id(), round.tries)?;
    report::reviewing(&declaration.gate, round.session.id());
    let (verdicts, findings) = review(declaration, &agent, intent, &round, ceiling)?;
    let blocked = verdicts.iter().any(Verdict::blocks);
    state::record(&declaration.gate, &verdicts, blocked)?;
    let after = state::progress()?;
    let remaining = next(&hook, &after)?.map(|d| d.gate.clone());
    report::reviewed(
        &declaration.gate,
        &verdicts,
        blocked,
        remaining.as_deref(),
        &findings,
        &survey(&hook, &after)?,
    );
    Ok(!blocked)
}

pub fn reset(reason: &str) -> Result<bool, String> {
    let count = state::log_reset(reason)?;
    report::reset_done(count, reason);
    Ok(true)
}
