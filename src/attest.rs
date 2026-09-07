// Concern: starting the review the commit being written needs next, and creating the commit once every gate has passed | Non-concern: conducting a review | IO: (intent) -> review, commit

use crate::declarations::{self, Declaration, Hook};
use crate::gate;
use crate::git;
use crate::report;
use crate::standing::{applies, latest, next, settled, survey};
use crate::state;
use crate::trailer::{self, Verdict};

fn session_of(step: &state::Step) -> Option<String> {
    let record = state::lookup(&step.token).ok()??;
    Some(record.verdicts.first()?.session.clone()).filter(|s| !s.is_empty())
}

fn prior_session(declaration: &Declaration, steps: &[state::Step]) -> Option<String> {
    session_of(latest(&declaration.gate, steps)?)
}

// A resumed reviewer told the wrong one of these reports on fixes nobody made.
enum Opening {
    First,
    Again,
    Interrupted,
}

struct Round {
    opening: Opening,
    session: crate::agent::Session,
}

fn round_for(declaration: &Declaration, steps: &[state::Step]) -> Result<Round, String> {
    if let Some(held) = state::in_flight()? {
        if held.gate == declaration.gate && crate::agent::transcript(&held.session).is_some() {
            return Ok(Round {
                opening: Opening::Interrupted,
                session: crate::agent::Session::resumed(&held.session),
            });
        }
    }
    Ok(match prior_session(declaration, steps) {
        Some(session) => Round {
            opening: Opening::Again,
            session: crate::agent::Session::resumed(&session),
        },
        None => Round {
            opening: Opening::First,
            session: crate::agent::Session::opened(),
        },
    })
}

fn briefing(
    declaration: &Declaration,
    intent: &str,
    round: &Round,
) -> Result<(String, String), String> {
    let system = crate::brief::system(declaration, crate::brief::Reach::Diff)?;
    let prompt = match round.opening {
        Opening::First => crate::brief::opening(intent),
        Opening::Again => crate::brief::continuing(),
        Opening::Interrupted => crate::brief::resuming(),
    };
    Ok((system, prompt))
}

fn review(
    declaration: &Declaration,
    agent: &crate::agent::Agent,
    (system, prompt): (&str, &str),
    round: &Round,
    ceiling: std::time::Duration,
) -> Result<(Vec<Verdict>, String), String> {
    let answer = agent
        .run(
            crate::agent::Role::Review,
            system,
            prompt,
            &round.session,
            &crate::agent::Terms {
                model: declaration.model.as_deref(),
                ceiling,
                read_only: declaration.read_only,
            },
        )
        .map_err(|said| declared_model_fault(declaration, &said))?;
    let verdicts = crate::runner::verdicts(&answer, declaration.brief.simple)?;
    Ok((verdicts, crate::runner::findings(&answer.text)))
}

// A model the agent won't answer for is the hook's wiring, not this commit's — naming the declaration turns it into the maintenance it is.
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
        // Common case: the commit already landed, HEAD moved, and the diary it was keyed on went with it.
        if git::staged(&[])?.is_empty() {
            return Err("nothing staged: nothing to review, nothing to commit".to_string());
        }
        return Err(format!(
            "{} declares no gate matching this commit",
            hook.path
        ));
    }
    Ok(lines)
}

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

// No token handoff: the commit-msg hook verifies the trailers this writes as it would one made by hand.
fn land(hook: &Hook, steps: &[state::Step], intent: Option<&str>) -> Result<bool, String> {
    let trailers = trailers(hook, steps)?;
    let intent = intent.ok_or("this commit has no accepted intent")?;
    let message = compose(intent, &trailers, &state::reasons()?);
    let out = git::commit(&message)?;
    report::committed(&trailers, &out);
    Ok(true)
}

pub fn run(
    asked: Option<&str>,
    ceiling: std::time::Duration,
    staged_only: bool,
) -> Result<bool, String> {
    let hook = declarations::read()?;
    // Worth refusing: the reviewer reads the working tree, the commit records the index, and staged-then-edited means those differ.
    let mut drifting: Vec<String> = Vec::new();
    for declaration in &hook.gates {
        let staged = git::staged(&declaration.paths)?;
        for file in git::unstaged(&declaration.paths)? {
            if staged.contains(&file) && !drifting.contains(&file) {
                drifting.push(file);
            }
        }
    }
    if !drifting.is_empty() && !staged_only {
        report::drifted(&drifting);
        return Ok(false);
    }
    let staged_machinery = gate::machinery_staged()?;
    if !staged_machinery.is_empty() {
        report::maintenance(&staged_machinery);
        return Ok(false);
    }
    crate::runner::configured()?;
    let held = crate::lock::take()?;
    let steps = state::progress()?;
    let recorded = state::intent()?;
    let proposed = state::proposed()?;
    // Fixed once stated: every reviewer must be judged against the same aim the first one was briefed with.
    match (asked, recorded.as_deref(), proposed.as_deref()) {
        (Some(_), Some(held), _) => {
            return Err(format!(
                "this commit already has an accepted intent, which is fixed:\n  {held}\nattest takes no --intent after the first run."
            ))
        }
        (Some(asked), None, Some(standing)) if standing != asked => {
            return Err(format!(
                "this commit already has a proposed intent, which is fixed:\n  {standing}\nRun attest with no --intent, or reset to state another."
            ))
        }
        (Some(asked), None, _) => state::propose(asked)?,
        (None, None, None) => {
            return Err("attest needs --intent: no intent is recorded for this commit".to_string())
        }
        _ => {}
    }
    // "Through" isn't "committed": passing findings are carried by the author's own commit verb, not this one.
    if next(&hook, &steps)?.is_none() {
        if git::staged(&[])?.is_empty() {
            return Err("nothing staged: nothing to review, nothing to commit".to_string());
        }
        if !survey(&hook, &steps)?
            .iter()
            .any(|(_, standing)| !matches!(standing, report::Standing::Skipped(_)))
        {
            return Err(format!(
                "{} declares no gate matching this commit",
                hook.path
            ));
        }
        report::what_was_reviewed();
        report::all_passed();
        return Ok(true);
    }
    let started = crate::round::spawn(held, "attest", ceiling, move |round| {
        review_all(&hook, round, ceiling)
    })?;
    report::started(&started);
    Ok(true)
}

pub fn commit() -> Result<bool, String> {
    let hook = declarations::read()?;
    let held = crate::lock::take()?;
    let steps = state::progress()?;
    if let Some(declaration) = next(&hook, &steps)? {
        report::what_was_reviewed();
        return Err(report::not_passed(&declaration.gate));
    }
    held.describe(&crate::lock::Landed::Landing)?;
    let landed = land(&hook, &steps, state::intent()?.as_deref());
    if landed.is_ok() {
        // Left behind, the next await would answer for a commit nobody is writing.
        crate::round::forget_last();
    }
    landed
}

pub fn abandon() -> Result<bool, String> {
    crate::round::abort(|| {
        state::close_round();
        let _ = state::settle_intent(false);
        declarations::read()
            .and_then(|hook| survey(&hook, &state::progress()?))
            .unwrap_or_default()
    })
}

// Stops at the first block: past a MAJOR, gates behind it are about to change, so reviewing them now verdicts text nobody keeps.
fn review_all(
    hook: &Hook,
    round: &crate::round::Round,
    ceiling: std::time::Duration,
) -> Result<crate::round::Outcome, String> {
    let intent = accepted_intent(ceiling)?;
    loop {
        let steps = state::progress()?;
        let Some(declaration) = next(hook, &steps)? else {
            return Ok(crate::round::Outcome::Clean);
        };
        if review_one(hook, round, declaration, &steps, &intent, ceiling)? {
            return Ok(crate::round::Outcome::Blocked);
        }
    }
}

// Judged once for the commit, not once per gate: every reviewer is briefed against the same aim.
fn accepted_intent(ceiling: std::time::Duration) -> Result<String, String> {
    let intent = match (state::intent()?, state::proposed()?) {
        (Some(accepted), _) => accepted,
        (None, Some(asked)) => {
            let judge = crate::runner::configured()?;
            report::judging();
            let answer = judge.run(
                crate::agent::Role::JudgeIntent,
                &crate::brief::judge_system(),
                &crate::brief::judge_prompt(&asked),
                // Its own session, opened and finished within this one question: there is nothing here worth resuming, and nothing a later round would want from it.
                &crate::agent::Session::opened(),
                &crate::agent::Terms {
                    model: None,
                    ceiling,
                    read_only: false,
                },
            )?;
            let judged = crate::runner::judge(&answer, &asked);
            state::settle_intent(judged.is_ok())?;
            judged?;
            asked
        }
        (None, None) => {
            return Err("no intent was recorded for this review to validate".to_string())
        }
    };
    Ok(intent)
}

// One gate, and whether it blocked.
fn review_one(
    hook: &Hook,
    round: &crate::round::Round,
    declaration: &Declaration,
    steps: &[state::Step],
    intent: &str,
    ceiling: std::time::Duration,
) -> Result<bool, String> {
    let agent = crate::runner::configured()?;
    round.at_gate(&declaration.gate);
    let opened = round_for(declaration, steps)?;
    if matches!(opened.opening, Opening::Interrupted) {
        report::resuming(
            &declaration.gate,
            opened.session.id(),
            crate::agent::last_wrote(opened.session.id()),
        );
    }
    // Built before the marker: a wiring fault here shouldn't spend an interrupted round's one resume.
    let (system, prompt) = briefing(declaration, intent, &opened)?;
    // Written before the reviewer spawns, so a round that dies still leaves a marker naming what it was doing.
    state::open_round(&declaration.gate, opened.session.id())?;
    report::reviewing(
        &declaration.gate,
        opened.session.id(),
        crate::agent::transcript_path(opened.session.id()).as_deref(),
    );
    crate::signals::say(&format!(
        "while reviewing {}, session {}.",
        declaration.gate,
        opened.session.id()
    ));
    let reviewed = review(declaration, &agent, (&system, &prompt), &opened, ceiling);
    crate::signals::quiet();
    // One attempt at taking a round up. If the resumed reviewer fails too, the session is not one this tool can finish, and every run from here would pay again to learn that.
    if reviewed.is_err() && matches!(opened.opening, Opening::Interrupted) {
        state::close_round();
    }
    let (verdicts, findings) = reviewed?;
    let blocked = verdicts.iter().any(Verdict::blocks);
    state::record(&declaration.gate, &verdicts, blocked)?;
    let after = state::progress()?;
    let remaining = next(hook, &after)?.map(|d| d.gate.clone());
    report::reviewed(
        round.dir(),
        &declaration.gate,
        &verdicts,
        blocked,
        remaining.as_deref(),
        &findings,
        &survey(hook, &after)?,
    );
    Ok(blocked)
}

pub fn reset(reason: &str) -> Result<bool, String> {
    let count = state::log_reset(reason)?;
    crate::round::abandon_logs();
    report::reset_done(count, reason);
    Ok(true)
}
