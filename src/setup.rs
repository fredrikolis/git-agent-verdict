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

1. .githooks/commit-msg, tracked:

     #!/usr/bin/env bash
     set -euo pipefail
     git agent-verdict --require-version {{line}}

     git agent-verdict "$1" core --model opus \
       --standard programming --standard testing --path .

     git agent-verdict "$1" repo --model opus \
       --doc docs/standards.md \
       --rule "every public item carries a one-line comment" \
       --path .

     git agent-verdict "$1" docs --simple --standard minimal-docs --path "*.md"

     chmod +x .githooks/commit-msg

   - Line order is review order. Gate names are yours, and reach the trailer as Reviewed-<name>.
   - A gate needs one --standard, --doc or --rule, and takes any mix.
   - --simple drops the MAJOR rung: reports, never blocks.
   - --model passes through unchecked. Omitted, the agent picks.
   - --path is a git pathspec, matching at any depth.
   - --doc paths are placeholders here; one that does not resolve is refused.
   - $KB and friends expand, so a rubric may live outside the repo where nothing can stage it.
   - Staging a rubric is refused. It lands on its own with --no-verify.
   - --standards lists what this build ships; --standards <name> prints one.
   - A rule over 128 KiB fails the exec, and one carrying a newline splits in two. Redirect it to
     a file and --doc that:

     rubric="$(git rev-parse --git-path agent-verdict-generated.md)"
     generate-rubric > "$rubric"

2. Per clone, by hand:

     git config core.hooksPath .githooks

3. Per host. No default: unset, attest refuses rather than spend on an agent nobody chose.

     git config --global agent-verdict.runner claude

4. Commit through the tool:

     git agent-verdict attest --repo /abs/path/to/this/repo \
       --intent "<the aim, one flat line>"

   - One gate per run, in declaration order. The last run commits.
   - The message is composed from --intent. Nothing is handed back to paste.
   - --repo is absolute; the shell's directory is never consulted.
   - Fixing what a review named re-opens its gate. Run it again after each fix.
   - --intent is needed on the first run only.
   - No wait loop. A second attest refuses at once, naming the pid holding the repo."#;

pub fn guide() -> String {
    GUIDE.replace("{{line}}", &line())
}
