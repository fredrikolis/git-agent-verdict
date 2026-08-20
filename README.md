<!-- Concern: what this tool is worth to a repo running agents, the loop an agent follows to commit, and how to install it | Non-concern: the flags (--help) or the wiring (--repo-setup-guide) | IO: none -->
# git-agent-verdict

[![crates.io](https://img.shields.io/crates/v/git-agent-verdict.svg)](https://crates.io/crates/git-agent-verdict)
[![CI](https://github.com/fredrikolis/git-agent-verdict/actions/workflows/ci.yml/badge.svg)](https://github.com/fredrikolis/git-agent-verdict/actions/workflows/ci.yml)
[![license](https://img.shields.io/crates/l/git-agent-verdict.svg)](LICENSE)

git-agent-verdict is a CLI tool that makes an independent review a condition of the commit. An agent
that reviews its own work passes its own work. Run it unattended and it writes the code, grades the
code, commits, and the only record is a sentence it wrote about itself.

![you-shall-not-pass.gif](https://media2.giphy.com/media/v1.Y2lkPTc5MGI3NjExOGN1MTdmZzM3OWpwMXg0MjZrcGsyZmZ3MjN6Y2dsb3I3MzdydHc3byZlcD12MV9pbnRlcm5hbF9naWZfYnlfaWQmY3Q9Zw/YkfhemFXalh7O/giphy.gif)

**Review is mandatory.** Every commit touching a gated path. Not the ones the agent felt unsure about.

**The author never grades the author.** A separate reviewer, briefed on the rubric you committed, not
on the model's own taste.

**The verdict is the commit.** Reviewer, verdict and finding counts ride in the message, so `git log`
answers who reviewed a change and what they found.

## Expected agent usage

The agent finds the tool by hitting it. Nothing has to be read first.

```console
$ git commit -am "add retry to the uploader"

git-agent-verdict: error: standards: no reviewable trailer

This repository mandates `git agent-verdict` for all commits. To commit:

  git agent-verdict attest --repo /home/you/proj --intent "<intent: one line, at most 300 characters>"

That runs this repository's 2 gates, `standards` then `annotations`, in declaration order, and stops
at the first MAJOR. MODERATE and MINOR are recorded, not blocking. Address the MAJOR findings and run
attest again with no --intent, until every gate has passed. Then address the remaining MODERATE and
MINOR findings at your discretion and:

  git agent-verdict commit --repo /home/you/proj
```

`attest` spawns the review and returns. It never blocks the caller, and it never commits.

```console
$ git agent-verdict attest --repo /home/you/proj --intent "retry a failed upload three times"

git-agent-verdict: spawned attestation process (pid 48213)
/home/you/proj/.git/agent-verdict/9f2c1ab/
Use `git agent-verdict await --repo /home/you/proj` to wait for it.
Do not poll with pgrep, sleep or any combination of them: those guards match their own shell and can
stall for hours. If your harness interrupts the await, run it again.
```

`await` is the only waiter. It returns the verdict and lists what every gate wrote.

```console
$ git agent-verdict await --repo /home/you/proj

git-agent-verdict: BLOCKED
/home/you/proj/.git/agent-verdict/9f2c1ab/
  1-standards.log  # BLOCKED
address the reported findings, then: git agent-verdict attest --repo /home/you/proj
```

The agent reads `1-standards.log`, fixes the MAJOR findings, and attests again. A gate that has
passed is not re-reviewed.

```console
$ git agent-verdict await --repo /home/you/proj

git-agent-verdict: PASSED
/home/you/proj/.git/agent-verdict/9f2c1ab/
  1-standards.log  # BLOCKED
  2-standards.log  # PASSED
  3-annotations.log  # PASSED
git agent-verdict commit --repo /home/you/proj
```

Committing is a separate verb, so the findings under a passing verdict are read before anything
lands. The subject is the intent the reviewers were briefed on.

```console
$ git agent-verdict commit --repo /home/you/proj

[main 3f9a1c2] retry a failed upload three times
  Reviewed-standards: reviewer=claude-opus-5 major=0 moderate=2 minor=1 token=1f0c...
  Reviewed-annotations: reviewer=claude-opus-5 major=0 moderate=0 minor=0 token=8ba7...
```

## Install

```bash
cargo install git-agent-verdict
git config --global agent-verdict.runner claude
```

Installs as `git agent-verdict`.

## Configure a repo

Declare one gate per line in `.githooks/commit-msg`, tracked and executable. Line order is review
order, and the gate name becomes the trailer key.

```bash
#!/usr/bin/env bash
set -euo pipefail
git agent-verdict --require-version 2.0

git agent-verdict "$1" standards --model opus \
  --standard programming --standard testing --path .

git agent-verdict "$1" docs --simple --doc docs/house-style.md --path "*.md"
```

```bash
chmod +x .githooks/commit-msg
git config core.hooksPath .githooks
```

`git agent-verdict --repo-setup-guide` prints the full reference: every flag a declaration takes,
what `--simple` and `--read-only` change, and how to feed a gate a rubric that lives outside the
repo. `--help` is the flag grammar.

## Contributing

The reviewer runs on Claude Code today. **Please help us add support for other agents.** The seam is
`src/runner.rs` and `src/agent.rs`: an agent is an argv, a session, and an answer this tool parses
verdicts out of. Nothing above that layer knows which one answered.
