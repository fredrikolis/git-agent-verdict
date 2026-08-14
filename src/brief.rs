// Concern: what a gate tells its reviewer — standing instructions, and the line opening one round | Non-concern: what the tool tells its author | IO: (declaration, intent) -> system, prompt

use crate::declarations::Declaration;
use crate::runner::MARKER;
use crate::trailer::{ADVISORY_SHAPE, COUNTS_SHAPE};

const TEMPLATE: &str = include_str!("prompt.md");

// The whole of what --simple changes: an advisory gate has no MAJOR rung, so the rung is absent rather than shown and annotated away. The tool reports its zero, and the trailer keeps one shape everywhere.
const LADDER: &str = "MAJOR — blocks the commit, and is reviewed again.
  The work is wrong, or has a severe flaw. An incremental fix will not reach the right answer.

MODERATE — must fix, no re-review.
  The outcome is right, the execution is not. The author fixes it. Nobody checks the fix.

MINOR — optional: fix it, or leave it. Recorded either way.

Grade by what is wrong, not by what the fix costs.
A MODERATE rounded up to MAJOR sends back work that is already right.
A MAJOR rounded down to MODERATE leaves a defect nobody has to fix.";

const LADDER_ADVISORY: &str =
    "This gate has no MAJOR rung. Nothing you report blocks the commit, and there is no re-review.

MODERATE — must fix, no re-review.
  The outcome is right, the execution is not. The author fixes it. Nobody checks the fix.

MINOR — optional: fix it, or leave it. Recorded either way.

Grade by what is wrong, not by what the fix costs.
A MINOR rounded up to MODERATE makes work for the author that nobody asked for.";

// Its own question, answered once per commit by a runner of its own: the check is on one line of text, not on the code, and it costs a reviewer nothing to have never been asked it.
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

// Standing instructions and the line to judge, split as a review's are: the same runner answers both, so the same two halves reach it either way.
pub fn judge_system() -> String {
    JUDGE.replace("{{marker}}", MARKER)
}

pub fn judge_prompt(intent: &str) -> String {
    format!("<diff-intent>{intent}</diff-intent>\n")
}

// Only a built-in template carries an annotation line; an override is a repo's own file, and eating its first line would be a silent edit.
fn built_in(text: &str) -> String {
    text.lines().skip(1).collect::<Vec<_>>().join("\n")
}

// The counts alone. Who reviewed and on what session are read from the agent, not asked of the model: it would be guessing at one and cannot know the other.
fn asked_of(simple: bool) -> &'static str {
    if simple {
        ADVISORY_SHAPE
    } else {
        COUNTS_SHAPE
    }
}

// Quoted for the shell the reviewer types it into: a pathspec is written to be globbed by git, not by that shell.
fn scope(paths: &[String]) -> String {
    let quoted: Vec<String> = paths
        .iter()
        .map(|p| format!("'{}'", p.replace('\'', r"'\''")))
        .collect();
    format!("git diff --cached -- {}", quoted.join(" "))
}

// Read in, not pointed at: a path is something a reviewer may skim or skip, and re-reads every round. Content sits apart from process so neither reads as a footnote to the other.
fn criteria(declaration: &Declaration) -> Result<String, String> {
    let mut out = String::new();
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

// Everything that does not change between rounds, or between commits: a runner that hands this to a system prompt pays for it once and reads it from cache after.
pub fn system(declaration: &Declaration) -> Result<String, String> {
    let template = match &declaration.brief.prompt {
        Some(path) => {
            std::fs::read_to_string(path).map_err(|e| format!("--override-prompt {path}: {e}"))?
        }
        None => built_in(TEMPLATE),
    };
    let ladder = if declaration.brief.simple {
        LADDER_ADVISORY
    } else {
        LADDER
    };
    Ok(template
        .replace("{{gate}}", &declaration.gate)
        .replace("{{criteria}}", &criteria(declaration)?)
        .replace("{{scope}}", &scope(&declaration.paths))
        .replace("{{ladder}}", ladder)
        .replace("{{marker}}", MARKER)
        .replace("{{shape}}", asked_of(declaration.brief.simple)))
}

// The one line a round adds. Everything else is standing instruction, so this is all that changes between rounds — and all a resumed reviewer has not already been told.
pub fn opening(intent: &str) -> String {
    format!(
        "<diff-intent>{intent}</diff-intent>\n\nExecute the review per the instructions above.\n"
    )
}

pub fn continuing() -> String {
    "Fixes incorporated, re-review requested.\n".to_string()
}
