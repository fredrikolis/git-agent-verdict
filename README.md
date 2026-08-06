<!-- Concern: what git-agent-verdict is, why it exists, and how to install and wire it | Non-concern: the flag grammar (--help owns it) or the reviewer's brief (src/prompt.md and the ladders) | IO: none -->
# git-agent-verdict

Verifies that a commit message carries an attested review verdict, and blocks on any declared
blocker. It is blind to what the standards say: the reviewer reads the standard, the gate reads a
verdict.

Built for repos where commits are written by agents. No human is expected to write a trailer.

**Why it exists:** so a repo can stay about its own code. Every repo that wants agent-written commits
reviewed ends up growing the same few hundred lines of bash to demand and check an attestation, and
every copy drifts from the others. That machinery is not what any of those repos are for. This is
that machinery, once, outside them.

## Install

```bash
cargo install git-agent-verdict
```

Installs as `git agent-verdict`. `--help` is the flag reference; this file is not.

## Wiring it into a repo

`.githooks/commit-msg`, enabled per clone with `git config core.hooksPath .githooks`:

```bash
#!/usr/bin/env bash
set -euo pipefail
git agent-verdict --check-min-version 0.3.0
git agent-verdict --rubric-guard \
  --doc docs/repo-standards.md --doc docs/annotation-guide.md --doc docs/communication-style.md
git agent-verdict "$1" standards   --doc docs/repo-standards.md --path .
git agent-verdict "$1" annotations --per-file --simple --doc docs/annotation-guide.md --path .
git agent-verdict "$1" prose       --simple --doc docs/communication-style.md --path README.md
```

Gate names are repo-chosen labels, not keywords. Line order is review order, and `set -e` stops at
the first unattested gate on purpose: a later gate must never be judged against content an earlier
one is still changing.

Each line is the whole declaration of its gate, which is what lets `--reviewer-prompt <gate>` hand
back exactly the brief that gate will judge by — it re-runs this hook to read the declaration rather
than keeping a second copy in a config file.

A version floor, not an equality: what must not arrive silently is a different reviewer brief, and
that only happens when the floor is raised deliberately, so an additive release passes. One line in
the hook, rather than a shell version-compare beside every gate.

Most gates are `--simple` above, and that is the usual shape: one gate holds the bar the work has to
clear, and the rest report. A repo that blocks on every dimension it cares about spends its review
budget on the ones that were never going to sink the change.

## How it goes

The agent is never briefed on the review process. It is told to do the work, it does it, and it
tries to commit. The commit fails by design, and the gate prints what it wants — the missing trailer
first, then the reviewer prompt that earns it, so the target and the remedy arrive in one turn.

```console
$ git commit
git-agent-verdict: my-code-review: REVIEW GATE FAILED

MISSING — the message needs this trailer and has none

  Reviewed-my-code-review: reviewer=<id> major=0 moderate=<n> minor=<n>

── FORWARD BELOW THIS LINE ──
NEUTRAL REVIEW — gate: my-code-review
...
```

The agent spawns a reviewer in a fresh context, hands it that block with the `INTENT` filled in,
fixes what comes back, writes down what the reviewer reported, and commits again.

```console
$ git agent-verdict --reviewer-prompt my-code-review | claude -p \
    "INTENT: the commit-msg hook delegates verdict verification to an external CLI
             and keeps only the gate declarations."
```

Nothing in that pass was configured, and `claude -p` is one runtime among many: the gate never sees
the reviewer, only the trailer it produces. The requirement is printed at the moment it is wanted,
which is what keeps the process out of the agent's briefing and out of the repo's docs.

## The trailer

```
Reviewed-standards: reviewer=claude-opus-5 major=0 moderate=1 minor=3
Reviewed-annotations: reviewer=claude-opus-5 major=0 moderate=0 minor=1 file=src/trailer.rs
```

Named fields, so a per-file gate adds one instead of changing the arity. Trailers must be the
message's last paragraph — that is what `git interpret-trailers --parse` reads.

The counts are the whole verdict, and asking for three numbers rather than a verdict is deliberate:
a reviewer that reports `major=0` has committed to a number it can be held to, which matters
precisely because the writer is an LLM. `moderate=1` on a passing commit is not an outstanding
defect. It was fixed, and the count records that the review found it.

**Trust model.** The gate verifies that an attestation is PRESENT and WELL-FORMED. It cannot verify
that a review happened. What it buys is attribution, not enforcement: a false attestation passes,
but it is then a claim on the record with a name on it.

## The ladder

Three rungs, graded by what is wrong, not by what the fix costs:

- **MAJOR**: the work is wrong, or carries a severe flaw. Not the author's to patch: the fix is
  re-planned by an agent that did not write it, then implemented and reviewed afresh. The only rung
  that blocks.
- **MODERATE**: the outcome is right, the execution is not. The author fixes it and does not go back
  for a second opinion on the fix.
- **MINOR**: the author's discretion — fixed, or consciously left. Recorded either way.

**The review runs once.** A review-and-fix loop fails in two directions, and the band between them is
narrow. Too tight and nothing ever settles: every nit blocks, so no round comes back clean. Too loose
and the reviewer rubber-stamps. The loop itself was most of the cost — a full second review to
confirm fixes the same reviewer had already specified — and MODERATE is what buys it away: real
enough to have to fix, small enough that the fix does not need looking at. MAJOR is the one case
where another review is genuinely cheaper than the alternative, and even it re-reviews nothing.

**MINOR is what keeps the count honest.** A finding can be recorded without obliging anyone to act
on it, so nothing is suppressed and no reviewer is asked to pretend the code is clean. Findings are
declassified, never hidden, which keeps the pressure on the code rather than on the report.

**The brief carries one thing: intent.** What the change set out to do, stated flatly, as a spec
would state it. A reviewer handed a case for the change grades the case, and that is most of what
stands between a review and a rubber stamp. Scope is not a reviewer's question either: it was settled
before a plan existed, and a scope observation comes back as one MINOR line.

A severity, not a score. It says what is wrong and what that costs, where a number lets a weak
dimension hide behind strong ones and invites argument that cannot change the outcome. Two shapes
tried before this one, both of which never settled:

| What we asked for | What we got |
|---|---|
| "Report any issues you can find." | It always found one, however minuscule. No round came back empty. |
| "Stop when every criterion scores above 9/10." | Driving DbC past 9 pushed KISS under it, and the next round traded back. |

## Two behaviours worth knowing

**It refuses a commit that stages its own rubric.** Judging a change to the measure against that same
measure is circular, so a `--doc` file lands alone via `--no-verify`. This is the one place the tool
refuses rather than verifies, and it lives here because the list of rubrics IS the list of `--doc`
paths — a copy in bash would be free to drift from it.

**It removes a `Co-authored-by:` trailer whose address is `@anthropic.com`.** Every commit in a repo
gated this way is agent-written, so a fixed attribution line is constant and carries nothing, while
`reviewer=` already records who did the work. The match is on the address, so a human co-author
called Claude keeps their credit. This is the only case where the tool writes to the message.

## Developing this repo

The two linters the pre-commit hook calls are pinned at install time, so the hook stays a flat list
of commands with no wrapper script holding a version:

```bash
npm i -g annotated-tree@0.6.0
cargo install --git https://github.com/fredrikolis/cargo-lint-extra \
  --rev 7a232179e45414108d28047acd6315d9a2c4946b --locked cargo-lint-extra
git config core.hooksPath .githooks
```
