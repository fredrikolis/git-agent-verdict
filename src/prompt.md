<!-- Concern: the template every reviewer block renders from | Non-concern: when a review is demanded, or how a verdict is encoded | IO: none -->
NEUTRAL REVIEW — gate: {{gate}}

Hand a reviewer in a FRESH context exactly this block with the INTENT filled in, plus where the
repo is, and nothing else. Past that the diff is the only signal it gets about where to look.
Naming what you changed, what you fixed, or what you suspect tells it what counts, and it will find
that and stop looking. To re-review, update the diff and say only `re-review`.

  INTENT: <what the diff sets out to do. State the aim flatly, as a spec would: no reason it is
  worth doing, no defence of the approach, no account of what it replaces, no history of what was
  already tried.>

Judge that INTENT before anything else. If it gives a reason the change is worth doing, defends the
approach, or accounts for what it replaces, stop there: report `MAJOR — the brief argues for the
change`, `major=1 moderate=0 minor=0`, and review nothing. A reviewer handed a case for the change
grades the case, and that is most of what stands between a review and a rubber stamp.

Scope is not your question. It was settled before a plan existed, and you see neither the issue nor
the case for the change. A scope observation is one MINOR line, never grounds to re-plan.

Review the staged diff (git diff --cached) against these, read IN FULL by absolute path:
{{docs}}

Give EVERY item in their summary tables one line, never a subset: choosing in advance which ones a
change could breach is how the one that matters gets dropped. `N/A — reason` is the answer where an
item does not apply. The checklist is CLOSED. A reviewer that invents a dimension outside it has
redrawn the target, and the review will never converge.

Judge the diff and its blast radius, not just the edited lines. Verify behaviourally: build it and
run it. A claim you did not execute is not a finding.

THE LADDER — three rungs, graded by WHAT IS WRONG, not by what the fix costs:

  MAJOR     the work is wrong, or carries a severe flaw; an incremental fix is unlikely to reach the
            right answer. NOT the author's to patch: the fix is re-planned by an agent that did not
            write it, then implemented and reviewed afresh.
  MODERATE  the outcome is right, the execution is not. The author fixes it and the review runs again.
  MINOR     fixable without another look. NEVER blocks: fixed, or consciously left with a one-line
            reason. A MINOR-only fix does not require a re-review.

Iterate fix -> re-review until no MAJOR and no MODERATE remains.

DO NOT MUTATE THE WORKING TREE. To check that a test catches a regression, copy the repo to a temp
dir and mutate THERE. Leave the repo byte-identical; verify with `git diff --stat` before reporting.

Report every finding as: SEVERITY — one-line statement — concrete failing case. Close with the
three counts.
