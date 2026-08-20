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
const GUIDE: &str = r#"CONFIGURING A REPOSITORY — git-agent-verdict

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

   - Line order is review order. Gate names are chosen by you and become the trailer key Reviewed-<name>.
   - A gate needs one --standard, --doc or --rule, and takes any mix.
   - --simple removes the MAJOR severity: reports, never blocks.
   - --read-only refuses the reviewer every write, at the harness. For a tree someone else is
     working in, or or a reviewer that must not write.
   - --model is passed through unchecked. If omitted, the agent selects one.
   - --override-prompt <path> replaces the reviewer's standing instructions for that gate.
   - A reviewer runs headless and cannot answer a prompt, so it is given no chance to ask: anything
     your agent settings would have prompted for is refused instead. Pre-approve what a review
     needs there; this tool never widens it.
   - --path is a git pathspec, matching at any depth.
   - --doc paths are placeholders here; one that does not resolve is refused.
   - Environment variables expand, so a criteria file may live outside the repository, where nothing can stage it.
   - Staging a criteria file is refused. Commit it separately with --no-verify.
   - --standards lists what this build ships; --standards <name> prints one.
   - --rule takes multi-line text, so `--rule "$(generate-rubric)"` works. Past the 128 KiB argv
     cap, pipe it: `generate-rubric | git agent-verdict "$1" gen --rule - --path .`

2. Per clone, by hand:

     git config core.hooksPath .githooks

3. Per host. No default: unset, attest refuses.

     git config --global agent-verdict.runner claude

4. Commit through the tool:

     git agent-verdict attest --repo /abs/path/to/this/repo \
       --intent "<intent: one line>"

   - One run reviews every gate in declaration order, stopping at the first MAJOR.
   - `git agent-verdict commit --repo <root>` creates the commit once every gate has passed.
   - The message is composed from --intent; nothing is emitted for the caller to paste.
   - --repo is absolute; the shell's directory is never consulted.
   - Every MAJOR and MODERATE finding is a required fix; MINOR is at your discretion. A MAJOR
     requires the re-review (a MODERATE does not).
   - --intent is needed on the first run only.
   - attest starts the review and returns. `await --repo <root>`, then `commit --repo <root>`.
   - `abort --repo <root>` if you need to stop one; verdicts already recorded are kept.
   - No wait loop. A second attest reports the review already running instead of starting another."#;

pub fn guide() -> String {
    GUIDE.replace("{{line}}", &line())
}
