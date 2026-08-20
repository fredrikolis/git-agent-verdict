<!-- Concern: what this tool is worth to a repo running agents, the loop an agent follows to commit, and how to install it | Non-concern: the flags (--help) or the wiring (--repo-setup-guide) | IO: none -->
# git-agent-verdict

[![crates.io](https://img.shields.io/crates/v/git-agent-verdict.svg)](https://crates.io/crates/git-agent-verdict)
[![CI](https://github.com/fredrikolis/git-agent-verdict/actions/workflows/ci.yml/badge.svg)](https://github.com/fredrikolis/git-agent-verdict/actions/workflows/ci.yml)
[![license](https://img.shields.io/crates/l/git-agent-verdict.svg)](LICENSE)

`git-agent-verdict` is a CLI tool that blocks agents from committing code that does not comply with
your organization's standards. It uses git's built-in `commit-msg` hook to enforce that every commit
carries a passing verdict. Built for repos where an agent is the sole maintainer and no human is
expected to see the diff before it lands.

![you-shall-not-pass.gif](https://media2.giphy.com/media/v1.Y2lkPTc5MGI3NjExOGN1MTdmZzM3OWpwMXg0MjZrcGsyZmZ3MjN6Y2dsb3I3MzdydHc3byZlcD12MV9pbnRlcm5hbF9naWZfYnlfaWQmY3Q9Zw/YkfhemFXalh7O/giphy.gif)

**Review is mandatory**, and happens before the commit exists.

**No self-review.** The tool invokes the reviewer, not the agent, and the one line the agent supplies
is auto-rejected if it argues for the change instead of stating what it does.

**The verdict rides in the commit**, with the neutral intent as the subject. `git log` is the audit
trail: what changed, who reviewed it, what they found.

## Expected agent usage

The agent finds the tool by hitting it. Nothing has to be read first.

```
Bash(git commit -am "add retry to the uploader")
  ⎿ git-agent-verdict: error: standards: no reviewable trailer
    use `git agent-verdict attest --intent "<one line>"` to commit
```

The agent states in one neutral line what the diff does, spawns the reviewer, and waits for it.

```
Bash(git agent-verdict attest --intent "retry a failed upload three times")
  ⎿ git-agent-verdict: spawned attestation process (pid 48213)
    use `git agent-verdict await` to wait for it

Bash(git agent-verdict await)
  ⎿ git-agent-verdict: BLOCKED
    1-standards.log  # BLOCKED
    address the reported findings, then: git agent-verdict attest
```

The agent reads the log, fixes the MAJOR findings, and attests again. A gate that has passed is not
re-reviewed. Committing is its own call, so findings under a passing verdict are read before
anything lands.

```
Bash(git agent-verdict commit)
  ⎿ [main 3f9a1c2] retry a failed upload three times
    Reviewed-standards: reviewer=claude-opus-5 major=0 moderate=2 minor=1 token=1f0c...
    Reviewed-docs: reviewer=claude-opus-5 major=0 moderate=0 minor=0 token=8ba7...
```

## Install

```bash
cargo install git-agent-verdict
git config --global agent-verdict.runner claude
```

Installs as `git agent-verdict`.

## Configure a repo

Declare one gate per line in the `commit-msg` hook. Line order is review order, and the gate name
becomes the trailer key.

```bash
#!/usr/bin/env bash
set -euo pipefail
git agent-verdict --require-version 2.0

git agent-verdict "$1" standards --model opus \
  --standard programming --standard testing --path .

git agent-verdict "$1" docs --simple --doc docs/house-style.md --path "*.md"
```

`.git/hooks/commit-msg` is the standard location and needs no configuration, but git does not track
it, so each clone and each agent would install its own. To make the gates a property of the repo,
commit the hook and point git at it:

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
