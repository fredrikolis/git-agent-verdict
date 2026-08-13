// Concern: driving a commit to a full set of verdicts — which gate runs next | Non-concern: the trailer's grammar, or how a gate is later checked | IO: (intent) -> reviews, commit

use crate::declarations::{self, Declaration, Hook};
use crate::gate;
use crate::git;
use crate::report;
use crate::state;
use crate::trailer::{self, Verdict};

// A gate with nothing staged is not part of this commit, and neither is one whose own measure is the whole of what is staged — exactly as the gate itself decides when the hook runs.
fn applies(declaration: &Declaration) -> Result<bool, String> {
    if git::staged(&declaration.paths)?.is_empty() {
        return Ok(false);
    }
    let state = gate::measure_state(&declaration.docs, &declaration.paths)?;
    Ok(!matches!(state, gate::Measure::Alone(_)))
}

// The last word on a gate, which is the only one that counts: a re-review supersedes what it was asked to look at again.
fn latest<'a>(gate: &str, steps: &'a [state::Step]) -> Option<&'a state::Step> {
    steps.iter().rfind(|s| s.gate == gate)
}

// Content moves because the author is fixing what the review named — that is the work, not an anomaly. The gate stays open until a verdict describes what will be committed.
fn settled(declaration: &Declaration, steps: &[state::Step]) -> Result<bool, String> {
    let Some(step) = latest(&declaration.gate, steps) else {
        return Ok(false);
    };
    if step.blocked {
        return Ok(false);
    }
    Ok(state::content_digest(&declaration.paths)? == step.content)
}

// Line order is review order, and a later gate must never be judged against content an earlier one is still changing: the position is held here so nothing has to sequence it by hand.
fn next<'a>(hook: &'a Hook, steps: &[state::Step]) -> Result<Option<&'a Declaration>, String> {
    for declaration in &hook.gates {
        if !applies(declaration)? || settled(declaration, steps)? {
            continue;
        }
        return Ok(Some(declaration));
    }
    Ok(None)
}

// The session the last reviewer reported. A runner that can resume one reads what changed, rather than sampling a rubric afresh every round.
fn prior_session(declaration: &Declaration, steps: &[state::Step]) -> Option<String> {
    let step = latest(&declaration.gate, steps)?;
    let record = state::lookup(&step.token).ok()??;
    let session = record.verdicts.first()?.session.clone();
    Some(session).filter(|s| !s.is_empty())
}

// An intent may only change once a MAJOR has sent the work back; anywhere else a changed brief is the review leaking into what the next reviewer is told.
fn hold_intent(intent: &str, steps: &[state::Step]) -> Result<(), String> {
    let sent_back = steps.last().is_some_and(|s| s.blocked);
    if let Some(previous) = state::intent()? {
        if previous != intent && !sent_back {
            let detail = "the intent may only change after a MAJOR: state the same aim, or reset with a reason";
            return Err(detail.to_string());
        }
    }
    state::set_intent(intent)
}

fn review(
    declaration: &Declaration,
    runner: &crate::runner::Runner,
    intent: &str,
    prior: Option<String>,
) -> Result<(Vec<Verdict>, String), String> {
    let files = git::staged_existing(&declaration.paths)?;
    let brief = crate::brief::compose(declaration, Some(intent), &files)?;
    report::reviewing(&declaration.gate, prior.is_some());
    let output = crate::runner::invoke(runner, &brief, prior.as_deref())?;
    let verdicts = crate::runner::verdicts(&output, declaration.brief.simple)?;
    Ok((verdicts, crate::runner::findings(&output)))
}

fn trailers(hook: &Hook, steps: &[state::Step]) -> Result<Vec<String>, String> {
    let resets = state::resets()?;
    let mut lines = Vec::new();
    // One line per gate, from its last verdict, and only where that verdict still describes what is staged: a superseded review and one whose files have left the commit both attest nothing about it.
    for declaration in &hook.gates {
        if !settled(declaration, steps)? {
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
        // A commit that is only the measure carries no verdict: no gate can judge it without judging it by itself, and there is nothing else here to judge.
        for declaration in &hook.gates {
            let state = gate::measure_state(&declaration.docs, &declaration.paths)?;
            if matches!(state, gate::Measure::Alone(_)) {
                return Ok(lines);
            }
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

// Every staged path no gate read, with the reason. The two are not the same news: a gate declining to judge its own measure is the design; a path no pathspec reaches is a hole in the wiring.
fn unreviewed(hook: &Hook, steps: &[state::Step]) -> Result<Vec<report::Unread>, String> {
    let mut unread = Vec::new();
    for file in git::staged(&[])? {
        let mut judged_by = None;
        let mut reviewed = false;
        for declaration in &hook.gates {
            if !git::staged(&declaration.paths)?.contains(&file) {
                continue;
            }
            if steps
                .iter()
                .any(|s| !s.blocked && s.gate == declaration.gate)
            {
                reviewed = true;
                break;
            }
            judged_by = Some(declaration.gate.clone());
        }
        if !reviewed {
            unread.push(report::Unread { file, judged_by });
        }
    }
    Ok(unread)
}

// Nothing hands a token to anyone: the last run writes the trailers itself, and the hook it triggers verifies them exactly as it would verify a commit made by hand.
fn land(hook: &Hook, steps: &[state::Step], intent: &str) -> Result<bool, String> {
    let trailers = trailers(hook, steps)?;
    report::unreviewed(&unreviewed(hook, steps)?);
    let message = compose(intent, &trailers, &state::reasons()?);
    let out = git::commit(&message)?;
    report::committed(&trailers, &out);
    Ok(true)
}

pub fn run(intent: &str) -> Result<bool, String> {
    let hook = declarations::read()?;
    // The gate refuses a mixed commit at commit time regardless, and attest pays for the reviews in between — so it asks first, per gate, since only the gate whose measure is moving cannot judge.
    for declaration in &hook.gates {
        if let gate::Measure::Mixed(rubrics) =
            gate::measure_state(&declaration.docs, &declaration.paths)?
        {
            report::circular(&declaration.gate, &rubrics);
            return Ok(false);
        }
    }
    let steps = state::progress()?;
    hold_intent(intent, &steps)?;
    let Some(declaration) = next(&hook, &steps)? else {
        return land(&hook, &steps, intent);
    };
    let runner = crate::runner::configured()?;
    let prior = prior_session(declaration, &steps);
    let (verdicts, findings) = review(declaration, &runner, intent, prior)?;
    let blocked = verdicts.iter().any(Verdict::blocks);
    let content = state::content_digest(&declaration.paths)?;
    state::record(&declaration.gate, &verdicts, blocked, &content)?;
    let remaining = next(&hook, &state::progress()?)?.map(|d| d.gate.clone());
    report::reviewed(
        &declaration.gate,
        &verdicts,
        blocked,
        remaining.as_deref(),
        &findings,
    );
    Ok(!blocked)
}

pub fn reset(reason: &str) -> Result<bool, String> {
    let count = state::log_reset(reason)?;
    report::reset_done(count, reason);
    Ok(true)
}
