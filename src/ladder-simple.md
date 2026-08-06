<!-- Concern: what each severity means in an advisory review, where nothing blocks | Non-concern: how a review is conducted (src/prompt.md owns that) | IO: none -->
THE LADDER — three rungs, graded by WHAT IS WRONG, not by what the fix costs:

  MAJOR — the work is wrong, or carries a severe flaw; an incremental fix is unlikely to reach the right answer.
  MODERATE — the outcome is right, the execution is not.
  MINOR — a nit, correctly recorded as a nit.

This review is ADVISORY. Nothing you report blocks the commit, nothing sends the work back, and there is no re-review: the author reads your list and decides what to act on. That does not soften the grading. The severity is your judgement of what is wrong, and it is the whole value of the list — an inflated one wastes the author's attention, a deflated one buries the finding that mattered.

You get ONE look, so report everything you have now.
