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

     # --standard names a rubric shipped inside this binary, so a repo gates on a general measure
     # without hosting or copying one. They move only when the tool does, which the line above pins.
     # `git agent-verdict --standards` lists what this build carries and what each one judges;
     # `--standards <name>` prints one in full. They are text inside the binary, not files in the repo.
     git agent-verdict "$1" core --model opus \
       --standard programming --standard testing --path .

     # Graded, and blocks on major=. A gate may be judged against several rubrics, and --rule
     # states one inline where a whole document would be more than the check is worth.
     # --model is passed to the agent as given and never checked here: what a gate is worth
     # reviewing at is the repo's call. Omitted, the agent picks.
     git agent-verdict "$1" my-standards-gate --model opus \
       --doc docs/standards.md --doc docs/annotations.md \
       --rule "every public item carries a one-line comment" \
       --path .

     # Holds documentation to a stub, across every .md in the repo.
     git agent-verdict "$1" docs --simple --standard minimal-docs --path "*.md"

     # Advisory: same ladder, no MAJOR rung, never blocks. The shell expands $KB, so a rubric may live
     # outside the repo, where nothing can stage it. A gate needs one --standard, --doc or --rule.
     # `*.md` is a git pathspec, so it matches at any depth: this gate reads docs/ too.
     git agent-verdict "$1" prose-gate --simple --model haiku \
       --doc "$KB/writing-style.md" --path "*.md"

     # A rule longer than a line does not belong in argv. One argument is capped (128 KiB on
     # Linux), so --rule "$(generate-rubric)" fails the exec once the text grows, and a rule
     # carrying a newline splits into two in the gate listing, which is line-based.
     # Redirect the same command into a file and point --doc at it. No cap, and no quoting:
     rubric="$(git rev-parse --git-path agent-verdict-generated.md)"
     generate-rubric > "$rubric"
     git agent-verdict "$1" generated-gate --doc "$rubric" --path .

     # Line order is review order: a later gate is never judged against what an earlier one is
     # still changing. Gate names are repo-chosen labels, and reach the trailer as Reviewed-<name>.
     # Staging a rubric is refused, whichever gate declares it: what the repo gates by is
     # maintenance, and lands on its own with --no-verify.

   chmod +x .githooks/commit-msg

   The rubrics are the repo's own, and these paths are placeholders: a --doc that does not
   resolve is refused, so a hook copied verbatim says so at the first commit.

2. Point git at that directory, per clone and by hand — git refuses to let a repo do it for you:

     git config core.hooksPath .githooks

3. Name the reviewer. Host configuration, not the repo's: maintainers do not share a machine, a
   budget or a preferred agent. There is no default — unset, attest refuses rather than spending
   on an agent nobody chose.

     git config --global agent-verdict.runner claude

4. Commit through the tool, not through git:

     git agent-verdict attest --repo /abs/path/to/this/repo \
       --intent "<the aim, one flat line>"

   One gate per run, in declaration order. It records what the reviewer reported and commits once
   every gate is attested. The message is composed from --intent; nothing is handed back to paste.

   --repo is the repo root, absolute, and the shell's directory is never consulted. A shell held
   open for an hour is often not where its owner believes; naming the tree puts that assumption in
   the command line, where it can be read back.

   Fixing what a review named re-opens its gate, so run attest again after each fix. --intent is
   only needed on the first run.

   Run it directly. Do not wrap it in a wait loop: attest holds the repo for as long as it runs,
   and a second one refuses at once, naming the pid holding it and for how long. A guard built
   from pgrep matches its own wrapper's command line and waits for ever."#;

pub fn guide() -> String {
    GUIDE.replace("{{line}}", &line())
}
