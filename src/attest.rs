// Concern: driving a commit to a full set of verdicts — which gate runs next | Non-concern: the trailer's grammar, or how a gate is later checked | IO: (intent) -> reviews, commit

use crate::declarations::{self, Declaration, Hook};
use crate::git;
use crate::report;
use crate::state;
use crate::trailer::{self, Verdict};

// A gate with nothing staged is not part of this commit, exactly as the gate itself decides when the hook runs.
fn applies(declaration: &Declaration) -> Result<bool, String> {
    Ok(!git::staged(&declaration.paths)?.is_empty())
}

// Line order is review order, and a later gate must never be judged against content an earlier one is still changing: the position is held here so nothing has to sequence it by hand.
fn next<'a>(hook: &'a Hook, steps: &[state::Step]) -> Result<Option<&'a Declaration>, String> {
    let passed: Vec<&str> = steps
        .iter()
        .filter(|s| !s.blocked)
        .map(|s| s.gate.as_str())
        .collect();
    for declaration in &hook.gates {
        if passed.contains(&declaration.gate.as_str()) || !applies(declaration)? {
            continue;
        }
        return Ok(Some(declaration));
    }
    Ok(None)
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
) -> Result<(Vec<Verdict>, String), String> {
    let files = git::staged_existing(&declaration.paths)?;
    let brief = crate::brief::compose(declaration, Some(intent), &files)?;
    report::reviewing(&declaration.gate);
    let output = crate::runner::invoke(runner, &brief)?;
    let verdicts = crate::runner::verdicts(&output, declaration.brief.simple)?;
    Ok((verdicts, crate::runner::findings(&output)))
}

fn trailers(hook: &Hook, steps: &[state::Step]) -> Result<Vec<String>, String> {
    let resets = state::resets()?;
    let mut lines = Vec::new();
    for step in steps.iter().filter(|s| !s.blocked) {
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
    message.push('\n');
    message.push_str(&trailers.join("\n"));
    message.push('\n');
    message
}

// A verdict is evidence about the content its reviewer saw, and fixing what it named re-opens no gate. What moved since is said, not refused: silence lets a trailer claim more than its review covered.
fn moved_since_review(hook: &Hook, steps: &[state::Step]) -> Result<Vec<String>, String> {
    let mut moved = Vec::new();
    for step in steps.iter().filter(|s| !s.blocked) {
        let Some(declaration) = hook.gates.iter().find(|d| d.gate == step.gate) else {
            continue;
        };
        if state::content_digest(&declaration.paths)? != step.content {
            moved.push(step.gate.clone());
        }
    }
    Ok(moved)
}

// Nothing hands a token to anyone: the last run writes the trailers itself, and the hook it triggers verifies them exactly as it would verify a commit made by hand.
fn land(hook: &Hook, steps: &[state::Step], intent: &str) -> Result<bool, String> {
    let trailers = trailers(hook, steps)?;
    report::moved(&moved_since_review(hook, steps)?);
    let message = compose(intent, &trailers, &state::reasons()?);
    let out = git::commit(&message)?;
    report::committed(&trailers, &out);
    Ok(true)
}

pub fn run(intent: &str) -> Result<bool, String> {
    let hook = declarations::read()?;
    // The hook's preflight refuses this at commit time regardless; attest is what pays for the reviews in between, so it asks first.
    let docs: Vec<String> = hook.gates.iter().flat_map(|d| d.docs.clone()).collect();
    let rubrics = crate::gate::staged_rubrics(&docs)?;
    if !rubrics.is_empty() {
        report::preflight(&rubrics);
        return Ok(false);
    }
    let steps = state::progress()?;
    hold_intent(intent, &steps)?;
    let Some(declaration) = next(&hook, &steps)? else {
        return land(&hook, &steps, intent);
    };
    let runner = crate::runner::configured()?;
    let (verdicts, findings) = review(declaration, &runner, intent)?;
    let blocked = !declaration.brief.simple && verdicts.iter().any(Verdict::blocks);
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
