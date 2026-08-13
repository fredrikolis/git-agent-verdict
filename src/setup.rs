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

     # Graded, and blocks on major=. One gate may be judged against several rubrics.
     git agent-verdict "$1" my-standards-gate \
       --doc docs/standards.md --doc docs/annotations.md --path .

     # Advisory: same ladder, no MAJOR rung, never blocks. The shell expands $KB, so a rubric may live
     # outside the repo, where nothing can stage it.
     # `*.md` is a git pathspec, so it matches at any depth: this gate reads docs/standards.md
     # too, which is what leaves the standards rubric reviewed by someone when it changes alone.
     git agent-verdict "$1" prose-gate --simple \
       --doc "$KB/writing-style.md" --path "*.md"

     # Line order is review order: a later gate is never judged against what an earlier one is
     # still changing. Gate names are repo-chosen labels, and reach the trailer as Reviewed-<name>.
     # A gate stands aside when its own rubric is the whole of what is staged, so let some other
     # gate's --path cover your rubrics — attest names every staged file no gate read.

   chmod +x .githooks/commit-msg

   The rubrics are the repo's own, and these paths are placeholders: a --doc that does not
   resolve is refused, so a hook copied verbatim says so at the first commit.

2. Point git at that directory, per clone and by hand — git refuses to let a repo do it for you:

     git config core.hooksPath .githooks

3. Name the reviewer. Host configuration, not the repo's: maintainers do not share a machine, a
   budget or a preferred agent. There is no default — unset, attest refuses rather than spending
   on an agent nobody chose.

     git config --global agent-verdict.runner "claude -p"

   Any command that reads a brief on stdin and closes with one VERDICT: line will do; that one is
   an example, not a dependency. It must report reviewer= and session= on the line, as the brief
   asks. Wrapping it, pass the reviewer's own output through: the counts say how much was found,
   and only that output says what.

   Fixing what a review names re-opens its gate, so the next attest reviews the same gate again.
   $AGENT_VERDICT_PRIOR_SESSION holds the session the last reviewer reported. A runner that can
   resume it reads what changed instead of sampling the whole rubric afresh:

     git config --global agent-verdict.runner \
       'if [ -n "$AGENT_VERDICT_PRIOR_SESSION" ]; then claude -p --resume "$AGENT_VERDICT_PRIOR_SESSION"; else claude -p; fi'

4. Commit through the tool, not through git:

     git agent-verdict attest --intent "<the aim, one flat line>"

   One gate per run, in declaration order. It records what the reviewer reported and commits once
   every gate is attested. The message is composed from --intent; nothing is handed back to paste."#;

pub fn guide() -> String {
    GUIDE.replace("{{line}}", &line())
}
