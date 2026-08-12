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
) -> Result<Vec<Verdict>, String> {
    let files = git::staged_existing(&declaration.paths)?;
    let brief = report::prompt(declaration, Some(intent), &files)?;
    report::reviewing(&declaration.gate, &runner.cmd);
    let output = crate::runner::invoke(runner, &brief)?;
    let verdicts = crate::runner::verdicts(&output, declaration.brief.simple)?;
    Ok(verdicts)
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

// Nothing hands a token to anyone: the last run writes the trailers itself, and the hook it triggers verifies them exactly as it would verify a commit made by hand.
fn land(hook: &Hook, steps: &[state::Step], intent: &str) -> Result<bool, String> {
    let trailers = trailers(hook, steps)?;
    let message = compose(intent, &trailers, &state::reasons()?);
    let out = git::commit(&message)?;
    report::committed(&trailers, &out);
    Ok(true)
}

pub fn run(intent: &str) -> Result<bool, String> {
    let hook = declarations::read()?;
    let steps = state::progress()?;
    hold_intent(intent, &steps)?;
    let Some(declaration) = next(&hook, &steps)? else {
        return land(&hook, &steps, intent);
    };
    let runner = crate::runner::configured()?;
    let verdicts = review(declaration, &runner, intent)?;
    let blocked = !declaration.brief.simple && verdicts.iter().any(Verdict::blocks);
    state::record(&declaration.gate, &verdicts, blocked)?;
    let remaining = next(&hook, &state::progress()?)?.map(|d| d.gate.clone());
    report::reviewed(&declaration.gate, &verdicts, blocked, remaining.as_deref());
    Ok(!blocked)
}

pub fn reset(reason: &str) -> Result<bool, String> {
    let count = state::log_reset(reason)?;
    report::reset_done(count, reason);
    Ok(true)
}
