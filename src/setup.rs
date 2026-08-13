// Concern: how a repo is wired to this tool — the hook it declares its gates in, and the two configs it needs | Non-concern: what any gate then decides | IO: () -> guide

// The pin a hook should declare: the leading run through the first non-zero field, which is the compatibility line this binary answers on.
fn line() -> String {
    let version = env!("CARGO_PKG_VERSION");
    let mut fields = version.split('.');
    match (fields.next(), fields.next()) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        _ => version.to_string(),
    }
}

// The sample carries what it can: gate names that say what they gate, one rubric per line, and a comment where naming cannot reach.
const GUIDE: &str = r#"WIRING A REPO — git-agent-verdict

1. Declare the gates in .githooks/commit-msg, tracked so they travel with the repo:

     #!/usr/bin/env bash
     set -euo pipefail

     # The compatibility line these flags are written against.
     git agent-verdict --require-version {{line}}

     # Refuses a commit that stages a rubric: judging a change to the measure against that same
     # measure is circular. IN-REPO docs only — one outside the worktree can never be staged.
     git agent-verdict --rubric-guard --doc docs/standards.md --doc docs/annotations.md

     # Graded, and blocks on major=. One gate may be judged against several rubrics.
     git agent-verdict "$1" my-standards-gate \
       --doc docs/standards.md --doc docs/annotations.md --path .

     # Advisory: counts findings, never blocks. The shell expands $KB, so a rubric may live
     # outside the repo — nothing there is ever staged, so the guard above does not name it.
     git agent-verdict "$1" readme-prose-gate --simple \
       --doc "$KB/writing-style.md" --path README.md

     # Line order is review order: a later gate is never judged against what an earlier one is
     # still changing. Gate names are repo-chosen labels, and reach the trailer as Reviewed-<name>.

   chmod +x .githooks/commit-msg

   The rubrics are the repo's own, and these paths are placeholders: a --doc that does not
   resolve is refused, so a hook copied verbatim says so at the first commit.

2. Point git at that directory, per clone and by hand — git refuses to let a repo do it for you:

     git config core.hooksPath .githooks

3. Name the reviewer. Host configuration, not the repo's: maintainers do not share a machine, a
   budget or a preferred agent. It reads a brief on stdin and closes with one VERDICT: line.

     git config --global agent-verdict.runner "<command reading a brief on stdin>"

4. Commit through the tool, not through git:

     git agent-verdict attest --intent "<the aim, one flat line>"

   One gate per run, in declaration order. It records what the reviewer reported and commits once
   every gate is attested. The message is composed from --intent; nothing is handed back to paste."#;

pub fn guide() -> String {
    GUIDE.replace("{{line}}", &line())
}
