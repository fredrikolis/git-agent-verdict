// Concern: the brief a gate hands its reviewer — its template, and the verdict line it demands back | Non-concern: what the tool tells its author | IO: (declaration, intent, files) -> brief

use crate::declarations::Declaration;
use crate::runner::{MARKER, REFUSED};
use crate::trailer::{ADVISORY_SHAPE, COUNTS_SHAPE};

const TEMPLATE: &str = include_str!("prompt.md");

// The whole of what --simple changes: an advisory gate has no MAJOR rung, so the rung is absent rather than shown and annotated away. The tool reports its zero, and the trailer keeps one shape everywhere.
const LADDER: &str = "  MAJOR (blocks the commit, and is reviewed again)
    The work is wrong, or has a severe flaw. An incremental fix will not reach the right answer.

  MODERATE (must fix, no re-review)
    The outcome is right, the execution is not. The author fixes it. Nobody checks the fix.

  MINOR (optional, fix or leave)
    The author's choice. Recorded either way.

Grade by WHAT IS WRONG, not by what the fix costs.
A MODERATE rounded up to MAJOR sends back work that is already right.
A MAJOR rounded down to MODERATE leaves a defect nobody has to fix.";

const LADDER_ADVISORY: &str = "  MODERATE (must fix, no re-review)
    The outcome is right, the execution is not. The author fixes it. Nobody checks the fix.

  MINOR (optional, fix or leave)
    The author's choice. Recorded either way.

Nothing you report blocks this commit, and there is no re-review.
Grade by WHAT IS WRONG, not by what the fix costs.
A MINOR rounded up to MODERATE makes work for the author that nobody asked for.";

const PLACEHOLDER: &str = "<the aim of the change, stated flatly, as a spec would state it>";

// Only a built-in template carries an annotation line; an override is a repo's own file, and eating its first line would be a silent edit.
fn built_in(text: &str) -> String {
    text.lines().skip(1).collect::<Vec<_>>().join("\n")
}

// Every field the runner must report, in the line it must report them on: what is not stated here cannot be demanded of it. An advisory gate is not asked for a rung it does not have.
fn asked_of(simple: bool) -> String {
    let counts = if simple { ADVISORY_SHAPE } else { COUNTS_SHAPE };
    format!("reviewer=<who reviewed> session=<this review's id> {counts}")
}

// The machine-read line is written here and nowhere else, so what the reviewer is asked for is the shape the runner parses.
fn verdict_spec(simple: bool, files: &[String]) -> String {
    let shape = asked_of(simple);
    let refusal = format!(
        "\n\nIf you are refusing the brief, close with this instead, and review nothing:\n\n  {MARKER} {REFUSED}"
    );
    let listed = files
        .iter()
        .map(|f| format!("  {f}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "The files under review, and no others:\n{listed}\n\nClose with exactly this line, and nothing after it:\n\n  {MARKER} {shape}{refusal}"
    )
}

pub fn compose(
    declaration: &Declaration,
    intent: Option<&str>,
    files: &[String],
) -> Result<String, String> {
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
    let docs = declaration
        .docs
        .iter()
        .map(|d| format!("  {d}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(template
        .replace("{{gate}}", &declaration.gate)
        .replace("{{ladder}}", ladder)
        .replace("{{docs}}", &docs)
        .replace("{{intent}}", intent.unwrap_or(PLACEHOLDER))
        .replace(
            "{{verdict}}",
            &verdict_spec(declaration.brief.simple, files),
        ))
}
