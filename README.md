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
git agent-verdict --require-version 1.7
git agent-verdict "$1" standards   --model opus --doc docs/repo-standards.md --path .
git agent-verdict "$1" annotations --simple --model haiku --doc docs/annotation-guide.md --path .
git agent-verdict "$1" prose       --simple --doc docs/communication-style.md \
                                   --rule "each entry is one half-sentence" --path "*.md"
```

Gate names are repo-chosen labels, not keywords. Line order is review order, and `attest` takes them
one at a time in that order: a later gate must never be judged against content an earlier one is
still changing.

`--model` names the model that reviews a gate, passed to the agent exactly as written and never
checked against a list here — that list would go stale, and the agent already answers for a name it
does not know. An annotation check and a correctness review are not worth the same model, and which
is which is the repo's call. Omitted, the agent picks. A model the agent will not answer for fails
the run and names the gate that declared it: that is the hook's wiring, not the commit's.

`--rule` states a measure inline where a whole document would be more than the check is worth. A
gate needs at least one `--doc` or `--rule`, and carries both where it wants both.

Who reviews is **host** configuration, not the repo's — maintainers of one repo do not share a
machine, a budget or a preferred agent:

```bash
git config --global agent-verdict.runner claude     # every repo on this machine
git config --local  agent-verdict.runner claude     # this clone only
```

There is no default: unset, `attest` refuses rather than spending on an agent nobody chose. The
name is an agent this build knows how to drive, not a command line — resuming a session, carrying
standing instructions and reading an answer back differ enough between agents that a repo cannot
express them in one line of shell. `claude` is the one it knows.

The verdict lands on stdout; the report is written to `~/.agent-verdicts/` and its path printed.

Each line is the whole declaration of its gate, which is what lets the tool brief a reviewer exactly
as that gate will judge — it re-runs this hook to read the declarations rather than keeping a second
copy in a config file.

A pin, not a floor. `1.7` names a compatibility line: every additive `1.x` satisfies it, and `2.0.0`
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
git-agent-verdict: error: standards: no reviewable trailer

  missing: the message needs this trailer and has none
  wanted:  Reviewed-standards: reviewer=<id> major=<n> moderate=<n> minor=<n> token=<issued>

  git agent-verdict attest --repo /home/me/src/my-repo --intent "<the aim, one flat line>"

  - runs each gate in turn, records what the reviewer reported
  - commits once every gate is attested
  - composes the message from --intent; this message file is discarded
```

`attest` reviews the next gate itself, records what the reviewer reported, and says what to fix. Run
it until it stops complaining; the last run has no gate left and commits.

Fixing what a review named re-opens its gate, so the next `attest` reviews it again. The loop ends
when the editing does, and a trailer never attests text that is no longer there. `--intent` is only
needed on the first run of a commit; the aim is held, and may not change without a MAJOR.

```console
$ git agent-verdict attest --repo /home/me/src/my-repo \
    --intent "the commit-msg hook delegates verdict verification to a CLI" \
    --confirm-running-in-background-shell-with-long-timeout
git-agent-verdict: judging the intent…
standards: major=0 moderate=1 minor=1

see the full report: ~/.agent-verdicts/my-repo-3f9a1c04/4da9793…-1-standards.log

agent-verdict gates mandated by repo:
  standards    [PASSED - major=0 moderate=1 minor=1]
  annotations  [PENDING]
  prose        [SKIPPED - nothing staged matches *.md]

next: address the findings, then attest again for annotations
```

A review prints what the reviewer is doing as it does it, one line per event, read from the
transcript the agent writes while it works:

```
git-agent-verdict: standards: reviewing — session eda8a571…, pid 2603531
  · » I'll review the repository's Rust files against the style gate criteria…
  · Bash("git ls-files -- '*.rs'", "List all Rust files in the repository")
  · Read(…/livecheck/lib.rs")
```

One line per event, never the event: a single tool result runs to 16 KB and a whole transcript to
megabytes, so a caller handed the bytes would have the review pasted into it in place of being told
the review is running. Where nothing happens at all, the elapsed time still prints every minute.

A review that stops answering is killed and reported, rather than left for whatever shell is holding
the run to kill without a word: the ceiling is 30 minutes, and `--timeout <minutes>` raises it where
a review here is genuinely longer. A reviewer that crashes, answers with no verdict, or is cut off
mid-answer exits 2 carrying what it said — a failed review never reads as a clean one.

Run `attest` directly. It holds the repo for as long as it runs, and a second one refuses at once,
naming the pid that holds it and how long it has been held — so there is nothing to guard against
and no reason to wrap it in a wait loop. A hand-built guard is where this goes wrong: a `pgrep -f`
on the attest command line matches the wrapping shell's own arguments and waits for ever, and the
tell is that nothing runs at all.

`--repo` is the repo root, absolute, and the shell's directory is never consulted. An agent holding
one shell open across a long task is often not standing where it believes; naming the tree puts that
assumption in the command line, where the transcript records it. `attest` also refuses while the
index and the working tree disagree on any file a gate reviews — the reviewer opens those files, and
the commit carries the index.

## Auditing after a rubric changes

`attest` reviews the staged diff. When a rubric itself changes, what it now condemns is mostly in
code nobody is touching, and no diff will ever show it. `audit` reviews the tree instead:

```console
$ git agent-verdict audit --repo /home/me/src/my-repo \
    --confirm-reviewing-the-whole-repo-not-a-commit \
    --confirm-running-in-background-shell-with-long-timeout
```

One full review per gate, on every tracked file that gate's pathspec reaches. It records nothing and
commits nothing: a trailer attests one commit, and there is no commit here. What it produces is the
report, which the author acts on in commits attested from their own diffs. Exit 1 if any gate
reported a MAJOR, 2 if a reviewer failed.

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
Reviewed-prose: reviewer=claude-opus-5 major=0 moderate=2 minor=0 token=b204…
Reviewed-annotations: reviewer=claude-opus-5 major=0 moderate=0 minor=1 token=9ca7…
```

One per gate. Named fields, so a field can be added without changing the arity. Trailers must be the
message's last paragraph — that is what `git interpret-trailers --parse` reads.

Scope is the gate's own pathspec, handed to the reviewer as the command that applies it, so an
unscoped diff never reaches it. `reviewer=` is the model that answered and the session id naming its
transcript are read from the agent, not asked of it: one it would guess at, the other it cannot
know. The session stays in the diary, unpushed, because a commit cannot be unpublished.

An advisory gate grades on the same ladder and has no MAJOR rung: it reports `major=0`, and nothing
it finds blocks the commit. One count shape everywhere, and `major=` is the count that reaches zero.
A gate whose only count cannot reach zero gives a review no place to stop, and the author decides
when to stop by feel — which is how a review turns into rounds of taste.

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

**A rubric is not reviewable here, and neither is the hook.** The hook names the gates and a
`--doc` is what one judges against; a change to either is reviewed by the maintainer who made it,
which is no review at all. Staging one is refused rather than reviewed, and it lands on its own
with `git commit --no-verify` — then the work behind it comes in a commit of its own. The friction
is the point: what a repo gates by should not move quietly alongside the code it gates. A rubric
kept outside the repo, `$KB/standards.md` expanded by the hook's own shell, is never staged and
never meets this at all.

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
