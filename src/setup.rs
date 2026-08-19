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
   - --read-only refuses the reviewer every write, at the harness. For a tree someone else is
     working in, or a reviewer that has no business touching one.
   - --model passes through unchecked. Omitted, the agent picks.
   - --override-prompt <path> replaces the reviewer's standing instructions for that gate.
   - A reviewer runs headless and cannot answer a prompt, so it is given no chance to ask: anything
     your agent settings would have prompted for is refused instead. Pre-approve what a review
     needs there; this tool never widens it.
   - --path is a git pathspec, matching at any depth.
   - --doc paths are placeholders here; one that does not resolve is refused.
   - $KB and friends expand, so a rubric may live outside the repo where nothing can stage it.
   - Staging a rubric is refused. It lands on its own with --no-verify.
   - --standards lists what this build ships; --standards <name> prints one.
   - --rule takes multi-line text, so `--rule "$(generate-rubric)"` works. Past the 128 KiB argv
     cap, pipe it: `generate-rubric | git agent-verdict "$1" gen --rule - --path .`

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
   - No wait loop. A second attest refuses at once, saying how long the repo has been held.
   - The reviewer inherits the claim, so a review that outlives the run which started it keeps the
     repo held. The refusal names what holds it, where the system will say."#;

pub fn guide() -> String {
    GUIDE.replace("{{line}}", &line())
}
