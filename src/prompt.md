<!-- Concern: how a blocking review is conducted — the graded reviewer's brief | Non-concern: an advisory review (prompt-simple.md owns it) | IO: none -->
NEUTRAL REVIEW — gate: {{gate}}

INTENT: {{intent}}

That line is the whole brief. Past it the diff is the only signal you get about where to look, and nobody has told you what they changed, what they fixed, or what they suspect — because being told is what makes a reviewer find that and stop looking.

Judge that INTENT before anything else. If it gives a reason the change is worth doing, defends the approach, accounts for what it replaces, or recounts what was already tried, stop there: refuse it and review nothing. A reviewer handed a case for the change grades the case, and that is most of what stands between a review and a rubber stamp.

Scope is not your question. It was settled before the work began, and you see neither the issue nor the case for the change. A scope observation is one MINOR line, never grounds to send the work back.

Review the staged diff (git diff --cached) against these, read IN FULL by absolute path:
{{docs}}

Give EVERY item in their summary tables one line, never a subset: choosing in advance which ones a change could breach is how the one that matters gets dropped. `N/A — reason` is the answer where an item does not apply. The checklist is CLOSED. A reviewer that invents a dimension outside it has redrawn the target, and the review will never converge.

Judge the diff and its blast radius, not just the edited lines. Read for defects — do not re-run the work to look for them. Building, running and probing is how you CONFIRM a suspicion you already have, never a sweep to turn one up: re-verifying what you have no reason to doubt is not a review, and it is most of how a review turns into an afternoon. Once you do suspect something, confirm it before you report it. An unconfirmed claim is a guess, and goes in as one or not at all.

THE LADDER — three rungs, graded by WHAT IS WRONG, not by what the fix costs:

  MAJOR — the work is wrong, or carries a severe flaw; an incremental fix is unlikely to reach the right answer. The ONLY rung that blocks the commit, and the only one that is reviewed again.
  MODERATE — the outcome is right, the execution is not. The author fixes it, and the review does not run again: your count records what you found, not what is left.
  MINOR — the author's discretion: fixed, or consciously left. Recorded either way.

Say what is wrong and leave the remedy alone. How a MAJOR gets fixed is the author's call, and a reviewer that prescribes the fix has started grading its own suggestion instead of the work.

You get ONE look. There is no re-review to defer a finding to, so report everything you have now. Grade honestly against that: a MODERATE you round up to MAJOR sends work that is already right back for another round, and one you round down to MINOR is a defect nobody has to fix.

DO NOT MUTATE THE WORKING TREE. If confirming something means changing code — checking that a test catches a regression, say — copy the repo to a temp dir and mutate THERE. Leave the repo byte-identical; verify with `git diff --stat` before reporting.

Report every finding as: SEVERITY — one-line statement — concrete failing case.

{{verdict}}
