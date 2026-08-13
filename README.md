<!-- Concern: what git-agent-verdict is, why it exists, and how to install and wire it | Non-concern: the flag grammar (--help owns it) or the reviewer's brief (src/prompt.md) | IO: none -->
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

`git agent-verdict --repo-setup-guide` prints this section from the binary, pinned to the installed
version and answering outside a repo. A declaration that no longer parses prints it too: stale
wiring is the maintainer's to fix, and the agent hitting it has no other way to reach this file.

`.githooks/commit-msg`, enabled per clone with `git config core.hooksPath .githooks`:

```bash
#!/usr/bin/env bash
set -euo pipefail
git agent-verdict --require-version 1.0
git agent-verdict --rubric-guard \
  --doc docs/repo-standards.md --doc docs/annotation-guide.md --doc docs/communication-style.md
git agent-verdict "$1" standards   --doc docs/repo-standards.md --path .
git agent-verdict "$1" annotations --simple --doc docs/annotation-guide.md --path .
git agent-verdict "$1" prose       --simple --doc docs/communication-style.md --path README.md
```

Gate names are repo-chosen labels, not keywords. Line order is review order, and `attest` takes them
one at a time in that order: a later gate must never be judged against content an earlier one is
still changing.

Who reviews is **host** configuration, not the repo's — maintainers of one repo do not share a
machine, a budget or a preferred agent:

```bash
git config --global agent-verdict.runner "claude -p"     # every repo on this machine
git config --local  agent-verdict.runner "…"             # this clone only
```

There is no default: unset, `attest` refuses rather than spending on an agent nobody chose.
`claude -p` is one runtime among many. The tool composes no argv of its own and acquires no model,
key or SDK: it runs that command line with the brief on stdin and reads the verdict back. The
command must report `reviewer=` and `session=` on the VERDICT line, and the brief says so — a
wrapper piping through `jq` lifts both out of `--output-format json`. Rewrite the VERDICT line and pass the rest through —
a wrapper that filters to the verdict alone throws away the only part that says what was wrong.

The verdict lands on stdout; the report is written to `~/.agent-verdicts/` and its path printed. Neither is defaulted: a
runner that omits one has broken the contract the brief states, and a label invented here would put
a guess on the record.

Each line is the whole declaration of its gate, which is what lets the tool brief a reviewer exactly
as that gate will judge — it re-runs this hook to read the declarations rather than keeping a second
copy in a config file.

A pin, not a floor. `1.0` names a compatibility line: every additive `1.x` satisfies it, and `2.0.0`
will not, because a major release may take a flag away or change what a trailer must carry. A hook
written against the old grammar cannot tell on its own, and finds out when a commit dies on an
unknown flag. Both directions are refused: too old cannot answer what the hook asks, and a later
line answers something else. One line in the hook, rather than a version-compare beside every gate.

Most gates are `--simple` above, and that is the usual shape: one gate holds the bar the work has to
clear, and the rest report. A repo that blocks on every dimension it cares about spends its review
budget on the ones that were never going to sink the change.

## How it goes

The agent is never briefed on the review process. It is told to do the work, it does it, and it
tries to commit. The commit fails by design, and the gate prints one command.

```console
$ git commit -m "…"
git-agent-verdict: standards: REVIEW GATE FAILED

MISSING — the message needs this trailer and has none

  Reviewed-standards: reviewer=<id> major=<n> moderate=<n> minor=<n> token=<issued>

Earned by a review this tool runs for you:

  git agent-verdict attest --intent "<the aim of the change, in one flat line>"
```

`attest` reviews the next gate itself, records what the reviewer reported, and says what to fix. Run
it until it stops complaining; the last run has no gate left and commits.

```console
$ git agent-verdict attest --intent "the commit-msg hook delegates verdict verification to a CLI"
git-agent-verdict: standards: reviewing…
standards: major=0 moderate=1 minor=1

see the full report: ~/.agent-verdicts/my-repo-3f9a1c04/4da9793…-1-standards.log

Address what it found, then run attest again for the annotations gate.
```

**Nothing is handed to the agent to forward.** The brief goes from the tool to the reviewer, and the
counts come back the same way. The agent supplies one thing — `--intent`.

Two things hold that one input honest, and they catch different failures. The **limit** — one line,
300 characters — bounds the *change*, not the prose: an aim that cannot be said in a line is more
than one change, and the error says so rather than asking for a shorter sentence. The **reviewer**
catches what fits and still argues: handed a brief that gives a reason the change is worth doing,
defends the approach, or recounts what was tried, it answers `VERDICT: refused` and reviews nothing.
A reviewer handed the case for a change grades the case. That refusal blocks on an advisory gate as
firmly as on a blocking one, which is what a `major=1` refusal could never do.

The commit `attest` makes runs the hook like any other, so the gates verify it exactly as they would
verify a commit written by hand.

## The trailer

```
Reviewed-standards: reviewer=claude-opus-5 major=0 moderate=1 minor=3 token=6f1d…
Reviewed-prose: reviewer=claude-opus-5 findings=2 token=b204…
Reviewed-annotations: reviewer=claude-opus-5 major=0 moderate=0 minor=1 token=9ca7…
```

One per gate. Named fields, so a field can be added without changing the arity. Trailers must be the
message's last paragraph — that is what `git interpret-trailers --parse` reads.

Scope is the gate's pathspec against the index, and the brief says so: it lists the staged files
under review and states there are no others. `reviewer=` is what the runner reported; the session id
that names its transcript stays in the diary, unpushed, because a commit cannot be unpublished.

An advisory gate carries `findings=` instead of three counts. Nothing it reports blocks, so a
severity would buy nothing and cost the reviewer the attention the next defect needs.

`token=` names the recorded review. The gate resolves it and compares the counts in the message
against the ones the reviewer actually reported — the one check that catches a trailer reading
better than its review did.

The counts are the whole verdict, and asking for three numbers rather than a verdict is deliberate:
a reviewer that reports `major=0` has committed to a number it can be held to, which matters
precisely because the writer is an LLM. `moderate=1` on a passing commit is not an outstanding
defect. It was fixed, and the count records that the review found it.

**Trust model — a diary, not a vault.** `--no-verify` exists, and so does an unset `core.hooksPath`.
Nothing here resists an author who means it, and it is not trying to: the whole design is aimed at
making the honest path the shortest one. What the recorded review buys is that a count cannot be
edited by accident, retyped by whoever read the review, or found by grep. Everything past that is
attribution, not enforcement — a false attestation is a claim on the record with a name on it.

## The ladder

Three rungs, graded by what is wrong, not by what the fix costs:

- **MAJOR**: the work is wrong, or carries a severe flaw; an incremental fix is unlikely to reach the
  right answer. The only rung that blocks, and the only one reviewed again. What to do about it is
  the author's call — a reviewer that prescribes the remedy has started grading its own suggestion.
- **MODERATE**: the outcome is right, the execution is not. The author fixes it and does not go back
  for a second opinion on the fix.
- **MINOR**: the author's discretion — fixed, or consciously left. Recorded either way.

**The review runs once.** A review-and-fix loop fails in two directions, and the band between them is
narrow. Too tight and nothing ever settles: every nit blocks, so no round comes back clean. Too loose
and the reviewer rubber-stamps. The loop itself was most of the cost — a full second review to
confirm fixes the same reviewer had already specified — and MODERATE is what buys it away: real
enough to have to fix, small enough that the fix does not need looking at. MAJOR is the one case
where another review is genuinely cheaper than the alternative, and it re-runs that gate alone.

**`reset <reason>` is the escape valve, and it is loud.** It clears the recorded reviews for this
commit and records why. The count and every reason reach the commit message, so a run that was
restarted three times says so on the record rather than looking like a clean first pass.

**MINOR is what keeps the count honest.** A finding can be recorded without obliging anyone to act
on it, so nothing is suppressed and no reviewer is asked to pretend the code is clean. Findings are
declassified, never hidden, which keeps the pressure on the code rather than on the report.

**The brief carries one thing: intent.** What the change set out to do, stated flatly, as a spec
would state it — and it becomes the commit's subject line, because that is the same sentence written
for the same reason. Scope is not a reviewer's question either: it was settled before a plan existed,
and a scope observation comes back as one MINOR line.

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
