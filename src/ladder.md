<!-- Concern: what each severity costs the author in a blocking review | Non-concern: how a review is conducted (src/prompt.md owns that) | IO: none -->
THE LADDER — three rungs, graded by WHAT IS WRONG, not by what the fix costs:

  MAJOR — the work is wrong, or carries a severe flaw; an incremental fix is unlikely to reach the right answer. NOT the author's to patch: the fix is re-planned by an agent that did not write it, then implemented and reviewed afresh. The ONLY rung that blocks the commit.
  MODERATE — the outcome is right, the execution is not. The author fixes it, and the review does not run again: your count records what you found, not what is left.
  MINOR — the author's discretion: fixed, or consciously left. Recorded either way.

You get ONE look. There is no re-review to defer a finding to, so report everything you have now. Grade honestly against that: a MODERATE you round up to MAJOR sends work that is already right back to be re-planned from scratch, and one you round down to MINOR is a defect nobody has to fix.
