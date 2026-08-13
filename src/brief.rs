// Concern: the brief a gate hands its reviewer — its template, and the verdict line it demands back | Non-concern: what the tool tells its author | IO: (declaration, intent, files) -> brief

use crate::declarations::Declaration;
use crate::runner::{MARKER, REFUSED};
use crate::trailer::counts_shape;

const TEMPLATE: &str = include_str!("prompt.md");
const TEMPLATE_SIMPLE: &str = include_str!("prompt-simple.md");
const PLACEHOLDER: &str = "<the aim of the change, stated flatly, as a spec would state it>";

// Only a built-in template carries an annotation line; an override is a repo's own file, and eating its first line would be a silent edit.
fn built_in(text: &str) -> String {
    text.lines().skip(1).collect::<Vec<_>>().join("\n")
}

// Every field the runner must report, in the line it must report them on: what is not stated here cannot be demanded of it.
fn asked_of(simple: bool) -> String {
    format!(
        "reviewer=<who reviewed> session=<this review's id> {}",
        counts_shape(simple)
    )
}

// The machine-read line is written here and nowhere else, so what the reviewer is asked for is the shape the runner parses.
fn verdict_spec(declaration: &Declaration, files: &[String]) -> String {
    let shape = asked_of(declaration.brief.simple);
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
        None if declaration.brief.simple => built_in(TEMPLATE_SIMPLE),
        None => built_in(TEMPLATE),
    };
    let docs = declaration
        .docs
        .iter()
        .map(|d| format!("  {d}"))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(template
        .replace("{{gate}}", &declaration.gate)
        .replace("{{docs}}", &docs)
        .replace("{{intent}}", intent.unwrap_or(PLACEHOLDER))
        .replace("{{verdict}}", &verdict_spec(declaration, files)))
}
