<!-- Concern: version history — one terse line per change | Non-concern: usage, rationale or roadmap | IO: none -->
# Changelog

[Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
[Semantic Versioning](https://semver.org/). One line per change; the README carries the reasoning.

## [1.0.1] - 2026-08-13

### Changed
- Configuring the reviewer answers itself. The unset-runner error and `--repo-setup-guide` gave a
  placeholder where the README gave `claude -p`, and neither said whether a default existed — so
  establishing that none does meant reading the source. Both now name a concrete command and state
  that unset means refused, not defaulted.
- The README's `# the host's default` on that line meant global scope, and read as a claim that
  `claude -p` is the default runner. It says `# every repo on this machine`.

## [1.0.0] - 2026-08-12

First release with a compatibility promise. Everything below is breaking against 0.3.0, and the
version says so: 0.x promised nothing, which is exactly how a hook pinned at a floor sailed into a
release that had taken its flags away. From here a break is a major bump and `^1` passes anything
additive.

### Added
- `attest --intent <line>`: the tool runs the review itself. One gate per run, in declaration order;
  it records what the reviewer reported and the last run commits. Nothing is handed out to forward.
- `agent-verdict.runner` in git config: the command that reviews. `--global` is the host default,
  `--local` overrides per clone, and neither is committed — a repo cannot pick an agent for its
  maintainers.
- `reset <reason>`: clears this commit's recorded reviews. The count and reasons reach the message.
- `token=` on every trailer: the gate resolves it and rejects counts that contradict the review.
- The reviewer's brief closes with a `VERDICT:` line, which is where the counts are read from.
- `--repo-setup-guide`: the hook a repo declares its gates in, `core.hooksPath`, and the runner
  config, pinned to the installed version. The one mode that answers outside a repo.
- A CLI syntax error prints that guide in full. A declaration that no longer parses is the repo's
  wiring gone stale, which its maintainer fixes — `attest` and `reset` are exempt, being what a dev
  agent types, where the guide buries the line naming the fault.
- `attest` names every gate whose content moved since its verdict, before it commits. Fixing what a
  review found does not re-open its gate and nothing recounts, so a bound the reviewer enforces can
  be broken by the fix and land unchecked. It is said rather than refused: re-reviewing every fix is
  the loop this tool exists to avoid.
- Everything else the reviewer said is written to `~/.agent-verdicts/<repo>/<head>-N-<gate>.log`
  and the path is printed. The counts say how much was found; only the report says what, and a
  full review is longer than the tail of a stream anyone reads. The log outlives the diary, which
  is dropped the moment HEAD moves — which is when an author wants to re-read it.
- The verdict goes to stdout, everything else to stderr.
- `VERDICT: refused` — the reviewer's answer to a brief that argues. It blocks on an advisory gate
  too, where the old guard's `major=1` could not.

### Changed
- **`--check-min-version` is replaced by `--require-version`, and it pins a compatibility line
  rather than a floor.** `0.4` is its own line in cargo's sense: `0.4.1` satisfies a pin on `0.4`,
  `0.5.0` and `0.3.0` do not. The floor could not see a breaking release — this one removes flags
  and changes the trailer grammar, and every hook pinned below it passed, then died on an unknown
  flag. The rename is deliberate: a hook still saying `--check-min-version` fails loudly on an
  unknown flag instead of silently passing.
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

- Every mode refuses a flag it does not take, `attest` and `reset` included, rather than acting on
  half the invocation. A gate name is letters, digits, `-`, `_` or `.`: it becomes the trailer key
  `Reviewed-<gate>`, and git parses a trailer key as one word.
- Every message is dense and scannable — a labelled fact per line, remedies as bullets — for the
  agent that reads them out of a hook's stderr. The reviewer's brief is unchanged.
- `attest` says `committed <sha> — every gate attested, nothing left to run` on stdout. The landing
  was announced on stderr alone, leaving the channel an agent parses to git's own output.
- The guidance an author reads describes what `attest` does: it commits. Three messages still told
  them to paste a trailer it returns.
- A run with nothing staged says so, where it blamed the hook for declaring no gate this commit
  reaches. It is what a run right after the commit landed finds.

### Fixed
- Every mode runs from the repo root. A hook declares its docs and pathspecs against the root,
  because that is where git runs it, but `attest` runs wherever the agent stands — and `--path .`
  resolved from a subdirectory reviewed a fraction of the change, passed, and said nothing.
- `--version`, `-V`, `--help` and `-h` are answered only as the sole argument. Scanned across the
  whole line, a stray one in a gate's declaration exited 0 and the gate passed having checked
  nothing.
- Enumerating a hook no longer fires its guards. `--rubric-guard` and `--require-version` acted
  during the listing run, and under `set -e` the refusal killed the hook — every gate below it left
  the listing, and a staged rubric was reported as a hook declaring no gates.
- `attest` refuses a staged rubric before it pays for a review, not after. The hook's preflight
  caught it only at commit time, by which point every gate had been reviewed and billed.
- A reviewer closing with more than one `VERDICT:` line is refused. The extra lines were recorded
  under one token and rendered as a trailer apiece, and the gate read them as contradicting the
  review they named — the tool made a commit its own hook then refused.
- A reviewer's `VERDICT:` count that is not a number is named as such, where it was read as an
  absent field and reported as a missing one.
- The brief is written to the reviewer from its own thread. Both pipes are bounded, so a reviewer
  that talked while a long brief was still going in deadlocked against it.
- A hook that declares no gates reports what it said while failing, instead of leaving the reader
  with a hook they can see declaring gates and an error saying it declares none.

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

[1.0.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v1.0.0
[0.3.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.3.0
[0.2.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.2.0
[0.1.6]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.6
[0.1.4]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.4
[0.1.3]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.3
[0.1.2]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.2
[0.1.1]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.1
[0.1.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.0
