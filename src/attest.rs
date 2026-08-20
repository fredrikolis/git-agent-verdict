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
}

fn round_for(declaration: &Declaration, steps: &[state::Step]) -> Result<Round, String> {
    if let Some(held) = state::in_flight()? {
        // A marker outlives only a run that never got to clear it, or a resumed round is not what this is. Its reviewer still holds everything it had read.
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

// A resumed reviewer holds the aim, the criteria and the ladder; one starting fresh is told them again. The tool chose which, so there is nothing here to detect.
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

// Only what the agent itself answers, or fails to. Everything a gate's own wiring can get wrong is settled before this is called, so a failure here is a failure of the round and nothing else.
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
            "{} declares no gate matching this commit",
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

// Nothing hands a token to anyone: the last run writes the trailers itself, and the hook it triggers verifies them as it would a commit made by hand. The intent is asked for after them, because an empty index and a hook that matches nothing are both worth saying before a missing intent is.
fn land(hook: &Hook, steps: &[state::Step], intent: Option<&str>) -> Result<bool, String> {
    let trailers = trailers(hook, steps)?;
    let intent = intent.ok_or("this commit has no accepted intent")?;
    // No gate table: the trailers carry the same gates and the same counts, and the commit keeps them.
    let message = compose(intent, &trailers, &state::reasons()?);
    let out = git::commit(&message)?;
    report::committed(&trailers, &out);
    Ok(true)
}

// The caller's half: everything that can refuse before money is spent, then a round it does not wait for.
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
    // Asked before a round is spawned: a host with no reviewer configured is the caller's own wiring, and learning it through await would cost a round to say nothing.
    crate::runner::configured()?;
    // A review already running is a refusal, not an answer: this one did not start, and what is running was briefed on whatever was staged then rather than now.
    let held = crate::lock::take()?;
    let steps = state::progress()?;
    let recorded = state::intent()?;
    let proposed = state::proposed()?;
    // Stated once, and it does not move: an aim restated is an aim that can drift, and what the first reviewer was briefed against is what the rest are judged by. Written down before it is judged, so a round killed mid-answer leaves it neither forgotten nor standing as accepted.
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
    // Through, and not committed: the findings under a passing verdict are read before they are carried, and the verb that carries them is the author's. Refused rather than shrugged at, because a caller that keeps attesting has not read them — and through is not the same answer as nothing to review, which is what an empty index gives.
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
    // Started, and not waited for: a caller that blocks is a caller an agent wraps in a background shell and then polls. The verdict is `await`'s to report.
    Ok(true)
}

// Nothing lands by itself. What this refuses is a commit asked for before the gates are through, which is the mistake the flow exists to prevent.
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
        // The reviews belonged to the commit that just landed. Left behind, the next await would answer for a commit nobody is writing.
        crate::round::forget_last();
    }
    landed
}

// Ending a round abandons the aim it was judging with it: the next caller states one afresh rather than inheriting one nobody answered.
pub fn abandon() -> Result<bool, String> {
    crate::round::abort(|| {
        state::close_round();
        let _ = state::settle_intent(false);
        // The hook is read for the table and nothing else, so a repo that has none still aborts.
        declarations::read()
            .and_then(|hook| survey(&hook, &state::progress()?))
            .unwrap_or_default()
    })
}

// Every gate the commit still needs, one after another, stopping at the first that blocks: after a MAJOR the content under the gates behind it is about to change, so reviewing them now buys verdicts on text nobody is keeping.
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
    // Built before the marker: a rubric that will not open or a prompt file that is missing is the hook's wiring, and discovering it afterwards would spend an interrupted round's one resume on a fault the reviewer never saw.
    let (system, prompt) = briefing(declaration, intent, &opened)?;
    // Written down before the reviewer is spawned: after this line, a round that dies leaves something naming what it was doing and what to take up.
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
