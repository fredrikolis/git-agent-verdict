// Concern: what a gate tells its reviewer — standing instructions, and the line opening one round | Non-concern: what the tool tells its author | IO: (declaration, intent) -> system, prompt

use crate::declarations::Declaration;
use crate::runner::MARKER;
use crate::trailer::{ADVISORY_SHAPE, COUNTS_SHAPE};

const TEMPLATE: &str = include_str!("prompt.md");

// Shipped in the binary rather than fetched: a rubric that arrives over the network can change between two runs of the same commit, and then a trailer attests a measure nobody can reconstruct. Carried here, they are pinned by whatever pins the tool — the hook's own --require-version line — so they move when a maintainer moves them and never on their own. The list itself is generated at build time from standards/*.md, so the folder is the only place a standard is declared; see build.rs.
include!(concat!(env!("OUT_DIR"), "/standards.rs"));

pub fn shipped(name: &str) -> Option<&'static str> {
    SHIPPED
        .iter()
        .find(|(known, _)| *known == name)
        .map(|(_, text)| *text)
}

// What each one is for, taken from its own first line rather than restated here: a second description is one that goes stale, and the annotation is already the file's statement of its concern.
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

// What a round is asked to look at. A commit's diff is the whole of normal development; the tree as it stands is what a rubric that just changed has never been read against, and no diff will ever show it.
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

    // What the reviewer is judging, named where the task step points at it.
    fn subject(self) -> &'static str {
        match self {
            Reach::Diff => "that diff",
            Reach::Whole => "every file it lists",
        }
    }

    // How far past what it was handed the reviewer must look. A tree has no edited lines to stop at, and a reviewer told to look past them anyway goes hunting for a change nobody made.
    fn rule(self) -> &'static str {
        match self {
            Reach::Diff => "Judge the diff and what it affects, not only the edited lines.",
            Reach::Whole => "Judge each file as it stands. Nothing here is a change, and there is no diff to read.",
        }
    }
}

// A reviewer that may write is told where it may write; one that may not is told so plainly, because the harness will refuse the call and a reviewer that does not know why spends its round arguing with the refusal.
const SANDBOX: &str = "Do not change the working tree. To test something, copy the repo to a temp directory and change it there. Confirm with `git diff --stat` before you answer.";
const NO_SANDBOX: &str = "This session cannot write anywhere, and every attempt will be refused. Confirm a suspicion by reading. One you cannot confirm that way is a guess: leave it out.";

// Quoted for the shell the reviewer types it into: a pathspec is written to be globbed by git, not by that shell.
fn quoted(paths: &[String]) -> String {
    let quoted: Vec<String> = paths
        .iter()
        .map(|p| format!("'{}'", p.replace('\'', r"'\''")))
        .collect();
    quoted.join(" ")
}

// Named here as well as at the flag: a hook is read back through the listing, so a name this build does not carry can reach a brief without ever passing the parser.
pub fn unknown_standard(name: &str) -> String {
    format!("--standard {name}: this build ships {}", shipped_names())
}

// Read in, not pointed at: a path is something a reviewer may skim or skip, and re-reads every round. Content sits apart from process so neither reads as a footnote to the other.
fn criteria(declaration: &Declaration) -> Result<String, String> {
    let mut out = String::new();
    // The general measure before the repo's own: a standard shipped here is what every repo using this tool is judged by, and the repo's documents narrow it.
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

// Everything that does not change between rounds, or between commits: a runner that hands this to a system prompt pays for it once and reads it from cache after.
pub fn system(declaration: &Declaration, reach: Reach) -> Result<String, String> {
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

// No aim, because there is no change to state one for: a tree is reviewed against the rubric as it now reads, and an intent here would be a sentence about a commit nobody is writing.
pub fn sweeping() -> String {
    "A rubric this gate judges by has changed. Review the repository as it now stands against it.\n\
     There is no commit and no diff. Report what the rubric names, and close with your verdict line.\n"
        .to_string()
}

pub fn continuing() -> String {
    "Fixes incorporated, re-review requested.\n".to_string()
}

// Not a re-review: nothing was fixed and nothing was asked again. The round this reviewer was in the middle of was cut short — killed, timed out, or crashed — and it is being taken up where it stopped. Told it was a re-review instead, it would report on changes nobody made.
pub fn resuming() -> String {
    "Your review was interrupted before you reported it. Nothing has changed since.\n\
     Continue from where you stopped, redoing only what you had not finished, and close with your verdict line.\n"
        .to_string()
}
