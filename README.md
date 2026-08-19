<!-- Concern: what git-agent-verdict is, why it exists, and how to install and wire it | Non-concern: the flag grammar (--help owns it) or the reviewer's brief (src/prompt.md) | IO: none -->
# git-agent-verdict

Verifies that a commit message carries an attested review verdict, and blocks on any declared
blocker. It is blind to what the standards say: the reviewer reads the standard, the gate reads a
verdict.

Built for repos where commits are written by agents. No human is expected to write a trailer.

Every repo that wants agent-written commits reviewed grows the same few hundred lines of bash to
demand and check an attestation, and every copy drifts. This is that machinery, once, outside them.

## Install

```bash
cargo install git-agent-verdict
```

Installs as `git agent-verdict`. `--help` is the flag reference; this file is not.

## Wiring it into a repo

`git agent-verdict --repo-setup-guide` prints the whole setup: the `commit-msg` hook to write, the
two `git config` lines it needs, and every flag a gate declaration takes. It is pinned to the
installed version and answers outside a repo. A declaration that no longer parses prints it too,
because the agent that hits one has no other way to reach this file.

What the guide leaves to this file:

- **Gate names are repo-chosen labels**, not keywords, and each reaches the trailer as
  `Reviewed-<name>`.
- **Line order is review order.** `attest` takes gates one at a time in that order, so a later gate
  is never judged against content an earlier one is still changing. The tool re-runs the hook to read
  the declarations rather than keeping a second copy in a config file.
- **`--model` passes through unchecked.** A name the agent will not answer for fails the run and names
  the gate that declared it: that is the hook's wiring, not the commit's.
- **`--require-version` pins a compatibility line.** Any later `1.x` satisfies `1.14`. An older build
  is refused as stale, and `2.0.0` as a different line, because a major release may take a flag away
  or change what a trailer must carry. Without it, a hook written against the old grammar finds out
  when a commit dies on an unknown flag.
- **Most gates should be `--simple`.** One gate holds the bar, the rest report. A repo that blocks on
  every dimension it cares about spends its budget on the ones that were never going to sink the
  change.
- **`--read-only`** runs a gate's reviewer in a mode that cannot write. Declare it where someone else
  is working in the tree.

### Bundled standards

`--standard` names a rubric shipped inside the binary, so a repo can gate on a general measure
without hosting or copying one.

| Name | What it judges |
| ---- | -------------- |
| `programming` | Language-agnostic design principles: data flow, boundaries, contracts, canonical form. |
| `testing` | Which assertions earn a committed test, and which are scratch. |
| `cli` | The command-line surface an agent invokes: flags, streams, exit codes, envelopes. |
| `frontend` | Component architecture, framework-neutral: data flow, boundaries, lifecycle, state. |
| `agent-communication` | Prose an agent acts on: findings not explanations, dense and scannable. |
| `human-communication` | Prose a person reads: a README, a guide, an error message. |
| `terse-log` | Entries in an append-only log: changelog, roadmap, worklog, decision record. |
| `minimal-docs` | How much documentation a repo carries. |

`git agent-verdict --standards` lists them, and `--standards <name>` prints one in full. They live as
Markdown in [`standards/`](standards/) and the build bundles the folder in, so adding one is a file,
never a code change. Reading them from the binary rather than fetching them keeps a review off the
network and stops a rubric changing under a repo between two runs of the same commit.

Where a whole document is more than the check is worth, `--rule` states a measure inline. A gate
needs at least one `--standard`, `--doc` or `--rule`, and carries any mix.

### Who reviews

Host configuration, not the repo's: maintainers of one repo do not share a machine, a budget or a
preferred agent.

```bash
git config --global agent-verdict.runner claude     # every repo on this machine
git config --local  agent-verdict.runner claude     # this clone only
```

There is no default. Unset, `attest` refuses rather than spending on an agent nobody chose. The value
names an agent this build knows how to drive, not a command line. `claude` is the one it knows.

## How it goes

The agent is never briefed on the review process. It is told to do the work, it does it, and it tries
to commit. The commit fails by design, naming the trailer the gate wanted and printing one command to
run instead: `git agent-verdict attest`. The message file written for that commit is discarded.

`attest` reviews the next gate itself, records what the reviewer reported, and says what to fix. Run
it until it stops complaining; the last run has no gate left and commits. The verdict lands on
stdout, and the report is written to `~/.agent-verdicts/` with its path printed.

Fixing what a review named re-opens that gate, so the next `attest` reviews it again. The loop ends
when the editing does, and a trailer never attests text that is no longer there.

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

Running `attest`:

- **`--intent` is taken on the first run of a commit only.** It is recorded, becomes the commit's
  subject line verbatim, and a later run that passes one again is refused. `reset` changes it.
- **`--repo` is the repo root, absolute**, and the shell's directory is never consulted. An agent
  holding one shell open across a long task is often not standing where it believes.
- **The index and the working tree must agree** on every file a gate reviews. The reviewer opens
  those files and the commit carries the index, so a disagreement is refused rather than attested.
- **Run it directly, with no wait loop.** A second `attest` refuses at once, saying how long the claim
  has been held and naming what holds it, where the system will say. The claim lives
  on a file descriptor the reviewer inherits, so it outlives a run killed from outside. A hand-built
  `pgrep -f` guard matches the wrapping shell's own arguments and waits for ever; the tell is that
  nothing runs at all.
- **The ceiling is 30 minutes**, raised with `--timeout`. A review that stops answering is killed and
  reported. A reviewer that crashes, answers with no verdict, or is cut off mid-answer exits 2
  carrying what it said, so a failed review never reads as a clean one.

Before each review starts, `attest` prints the reviewer's transcript path and a `jq` command that
tails the latest activity, so "is it still going" costs one line of output and nothing while nobody
asks.

## Auditing after a rubric changes

`attest` reviews the staged diff. When a rubric itself changes, what it now condemns is mostly in
code nobody is touching, and no diff will ever show it. `audit` reviews the tree instead:

```console
$ git agent-verdict audit --repo /home/me/src/my-repo \
    --confirm-reviewing-the-whole-repo-not-a-commit \
    --confirm-running-in-background-shell-with-long-timeout
```

One full review per gate, on every tracked file that gate's pathspec reaches. It records and commits
nothing, because a trailer attests one commit and there is no commit here. What it produces is the
report, which the author acts on in commits attested from their own diffs. Exit 1 if any gate
reported a MAJOR, 2 if a reviewer failed.

## What the agent supplies

Nothing is handed to the agent to forward. The brief goes from the tool to the reviewer and the
counts come back the same way. The agent supplies `--intent` and nothing else, and two checks hold it
honest:

- **The limit** is one line, 300 characters, bounding the change rather than the prose. An aim that
  cannot be said in a line is more than one change, and the error says so.
- **The reviewer** catches what fits and still argues. Handed a brief that gives a reason the change
  is worth doing, defends the approach, or recounts what was tried, it answers `VERDICT: refused` and
  reviews nothing. That refusal blocks on an advisory gate as firmly as on a blocking one, which a
  `major=1` refusal could never do.

Scope is not a reviewer's question either: it was settled before a plan existed, and a scope
observation comes back as one MINOR line. The commit `attest` makes runs the hook like any other, so
the gates verify it exactly as they would verify a commit written by hand.

## The trailer

```
Reviewed-standards: reviewer=claude-opus-5 major=0 moderate=1 minor=3 token=6f1d…
Reviewed-prose: reviewer=claude-opus-5 major=0 moderate=2 minor=0 token=b204…
Reviewed-annotations: reviewer=claude-opus-5 major=0 moderate=0 minor=1 token=9ca7…
```

One per gate, with named fields so a field can be added without changing the arity. Trailers must be
the message's last paragraph, which is what `git interpret-trailers --parse` reads.

- **Scope** is the gate's own pathspec, handed to the reviewer as the command that applies it, so an
  unscoped diff never reaches it.
- **`reviewer=`** is the model that answered. It and the session id naming the transcript are read
  from the agent, not asked of it. The session stays in the diary, unpushed, because a commit cannot
  be unpublished.
- **`token=`** names the recorded review. The gate resolves it and compares the counts in the message
  against the ones the reviewer actually reported, which catches a trailer reading better than its
  review did.
- **An advisory gate** grades on the same ladder with no MAJOR rung. It reports `major=0`, and nothing
  it finds blocks the commit. One count shape everywhere, and `major=` is the count that reaches zero.
  A gate whose only count cannot reach zero gives a review no place to stop.

A reviewer reporting `major=0` has committed to a number it can be held to, which matters precisely
because the writer is an LLM. `moderate=1` on a passing commit is not an outstanding defect: it was
fixed, and the count records that the review found it.

**Trust model: a diary, not a vault.** `--no-verify` exists, and so does an unset `core.hooksPath`.
Nothing here resists an author who means it. What the recorded review buys is that a count cannot be
edited by accident, retyped by whoever read the review, or found by grep. Everything past that is
attribution, not enforcement: a false attestation is a claim on the record with a name on it.

## The ladder

Three rungs, graded by what is wrong, not by what the fix costs:

- **MAJOR**: the work is wrong, or carries a severe flaw, and an incremental fix is unlikely to reach
  the right answer. The only rung that blocks, and the only one reviewed again. The remedy is the
  author's call: a reviewer that prescribes one has started grading its own suggestion.
- **MODERATE**: the outcome is right, the execution is not. The author fixes it and does not go back
  for a second opinion on the fix.
- **MINOR**: the author's discretion, fixed or consciously left, recorded either way. A finding
  obliges nobody to act on it, so nothing is suppressed and no reviewer is asked to pretend the code
  is clean.

**`reset <reason>` is the escape valve, and it is loud.** It clears the recorded reviews and the
recorded intent for this commit, and records why. The count and every reason reach the commit
message, so a run restarted three times says so rather than looking like a clean first pass.

## Two behaviours worth knowing

**A rubric is not reviewable here, and neither is the hook.** A change to either would be reviewed by
the maintainer who made it, so staging one is refused. It lands on its own with `git commit
--no-verify`, and the work behind it comes in a commit of its own. A rubric kept outside the repo,
`$KB/standards.md` expanded by the hook's own shell, is never staged and never meets this at all.

**It removes a `Co-authored-by:` trailer whose address is `@anthropic.com`.** Every commit in a repo
gated this way is agent-written, so a fixed attribution line carries nothing, while `reviewer=`
already records who did the work. The match is on the address, so a human co-author called Claude
keeps their credit. This is the only case where the tool writes to the message.

## Developing this repo

The two linters the pre-commit hook calls are pinned at install time, so the hook stays a flat list
of commands with no wrapper script holding a version:

```bash
npm i -g annotated-tree@0.6.0
cargo install --git https://github.com/fredrikolis/cargo-lint-extra \
  --rev 7a232179e45414108d28047acd6315d9a2c4946b --locked cargo-lint-extra
git config core.hooksPath .githooks
```
