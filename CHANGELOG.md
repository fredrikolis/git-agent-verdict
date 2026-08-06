<!-- Concern: version history — one terse line per change | Non-concern: usage, rationale or roadmap | IO: none -->
# Changelog

[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
[Semantic Versioning](https://semver.org/). One line per change; the README carries the reasoning.

## [Unreleased]

### Added
- `--check-min-version <version>`: a hook pins a floor in one line. Exit 1 names the install command.

### Changed
- The README states what the tool is for and why; `--help` states the grammar.
- The reviewer block says to spawn the review in the background — waiting on it is what blocks.

## [0.2.0] - 2026-08-05

### Added
- `--simple`: an advisory gate. The trailer is still demanded; no count blocks.
- `--override-prompt <path>`: a repo's own reviewer block, `{{gate}}`/`{{docs}}`/`{{ladder}}`
  substituted. An unresolvable path exits 2 rather than falling back in silence.

### Changed
- A gate blocks on `major=` alone. The review runs once and the counts record what it found.
- MINOR is the author's discretion, recorded either way.
- The reviewer reads for defects rather than re-running the work to look for them.
- `attested` names all three counts.
- The prompt is no longer hard-wrapped.

## [0.1.6] - 2026-08-05

### Added
- `--reviewer-prompt <gate>` prints a gate's block on stdout, so obtaining it no longer means
  provoking a failure. Docs are read back from the hook, re-run with `GIT_AGENT_VERDICT_LIST` set.

## [0.1.4] - 2026-08-05

### Added
- `--rubric-guard`: a flag-only preflight that refuses the commit when any gate's rubric is staged,
  before an earlier gate's review is paid for. No `--doc` exits 2.

## [0.1.3] - 2026-08-05

### Added
- The agent `Co-authored-by:` trailer is dropped, matched on the `@anthropic.com` address.

## [0.1.2] - 2026-08-05

### Added
- `--version` / `-V` and `--help` / `-h`, so a consuming repo can pin the gate.

## [0.1.1] - 2026-08-04

### Changed
- The prompt carries an `INTENT:` slot and rejects a brief that argues for the change.
- Scope is not the reviewer's question: it comes back as one MINOR line.

## [0.1.0] - 2026-08-04

### Added
- `git-agent-verdict <msg-file> <gate> --doc <path>... --path <pathspec>...`, run from a `commit-msg`
  hook: it demands a `Reviewed-<gate>:` trailer naming a reviewer and three counts.
- `--path` scopes a gate by git pathspec; nothing staged skips it, an untracked literal is a typo.
- `--per-file`: one trailer per staged file, listed by git rather than by the author.
- The reviewer prompt, embedded in the binary and printed with the trailer that is missing.
- A circular-rubric guard: staging a gate's own `--doc` refuses the commit.
- An exemption for `Merge`, `Revert`, `fixup!` and `squash!`, the first two confirmed by git's state.

[Unreleased]: https://github.com/fredrikolis/git-agent-verdict/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.2.0
[0.1.6]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.6
[0.1.4]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.4
[0.1.3]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.3
[0.1.2]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.2
[0.1.1]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.1
[0.1.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.0
