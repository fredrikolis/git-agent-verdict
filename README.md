<!-- Concern: what git-agent-verdict is, the trailer it verifies, and how to wire it into a repo | Non-concern: the reviews it attests (src/prompt.md owns the reviewer's brief) | IO: none -->
# git-agent-verdict

Verifies that a commit message carries an attested review verdict, and blocks on any declared
blocker. It is blind to what the standards say: the reviewer reads the standard, the gate reads a
verdict.

Built for repos where commits are written by agents. No human is expected to write a trailer.

## Trust model

The gate verifies that an attestation is PRESENT and WELL-FORMED. It cannot verify that a review
happened. What it buys is attribution, not enforcement. A false attestation passes, but it is then a
claim on the record with a name on it.

## The trailer

```
Reviewed-standards: reviewer=claude-opus-5 major=0 moderate=0 minor=3
Reviewed-annotations: reviewer=claude-opus-5 major=0 moderate=0 minor=1 file=src/trailer.rs
```

Named fields, so a per-file gate adds one instead of changing the arity. `file=` is terminal, so a
path may contain spaces. Trailers must be the message's last paragraph. That is what
`git interpret-trailers --parse` reads, and a trailer written above the body is invisible to it.
The tool detects that case by name rather than reporting the trailer as missing.

The counts are the whole verdict, and asking for three numbers rather than a verdict is deliberate.
A reviewer that reports `major=0` has committed to a number it can be held to, which matters
precisely because the writer is an LLM.

## The ladder

Three rungs, graded by what is wrong, not by what the fix costs:

- **MAJOR**: the work is wrong, or carries a severe flaw. Not the author's to patch: the fix is
  re-planned by an agent that did not write it, then implemented and reviewed afresh.
- **MODERATE**: the outcome is right, the execution is not. The author fixes it, and the review
  runs again.
- **MINOR**: fixable without another look. Never blocks; required, so a real nit has a home instead
  of being inflated into a blocker.

Iterate fix -> re-review until MAJOR and MODERATE are both zero. That terminates cleanly because
both blocking buckets mean the same thing: there is work the reviewer has not judged yet. When
there is none left, the loop is over.

A review-and-fix loop fails in two directions, and the band between them is narrow. Too tight and
nothing ever settles: every nit blocks, so no round converges. Too loose and nothing is caught: the
reviewer rubber-stamps.

**MINOR is what makes zero-zero reachable.** A finding can be recorded without restarting the review,
so nothing is suppressed and no reviewer is asked to pretend the code is clean. Findings are
declassified, never hidden, which keeps the pressure on the code rather than on the report. The
loop ends when the reviewer has seen everything that matters, not when it runs out of things to
say.

**The brief carries one thing: intent.** What the change set out to do, stated flatly, as a spec
would state it: no reason it is worth doing, no defence of the approach, no account of what it
replaces. A reviewer handed a case for the change grades the case, and that is most of what stands
between a review and a rubber stamp.

**Scope is not a reviewer's question.** It was settled before a plan existed, and the reviewer sees
neither the issue nor the case for the change. A scope observation comes back as one MINOR line,
never as grounds to re-plan.

A severity, not a score. It drives the weak dimension to zero directly, where a number lets it hide
behind strong ones and invites argument that cannot change the outcome. Two shapes tried before
this one, both of which never settled:

| What we asked for | What we got |
|---|---|
| "Report any issues you can find." | It always found one, however minuscule. No round came back empty. |
| "Stop when every criterion scores above 9/10." | Driving DbC past 9 pushed KISS under it, and the next round traded back. |

## Usage

```
git-agent-verdict <msg-file> <gate> [--per-file] --doc <path>... --path <pathspec>...
```

Installs as `git agent-verdict`. Every list is a repeated singular flag. No variadic can absorb the
token meant for its neighbour, which would otherwise leave a gate silently skipped.

`--path` goes straight to `git diff --cached`, so git's pathspec syntax comes free. No staged file
matching it means the gate is skipped, and the tool says so: a mis-scoped pathspec otherwise looks
identical to a passing commit. A literal `--path` naming nothing git tracks is a typo, and fails.

`--per-file` demands one trailer per staged file, deletions excluded, with the list taken from git
rather than from the author. An author-supplied list decides what gets looked at, and that is where a missed file hides.
It is the one check the author cannot fake by scoping.

Auto-generated messages (`Merge`, `Revert`, `fixup!`, `squash!`) carry no review and pass. `Merge`
and `Revert` are trusted only when git's own in-progress state confirms them.

Exit status: 0 pass or skip, 1 gate failed, 2 usage or git error.

## Wiring it into a repo

```bash
#!/usr/bin/env bash
set -euo pipefail
git agent-verdict "$1" standards   --doc docs/repo-standards.md --path .
git agent-verdict "$1" annotations --per-file --doc docs/annotation-guide.md --path .
git agent-verdict "$1" prose       --doc docs/communication-style.md --path README.md
```

Line order is review order. `set -e` stops at the first unattested gate on purpose: a later gate
must never be judged against content an earlier one is still changing.

When a trailer is missing the tool prints it first, then the reviewer prompt that earns it, so an
agent has the target and the remedy in one turn. The other failures name what is wrong and stop. The prompt is embedded in the binary, which is what
stops it becoming a per-repo file that drifts.

## Circular-rubric guard

A commit that stages one of its own `--doc` files is refused, and told to land alone via
`--no-verify`. Judging a change to the measure against that same measure is circular.

This is the one place the tool refuses rather than verifies. It lives here because the list of
rubrics IS the list of `--doc` paths, and a copy in bash would be free to drift from it. Docs outside the worktree can never be
staged, so the check is a no-op for them.

## Developing this repo

The two linters the pre-commit hook calls are pinned at install time, so the hook stays a flat list
of commands with no wrapper script holding a version:

```bash
npm i -g annotated-tree@0.6.0
cargo install --git https://github.com/fredrikolis/cargo-lint-extra \
  --rev 7a232179e45414108d28047acd6315d9a2c4946b --locked cargo-lint-extra
git config core.hooksPath .githooks
```
