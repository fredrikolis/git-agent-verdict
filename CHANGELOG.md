<!-- Concern: version history and notable changes | Non-concern: usage or roadmap | IO: none -->
# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres
to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed
- **A gate blocks on `major=` alone.** A MODERATE is now fixed without a second look, so the review
  runs once and the count that reaches the trailer is a record of what the reviewer found rather
  than a tally of what is left outstanding. The fix -> re-review loop it removes was most of what a
  gate cost: a full second review to confirm fixes the same reviewer had already specified. MAJOR
  still blocks, and still re-reviews nothing — the work is re-planned by an agent that did not write
  it and comes back new. A commit attesting `major=0 moderate=2 minor=1` passes and says so.
- MINOR is the author's discretion: fixed, or consciously left, recorded either way.
- The reviewer is told to read for defects rather than re-run the work looking for them. Building
  and probing is how a suspicion it already has gets confirmed before it is reported, not a sweep to
  turn one up, which is most of how a review turned into an afternoon.
- `attested` names all three counts, because a passing commit now carries findings.
- The prompt's paragraphs are no longer hard-wrapped: the block is piped into a reviewer, where
  fixed-column breaks are noise.

### Added
- `--simple` makes a gate advisory. The trailer is still demanded, so the review still happens and
  its findings still land on the record, but no count blocks and the reviewer is briefed against a
  ladder that says so. For a dimension worth a look and not worth a veto, which a repo otherwise
  answers by not gating it at all.
- `--override-prompt <path>` replaces the built-in reviewer block with a repo's own file, rendered
  verbatim apart from `{{gate}}`, `{{docs}}` and `{{ladder}}`. Unlike a built-in template its first
  line is content, never an annotation to strip. A path that does not resolve exits 2 rather than
  falling back to the default in silence.
- Both are declared on the gate's line in the hook, and `--reviewer-prompt <gate>` reads them back
  from there with the docs, so the block it prints is the one that gate will judge by. Rejected
  alongside `--reviewer-prompt` and `--rubric-guard`, neither of which briefs a reviewer.

## [0.1.6] - 2026-08-05

### Added
- `--reviewer-prompt <gate>` prints that gate's reviewer block on stdout and exits 0, so it can be
  piped straight into a reviewer. Before this the block could only be obtained by failing a gate, so
  wiring a repo up meant provoking a failure to read the instructions. It takes the gate name and
  nothing else: the docs come from the `commit-msg` hook, which is re-run with
  `GIT_AGENT_VERDICT_LIST` set so each gate prints its declaration instead of validating. Reading the
  hook as text would not do, because every real hook writes its docs through a shell variable. An
  unknown gate name exits 2 and lists the gates the hook does declare.

## [0.1.4] - 2026-08-05

### Added
- `git-agent-verdict --rubric-guard --doc <path>...`, a preflight that reads the index alone and
  refuses the commit when any named rubric is staged, naming which and asking for `--no-verify`.
  Run first in a hook it holds every gate's rubrics at once, so staging a later gate's rubric no
  longer costs a full review of an earlier gate before the refusal arrives. Flag-only, because the
  mode needs neither the message nor a gate: a `<msg-file>`, `<gate>`, `--path` or `--per-file`
  beside it exits 2, as does `--rubric-guard` with no `--doc` — a preflight guarding nothing is a
  hook that has silently stopped guarding.

### Documented
- The per-gate guard stays, and stays the correctness backstop. The preflight only fails faster, so
  a rubric its list has drifted away from costs an early exit and never a missed rubric. That is
  what makes stating the paths a second time safe here, where sharing a list between gates is not.

## [0.1.3] - 2026-08-05

### Added
- The agent `Co-authored-by:` trailer is dropped from the commit message. Matched on an
  `@anthropic.com` address rather than on the name, so a human co-author called Claude keeps their
  credit and no other trailer is touched. This is the only edit the tool makes to a message.

## [0.1.2] - 2026-08-05

### Added
- `--version` / `-V` and `--help` / `-h`, both info flags that print and exit 0. Without a version
  flag a consuming repo cannot pin the gate, and the trailer format plus the reviewer brief are the
  gate's contract, so an unpinned install changes what every commit must carry.

## [0.1.1] - 2026-08-04

### Changed
- The reviewer prompt now carries an `INTENT:` slot and tells the reviewer to judge it first,
  rejecting a brief that argues for the change. Before this the prompt said to hand a reviewer the
  block "and nothing else", which forbade attaching the intent the process requires, so the rule and
  the surface that delivers it disagreed at the point of use.
- The prompt states that scope is not the reviewer's question, so a scope observation comes back as
  one MINOR line rather than as grounds to re-plan.

### Documented
- The circular-rubric guard sees only its own gate's `--doc` paths, so staging a later gate's rubric
  costs one full review of an earlier gate before the refusal arrives.

## [0.1.0] - 2026-08-04

### Added
- `git-agent-verdict <msg-file> <gate> [--per-file] --doc <path>... --path <pathspec>...`, invoked
  from a `commit-msg` hook. It verifies that the message carries a `Reviewed-<gate>:` trailer naming
  a reviewer and three numeric counts, and fails when `major` or `moderate` is above zero.
- `--path`, handed verbatim to `git diff --cached`, so git's pathspec syntax scopes a gate. No
  staged file matching it skips the gate, and the tool says which pathspec matched nothing. A
  literal `--path` naming nothing git tracks is reported as a typo rather than a skip.
- `--per-file`, which demands one trailer per staged file with the list taken from git rather than
  from the author.
- The reviewer prompt, embedded in the binary and printed on any failure alongside the trailer that
  is missing, so the target and the remedy arrive together.
- A circular-rubric guard: staging one of a gate's own `--doc` files refuses the commit and asks for
  `--no-verify`, so a change to a yardstick lands on its own.
- An exemption for the subjects git writes itself. `Merge` and `Revert` are honoured only when
  `MERGE_HEAD` or `REVERT_HEAD` confirms them, so a hand-typed subject cannot forge it.

[0.1.6]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.6
[0.1.4]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.4
[0.1.3]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.3
[0.1.2]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.2
[0.1.1]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.1
[0.1.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.0
