<!-- Concern: version history — one terse line per change | Non-concern: usage, rationale or roadmap | IO: none -->
# Changelog

[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
[Semantic Versioning](https://semver.org/). One line per change; the README carries the reasoning.

## [0.4.0] - 2026-08-12

### Added
- `attest --intent <line>`: the tool runs the review itself. One gate per run, in declaration order;
  it records what the reviewer reported and the last run commits. Nothing is handed out to forward.
- `agent-verdict.runner` in git config: the command that reviews. `--global` is the host default,
  `--local` overrides per clone, and neither is committed — a repo cannot pick an agent for its
  maintainers.
- `reset <reason>`: clears this commit's recorded reviews. The count and reasons reach the message.
- `token=` on every trailer: the gate resolves it and rejects counts that contradict the review.
- The reviewer's brief closes with a `VERDICT:` line, which is where the counts are read from.
- Everything else the reviewer said is written to `~/.agent-verdicts/<repo>/<head>-N-<gate>.log`
  and the path is printed. The counts say how much was found; only the report says what, and a
  full review is longer than the tail of a stream anyone reads. The log outlives the diary, which
  is dropped the moment HEAD moves — which is when an author wants to re-read it.
- The verdict goes to stdout, everything else to stderr.
- `VERDICT: refused` — the reviewer's answer to a brief that argues. It blocks on an advisory gate
  too, where the old guard's `major=1` could not.

### Changed
- `--intent` is one line of at most 300 characters, and becomes the commit's subject. Over the limit
  names the remedy: an aim that will not fit is more than one change, so commit them separately.
- `--simple` gates report `findings=<n>` and are briefed by their own template: an advisory review
  no longer grades. Previously the ladder swapped but the grading burden did not.
- MAJOR states what is wrong and blocks; the remedy is the author's, not the reviewer's to prescribe.
- Review state lives in `.git/agent-verdict/<head>/`, dropped when HEAD moves.
- `--reviewer-prompt <gate>` is for reading a brief, not for feeding one to a reviewer.
- Every brief now names the staged files in scope, and says there are no others. Scoping was never
  something the reviewer was told before — `--path` bounded the gate, and the brief said nothing.
- The reviewer MUST report `reviewer=` and `session=` on the VERDICT line — the brief states both,
  so an omission is a broken contract and exits 2 rather than being defaulted to a guess.
  `reviewer=` reaches the trailer; `session=` stays in the diary, because a transcript id is local
  evidence, not a public claim, and a pushed commit cannot be unpublished.

### Removed
- `--per-file`, and `file=` from the trailer grammar. One verdict per gate. It demanded one trailer
  per staged file so none could be silently skipped; what it cost was a trailer per file on every
  message, and what replaces it is the brief naming the files in scope.
- The `── FORWARD BELOW THIS LINE ──` block and the ladder files. The reviewer is briefed by the
  tool, so there is nothing for an author to forward and no dispatcher preamble to forward it with.

## [0.3.0] - 2026-08-05

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

[0.4.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.4.0
[0.3.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.3.0
[0.2.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.2.0
[0.1.6]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.6
[0.1.4]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.4
[0.1.3]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.3
[0.1.2]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.2
[0.1.1]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.1
[0.1.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.0
