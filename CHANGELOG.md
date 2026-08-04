<!-- Concern: version history and notable changes | Non-concern: usage or roadmap | IO: none -->
# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres
to [Semantic Versioning](https://semver.org/).

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

[0.1.0]: https://github.com/fredrikolis/git-agent-verdict/releases/tag/v0.1.0
