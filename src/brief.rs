// Concern: what a gate tells its reviewer — the standing prompt, and the line opening one review | Non-concern: what the tool tells its author | IO: (declaration, intent) -> system, prompt

use crate::declarations::Declaration;
use crate::runner::MARKER;
use crate::trailer::{ADVISORY_SHAPE, COUNTS_SHAPE};

const TEMPLATE: &str = include_str!("prompt.md");

// Shipped in the binary, not fetched, so a trailer always attests a reconstructable rubric. Generated at build time from standards/*.md (see build.rs).
include!(concat!(env!("OUT_DIR"), "/standards.rs"));

pub fn shipped(name: &str) -> Option<&'static str> {
    SHIPPED
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, text)| *text)
}

// Taken from each file's own first line rather than restated here, so the description can't go stale.
pub fn shipped_listing() -> String {
    SHIPPED
        .iter()
        .map(|(name, text)| {
            let annotation = text
                .lines()
                .next()
                .unwrap_or_default()
                .trim_start_matches("<!--")
                .trim_end_matches("-->")
                .trim();
            format!("{name}\n    {annotation}")
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

pub fn shipped_names() -> String {
    SHIPPED
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

// The whole of what --simple changes: an advisory gate has no MAJOR rung, so it's absent, not shown-and-zeroed.
const SEVERITY: &str = "MAJOR — blocks the commit, and is reviewed again.
  The work is wrong, or has a severe flaw. An incremental fix will not reach the right answer.

MODERATE — must fix, no re-review.
  The outcome is right, the execution is not. The author fixes it. Nobody checks the fix.

MINOR — optional: fix it, or leave it. Recorded either way.

Grade by what is wrong, not by what the fix costs.
A MODERATE rounded up to MAJOR sends back work that is already right.
A MAJOR rounded down to MODERATE leaves a defect nobody has to fix.";

const SEVERITY_ADVISORY: &str =
    "This gate has no MAJOR severity. Nothing you report blocks the commit, and there is no re-review.

MODERATE — must fix, no re-review.
  The outcome is right, the execution is not. The author fixes it. Nobody checks the fix.

MINOR — optional: fix it, or leave it. Recorded either way.

Grade by what is wrong, not by what the fix costs.
A MINOR rounded up to MODERATE makes work for the author that nobody asked for.";

const JUDGE: &str =
    "You judge one line of text. Do not review any code, and do not read the repository.

The <diff-intent> you are given states what a change does. Refuse it if it does any of these:
  - gives a reason the change is worth doing
  - defends the approach
  - says what it replaces
  - says what was already tried

Answer with exactly one line, and nothing after it:

  {{marker}} accepted

or, naming which of the four it does and quoting the words that do it:

  {{marker}} refused — <which one, and the words>
";

pub fn judge_system() -> String {
    JUDGE.replace("{{marker}}", MARKER)
}

pub fn judge_prompt(intent: &str) -> String {
    format!("<diff-intent>{intent}</diff-intent>\n")
}

// Only a built-in template carries an annotation line; eating an override's first line would be a silent edit.
fn built_in(text: &str) -> String {
    text.lines().skip(1).collect::<Vec<_>>().join("\n")
}

// Counts only: who reviewed and on what session are read from the agent, never asked of the model.
fn asked_of(simple: bool) -> &'static str {
    if simple {
        ADVISORY_SHAPE
    } else {
        COUNTS_SHAPE
    }
}

// Whole exists because a rubric that just changed has never been read against the tree, and no diff will ever show that.
#[derive(Clone, Copy)]
pub enum Reach {
    Diff,
    Whole,
}

impl Reach {
    fn command(self, paths: &[String]) -> String {
        match self {
            Reach::Diff => format!("git diff --cached -- {}", quoted(paths)),
            Reach::Whole => format!("git ls-files -- {}", quoted(paths)),
        }
    }

    fn subject(self) -> &'static str {
        match self {
            Reach::Diff => "that diff",
            Reach::Whole => "every file it lists",
        }
    }

    fn rule(self) -> &'static str {
        match self {
            Reach::Diff => "Judge the diff and what it affects, not only the edited lines. Only the staged change is under review, and the working tree may hold edits that are not part of it.",
            Reach::Whole => "Judge each file as it stands. Nothing here is a change, and there is no diff to read.",
        }
    }
}

// A reviewer told plainly it can't write: otherwise it spends its round arguing with the harness's silent refusals.
const SANDBOX: &str = "Do not change the working tree. To test something, copy the repo to a temp directory and change it there. Confirm with `git diff --stat` before you answer.";
const NO_SANDBOX: &str = "This session cannot write anywhere, and every attempt will be refused. Confirm a suspicion by reading. One you cannot confirm that way is a guess: leave it out.";

fn quoted(paths: &[String]) -> String {
    let quoted: Vec<String> = paths
        .iter()
        .map(|p| format!("'{}'", p.replace('\'', r"'\''")))
        .collect();
    quoted.join(" ")
}

pub fn unknown_standard(name: &str) -> String {
    format!("--standard {name}: this build ships {}", shipped_names())
}

// Content is read in, not pointed at by path — a path is something a reviewer may skim or skip.
fn criteria(declaration: &Declaration) -> Result<String, String> {
    let mut out = String::new();
    for name in &declaration.standards {
        let text = shipped(name).ok_or_else(|| unknown_standard(name))?;
        out.push_str(&format!(
            "<document title=\"{name}\">\n{}\n</document>\n",
            text.trim_end()
        ));
    }
    for doc in &declaration.docs {
        let text = std::fs::read_to_string(doc).map_err(|e| format!("--doc {doc}: {e}"))?;
        let title = std::path::Path::new(doc)
            .file_name()
            .map_or_else(|| doc.clone(), |n| n.to_string_lossy().into_owned());
        out.push_str(&format!(
            "<document title=\"{title}\">\n{}\n</document>\n",
            text.trim_end()
        ));
    }
    for (n, rule) in declaration.rules.iter().enumerate() {
        let n = n + 1;
        out.push_str(&format!("<inline-rule-{n}>{rule}</inline-rule-{n}>\n"));
    }
    Ok(out)
}

pub fn system(declaration: &Declaration, reach: Reach) -> Result<String, String> {
    let template = match &declaration.brief.prompt {
        Some(path) => {
            std::fs::read_to_string(path).map_err(|e| format!("--override-prompt {path}: {e}"))?
        }
        None => built_in(TEMPLATE),
    };
    let severity = if declaration.brief.simple {
        SEVERITY_ADVISORY
    } else {
        SEVERITY
    };
    Ok(template
        .replace("{{gate}}", &declaration.gate)
        .replace("{{criteria}}", &criteria(declaration)?)
        .replace("{{scope}}", &reach.command(&declaration.paths))
        .replace("{{subject}}", reach.subject())
        .replace("{{reach}}", reach.rule())
        .replace(
            "{{sandbox}}",
            if declaration.read_only {
                NO_SANDBOX
            } else {
                SANDBOX
            },
        )
        .replace("{{severity}}", severity)
        .replace("{{marker}}", MARKER)
        .replace("{{shape}}", asked_of(declaration.brief.simple)))
}

pub fn opening(intent: &str) -> String {
    format!(
        "<diff-intent>{intent}</diff-intent>\n\nExecute the review per the instructions above.\n"
    )
}

pub fn sweeping() -> String {
    "A standard this gate judges by has changed. Review the repository as it now stands against it.\n\
     There is no commit and no diff. Report what the standard requires, and close with your verdict line.\n"
        .to_string()
}

pub fn continuing() -> String {
    "Fixes incorporated, re-review requested.\n".to_string()
}

// Not a re-review: this round was cut short and is being taken up where it stopped, not asked to judge changes nobody made.
pub fn resuming() -> String {
    "Your review was interrupted before you reported it. Nothing has changed since.\n\
     Continue from where you stopped, redoing only what you had not finished, and close with your verdict line.\n"
        .to_string()
}
