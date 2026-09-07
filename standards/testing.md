<!-- Concern: decides which assertions earn a committed test | Non-concern: test tooling and framework choice | IO: none -->
# Universal Testing Principles

Every assertion is a freeze decision — writing `assert X == Y` says "future agents must update this test to change this behavior." Tests are deliberate governance, not free insurance.

## Decision rule (in order)

1. External contract, consumers you can't coordinate? → **freeze** (commit a regression test).

2. You control both sides of this interface? → **don't freeze** (DbC: validate at the boundary, update call sites together instead).

3. Already caught by an e2e/integration test at a higher level? → **don't freeze** (redundant).

4. Would this test block a principled refactor? → **delete the test**, fix the architecture — never compromise the design to keep a test green.

## Scoring (positive = commit, negative = skip/delete)

External contract +10 · leaf-node stable abstraction +9 · high downstream dependency +8 · edge case not caught by e2e +8 · single internal caller -5 · glue/orchestration code -9 (test via e2e) · implementation detail, not interface -7 · already caught by e2e -8 · you control both sides -10 · blocks a principled refactor -∞ (delete it).

## Freeze vs. don't freeze

Freeze: external consumers you can't coordinate, high downstream dependency, or a format/protocol that's both unlikely to change and important that it doesn't.

Don't freeze: internal seams where you control all callers (including a frontend/backend split inside the same app/repo — that's internal, not a public API), glue/orchestration code, or implementation detail (private methods, internal state) — that freezes "how," not "what."

## Not traditional TDD

Tests aren't written first and all-committed. Workflow: write code, write a scratch test to *prove to yourself* it works, keep the scratch test in `artifacts/` (gitignored, evidence for review — not committed). Only promote to a committed regression test if the freeze criteria above say the contract deserves it.

| Type | Purpose | Assertions | Committed? |
|---|---|---|---|
| Scratch | prove it works to the author | yes | no — gitignored in `artifacts/` |
| Walkthrough/tour | demonstrate usage to future agents | none (LLM reads the output) | yes |
| Regression | freeze an external contract | yes, boundaries only | yes, forever |

## Architectural position

Leaf nodes (stable abstractions): freeze the interface — would the test still make sense if the class were swapped out? Glue/orchestration: don't freeze the how, let e2e verify the integration. External APIs: freeze shape/format/semantics. Internal APIs (you own both sides): don't freeze, update call sites together.

Two failure modes a frozen test can catch: an **external** change (e.g. a library update shifts a format) — assert on shape/contract, treat a failure as informational, go investigate. An **internal accident** (a developer changes behavior unintentionally) — assert on the specific behavior, treat a failure as preventive, block the change.

Distribution: few e2e (critical paths), moderate integration (key boundaries), minimal unit (only stable leaf interfaces), many scratch (during dev, never committed).

## Test representativeness

Every test is a production approximation, and every divergence from production is a blind spot: mocked dependencies hide integration/contract drift, simplified data hides edge cases and scale, single-threaded execution hides races, local execution hides latency/DNS/timeouts, clean state each run hides accumulation bugs, deterministic ordering hides order-dependent failures.

Not a rule to eliminate — a lens: before taking a shortcut, name what it might stop you from catching, and document the known blind spot.

## Anti-patterns

Testing implementation details (freezes "how," test the interface instead) · redundant coverage of the same failure at multiple levels (one test at the right level) · defensive tests for your own code (DbC violation — assert at boundaries only) · scratch tests committed to the regression suite (keep them gitignored) · assertion-free "tests" providing no governance (add assertions, or call it a walkthrough) · over-mocking (prefer real dependencies where feasible).

## When a test blocks a refactor

Validate the new design is actually better, then check why the test exists: external contract → adapt the refactor or version/deprecate the old contract; implementation detail → delete the test and proceed; internal boundary → update both sides together, no backwards-compat shim.

If adding one feature forces many test edits, the tests are coupled to implementation — rewrite them against the interface, consolidate redundant ones, or accept it as an intentional breaking change.

## Before you commit or delete

Commit a regression test only if: the freeze decision is justified, it's not redundant with a higher-level test, it asserts on the interface not the implementation, and it has a clear failure message.

Delete a test only if: coverage exists elsewhere, it's not guarding an external contract, and the deletion rationale is documented.
