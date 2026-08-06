<!-- Concern: what git-agent-verdict is, the trailer it verifies, and how to wire it into a repo | Non-concern: the reviews it attests (src/prompt.md and the ladders own the reviewer's brief) | IO: none -->
# git-agent-verdict

Verifies that a commit message carries an attested review verdict, and blocks on any declared
blocker. It is blind to what the standards say: the reviewer reads the standard, the gate reads a
verdict.

Built for repos where commits are written by agents. No human is expected to write a trailer.

**Why it exists:** so a repo can stay about its own code. Every repo that wants agent-written commits
reviewed ends up growing the same few hundred lines of bash to demand and check an attestation, and
every copy drifts from the others. That machinery is not what any of those repos are for. This is
that machinery, once, outside them.

## How it goes

The agent is never briefed on the review process. It is told to do the work, it does it, and it
tries to commit. The commit fails by design, and the gate prints what it wants.

Gate names below (`my-code-review`) are repo-chosen labels, not keywords. A repo declares whatever
gates it wants in its hook, one line each.

**1. The agent finishes the work and commits.**

```console
$ git commit
git-agent-verdict: my-code-review: REVIEW GATE FAILED

MISSING — the message needs this trailer and has none

  Reviewed-my-code-review: reviewer=<id> major=0 moderate=<n> minor=<n>

Earned by a review you run yourself: spawn a reviewer in a fresh context, hand it the
block below, fix every MODERATE it names, then write the counts it REPORTED into the
trailer. Only major=0 passes; there is no re-review. Trailers must be the LAST paragraph.

── FORWARD BELOW THIS LINE ──
NEUTRAL REVIEW — gate: my-code-review

Hand a reviewer in a FRESH context exactly this block with the INTENT filled in, plus where the
repo is, and nothing else. Past that the diff is the only signal it gets about where to look
...
```

The missing trailer comes first, then the prompt that earns it. Target and remedy in one turn.

**2. The agent spawns a reviewer** in a fresh context and hands it that block with the `INTENT`
filled in.

```console
$ git agent-verdict --reviewer-prompt my-code-review | claude -p \
    "INTENT: the commit-msg hook delegates verdict verification to an external CLI
             and keeps only the gate declarations."
- KISS: none
- SoC: MODERATE — install.sh declares the roster twice and the gate holds neither
...
major=0 moderate=1 minor=3
```

`--reviewer-prompt` takes the gate name and nothing else: the docs come from the `commit-msg` hook,
which already declares them, by re-running it in a mode where each gate prints its declaration
instead of validating. The shell expands whatever the hook wrote, so a `--doc "$KB/standards.md"`
resolves the same way it does at commit time.

The `INTENT` is flat, and it is the only thing added. Naming what changed, what was fixed, or what
the author suspects tells the reviewer what counts, and it will find that and stop looking.

**3. The agent fixes the MODERATE and writes down what the reviewer reported.** Not what is left
after the fix: the trailer is a record of the review, and `moderate=1` is what this review found.
The reviewer is not asked again — one look is all a gate buys, and the counts stand as it left them.

**4. Repeat per gate, in the hook's order, never in parallel.** A gate judging annotations must not
run while the gate judging code is still changing them.

**5. Commit again.**

```console
$ git commit
git-agent-verdict: my-code-review: attested (1 verdict(s), major=0 moderate=1 minor=3)
git-agent-verdict: annotations: attested (5 verdict(s), major=0 moderate=0 minor=2)
git-agent-verdict: prose: skipped (no staged file matches README.md, CONTRIBUTING.md)
[main 4f2a1c8] Delegate the commit-msg review gate to git-agent-verdict
```

Nothing in that pass was configured. `claude -p` is one runtime; the gate never sees the reviewer,
only the trailer it produces. The requirement was printed at the moment it was wanted, which is what
keeps the process out of the agent's briefing and out of the repo's docs.

## The commit message it produces

```
Delegate the commit-msg review gate to git-agent-verdict

The hook implemented verdict-checking itself in 267 lines of bash, duplicated in
two sibling repos with three distinct md5s between them. That mechanism is now an
external CLI, and the hook keeps only the gate declarations.

Reviewed-my-code-review: reviewer=claude-opus-5 major=0 moderate=1 minor=3
Reviewed-annotations: reviewer=claude-opus-5 major=0 moderate=0 minor=1 file=src/gate.rs
Reviewed-annotations: reviewer=claude-opus-5 major=0 moderate=0 minor=0 file=src/trailer.rs
Reviewed-prose: reviewer=claude-opus-5 major=0 moderate=0 minor=0
```

Subject, body, then the trailers as the last paragraph. One per gate, plus one per file where the
gate is `--per-file`. The counts are the verdict, and they are on the record with a name against
them. `moderate=1` is not an outstanding defect: it was fixed, and the count records that the
review found it.

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

## Usage

```
git-agent-verdict <msg-file> <gate> [--per-file] [--simple] [--override-prompt <path>]
                  --doc <path>... --path <pathspec>...
git-agent-verdict --rubric-guard --doc <path>...
git-agent-verdict --reviewer-prompt <gate>
```

Installs as `git agent-verdict`. Every list is a repeated singular flag. No variadic can absorb the
token meant for its neighbour, which would otherwise leave a gate silently skipped.

The second form is the rubric preflight, described below. It reads the index alone, so it takes
neither a message file nor a gate. The third prints a gate's reviewer block on stdout without
failing anything first, and takes the gate name only: everything about how that gate is briefed —
its docs, its ladder, its template — is read back out of the hook that declares it.

`--path` goes straight to `git diff --cached`, so git's pathspec syntax comes free. No staged file
matching it means the gate is skipped, and the tool says so: a mis-scoped pathspec otherwise looks
identical to a passing commit. A literal `--path` naming nothing git tracks is a typo, and fails.

`--per-file` demands one trailer per staged file, deletions excluded, with the list taken from git
rather than from the author. An author-supplied list decides what gets looked at, and that is where a missed file hides.
It is the one check the author cannot fake by scoping.

`--simple` makes the gate advisory. It still demands the trailer, so the review still has to happen
and its findings still land on the record — but no count blocks, and the reviewer is briefed to say
so. For a dimension worth a look and not worth a veto, which is otherwise the case a repo answers
by not gating it at all.

`--override-prompt <path>` replaces the built-in reviewer block with a file of the repo's own,
rendered verbatim apart from `{{gate}}`, `{{docs}}` and `{{ladder}}`. The default is what stops the
brief becoming a per-repo file that drifts, so reach for this when a repo's review genuinely is not
the default one — not to reword it.

Auto-generated messages (`Merge`, `Revert`, `fixup!`, `squash!`) carry no review and pass. `Merge`
and `Revert` are trusted only when git's own in-progress state confirms them.

Exit status: 0 pass or skip, 1 gate failed, 2 usage or git error.

## The ladder

Three rungs, graded by what is wrong, not by what the fix costs:

- **MAJOR**: the work is wrong, or carries a severe flaw. Not the author's to patch: the fix is
  re-planned by an agent that did not write it, then implemented and reviewed afresh. The only rung
  that blocks.
- **MODERATE**: the outcome is right, the execution is not. The author fixes it and does not go back
  for a second opinion on the fix.
- **MINOR**: the author's discretion — fixed, or consciously left. Recorded either way.

**The review runs once.** Only `major=0` is demanded, so the counts on a passing commit are what the
reviewer found, not what survived it. `moderate=2` is a commit whose two MODERATEs were fixed, and
the trailer says a review happened and what it saw. That is what terminates: there is no round two
to converge in.

A review-and-fix loop fails in two directions, and the band between them is narrow. Too tight and
nothing ever settles: every nit blocks, so no round comes back clean. Too loose and nothing is
caught: the reviewer rubber-stamps. The loop itself was most of the cost — a full second review to
confirm fixes the same reviewer had already specified — and MODERATE is what buys it away: real
enough to have to fix, small enough that the fix does not need looking at. MAJOR is the one case
where another review is genuinely cheaper than the alternative, and even it re-reviews nothing: the
work is re-planned by an agent that did not write it, and comes back new.

**MINOR is what keeps the count honest.** A finding can be recorded without obliging anyone to act
on it, so nothing is suppressed and no reviewer is asked to pretend the code is clean. Findings are
declassified, never hidden, which keeps the pressure on the code rather than on the report.

**`--simple` is the rung below the ladder.** A gate whose findings are worth reading and not worth
blocking on: the review is still demanded and still recorded, and the reviewer is told plainly that
nothing it reports blocks, so it grades rather than negotiates.

**The brief carries one thing: intent.** What the change set out to do, stated flatly, as a spec
would state it: no reason it is worth doing, no defence of the approach, no account of what it
replaces. A reviewer handed a case for the change grades the case, and that is most of what stands
between a review and a rubber stamp.

**Scope is not a reviewer's question.** It was settled before a plan existed, and the reviewer sees
neither the issue nor the case for the change. A scope observation comes back as one MINOR line,
never as grounds to re-plan.

A severity, not a score. It says what is wrong and what that costs, where a number lets a weak
dimension hide behind strong ones and invites argument that cannot change the outcome. Two shapes
tried before this one, both of which never settled:

| What we asked for | What we got |
|---|---|
| "Report any issues you can find." | It always found one, however minuscule. No round came back empty. |
| "Stop when every criterion scores above 9/10." | Driving DbC past 9 pushed KISS under it, and the next round traded back. |

## Wiring it into a repo

```bash
#!/usr/bin/env bash
set -euo pipefail
git agent-verdict --rubric-guard \
  --doc docs/repo-standards.md --doc docs/annotation-guide.md --doc docs/communication-style.md
git agent-verdict "$1" standards   --doc docs/repo-standards.md --path .
git agent-verdict "$1" annotations --per-file --simple --doc docs/annotation-guide.md --path .
git agent-verdict "$1" prose       --simple --doc docs/communication-style.md --path README.md
```

Line order is review order. `set -e` stops at the first unattested gate on purpose: a later gate
must never be judged against content an earlier one is still changing.

Each line is the whole declaration of its gate. `--simple` and `--override-prompt` live here beside
the docs rather than in a config file, so `--reviewer-prompt <gate>` can hand back exactly the brief
that gate will judge by, read from the one place that states it.

Annotations and prose are `--simple` above, and that is the usual shape: one gate holds the bar the
work has to clear, and the rest report. A repo that blocks on every dimension it cares about spends
its review budget on the ones that were never going to sink the change.

When a trailer is missing the tool prints it first, then the reviewer prompt that earns it, so an
agent has the target and the remedy in one turn. The other failures name what is wrong and stop. The
prompt is embedded in the binary, which is what stops it becoming a per-repo file that drifts —
`--override-prompt` is the deliberate exception, and costs a repo that file.

## The one edit it makes

A `Co-authored-by:` trailer whose address is `@anthropic.com` is removed from the message. Every
commit in a repo gated this way is agent-written, so a fixed attribution line is constant and
carries nothing, while `reviewer=` in each verdict already records who did the work.

The match is on the address, not the name, so a human co-author called Claude keeps their credit,
and no other trailer is touched. `Signed-off-by`, DCO trailers and anyone else's `Co-authored-by`
survive untouched.

This is the only case where the tool writes to the message rather than reading it.

## Circular-rubric guard

A commit that stages one of its own `--doc` files is refused, and told to land alone via
`--no-verify`. Judging a change to the measure against that same measure is circular.

This is the one place the tool refuses rather than verifies. It lives here because the list of
rubrics IS the list of `--doc` paths, and a copy in bash would be free to drift from it. Docs
outside the worktree can never be staged, so the check is a no-op for them.

## The rubric preflight

A gate sees only its own `--doc` paths, so on its own it cannot refuse on behalf of a gate further
down the hook. Staging a later gate's rubric would cost one full review of an earlier gate before
the refusal arrives, and that wasted review is what `--rubric-guard` removes:

```
git agent-verdict --rubric-guard --doc <path>...
```

It holds every gate's rubrics at once and runs first in the hook, so the refusal arrives before any
review is asked for. It reads the index and nothing else: no message file, no gate, no `--path`.
Staged rubrics are named and the commit is refused with exit 1; otherwise it exits 0 in silence.

**It is an optimisation, not the check.** The per-gate guard stays, and stays the correctness
backstop, so a rubric this list has drifted away from costs an early exit and never a missed rubric.
That is what makes stating the paths a second time safe here, where sharing a list between gates
would not be.

`--rubric-guard` with no `--doc` is a usage error (exit 2), as it is for a gate: a preflight
guarding nothing is a hook that has silently stopped guarding. So is a `<msg-file>`, `<gate>`,
`--path` or `--per-file` given alongside it, since the mode has no use for any of them. A `--doc`
that does not resolve on disk is a usage error in both modes, because a rubric that exempted itself
by being mistyped is the drift the guard exists to prevent.

## Developing this repo

The two linters the pre-commit hook calls are pinned at install time, so the hook stays a flat list
of commands with no wrapper script holding a version:

```bash
npm i -g annotated-tree@0.6.0
cargo install --git https://github.com/fredrikolis/cargo-lint-extra \
  --rev 7a232179e45414108d28047acd6315d9a2c4946b --locked cargo-lint-extra
git config core.hooksPath .githooks
```
