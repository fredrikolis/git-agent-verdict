<!-- Concern: decides which assertions earn a committed test | Non-concern: test tooling and framework choice | IO: none -->
# Universal Testing Principles

Testing as governance. Every assertion is a freeze decision. Freeze deliberately.

---

## CRITICAL: Quick Decision Rule

**BEFORE WRITING ANY ASSERTION:**

| Question | If Yes | If No |
|----------|--------|-------|
| External contract (can't coordinate consumers)? | **FREEZE** (commit test) | Continue |
| You control both sides of this interface? | **DON'T FREEZE** (DbC) | Continue |
| Already caught by e2e/integration test? | **DON'T FREEZE** (redundant) | Consider freezing |
| Test would block principled refactor? | **DELETE TEST** (-∞) | Proceed |

**Positive score = commit. Negative = skip or delete.**

---

## Scoring: When to Commit a Test

| Criteria | Score | Rationale |
|----------|-------|-----------|
| External contract (can't coordinate consumers) | **+10** | Must freeze |
| High downstream dependency | +8 | Many dependents at risk |
| Leaf node stable abstraction | +9 | Interface-level freeze |
| Edge case not caught by e2e | +8 | Unique coverage value |
| You control both sides | **-10** | DbC violation |
| Already caught by e2e | -8 | Redundant freeze |
| Glue/orchestration code | -9 | Test via e2e instead |
| Implementation detail (not interface) | -7 | Freezes "how" not "what" |
| Single internal caller | -5 | Low blast radius |
| Test blocks principled refactor | **-∞** | Delete test, fix architecture |

**Threshold**: Positive = commit. Negative = skip or delete.

---

## Core Principle: Every Assertion is a Freeze Decision

**When you write `assert X == True`, you signal**: "I want this behavior FROZEN. Future agents MUST update this test to change this behavior."

Tests create "refactor penalty" — intentional governance. Must be deliberate.

### Before Asserting: Three Questions

| Question | Purpose | Red Flag |
|----------|---------|----------|
| **1. Already captured higher-level?** | Redundancy check | Same failure mode caught by e2e |
| **2. What's actual harm if False?** | Harm analysis | "Different" vs "broken" |
| **3. Control both sides?** | DbC check | Asserting on internal interface |

**Failure to ask → test pollution → refactor gridlock.**

---

## Agent-Specific Testing Challenges

### The DRY-Like Problem

Adding test means maintaining **THREE places**:
1. Code (implementation)
2. Call sites (usage)
3. Test (verification)

**For agents**: Tests and code not in same context window → must connect dots across codebase → hidden dependencies.

### Perverse Behavior to Avoid

| Anti-Pattern | Symptom | Remedy |
|--------------|---------|--------|
| Suboptimal refactor to pass tests | Awkward design because easiest path to green | Test is wrong — delete/rewrite |
| Tests driving architecture | Design compromised for test suite | Principles drive architecture |
| Over-accommodation | Complexity added to preserve test expectations | Tests serve code, not vice versa |

**RULE**: If test blocks principled refactor → delete or rewrite test. Never compromise architecture.

---

## Freeze Criteria

### Freeze (commit regression test) when:

| Criteria | Example | Rationale |
|----------|---------|-----------|
| External consumers you can't coordinate | Public API, library interface | Breaking change = downstream failures |
| High downstream dependency | Critical system contracts | Bug propagates widely |
| Unlikely to change AND important it doesn't | Data format, protocol | Stability requirement explicit |

### Don't freeze when:

| Criteria | Example | Rationale |
|----------|---------|-----------|
| You control all callers | Internal service boundaries | Update together (DbC) |
| Internal seam, own both sides | Frontend/backend same repo | Coordinate changes |
| Glue/orchestration code | Coordination logic | Test via e2e |
| Implementation detail | Private methods, internal state | Freezes "how" not "what" |

**DbC connection**: Frontend/backend boundary in same app = INTERNAL seam, not external API. Don't freeze like public contract.

---

## Not Traditional TDD

**This is NOT traditional TDD** (red-green-refactor with all tests committed).

| Traditional TDD | Our Approach |
|-----------------|--------------|
| Write failing test first | Write code, then scratch test to prove it works |
| All tests committed | Most tests kept in artifacts/ (gitignored, not committed) |
| Tests = specification | Scratch tests = proof; regression tests = governance |
| Pass = success | Pass ≠ success |

**Development workflow:**
```
Code → Scratch test (prove it works) → KEEP in artifacts/ (gitignored)
                                              ↓
                            Only commit regression test if external contract deserves freezing
```


---

## Test Types & Purposes

| Type | Purpose | Assertions | Lifecycle | Audience |
|------|---------|------------|-----------|----------|
| **Scratch** | Self-convincing ("does this work?") | Yes, evidence persists | Gitignored (kept in artifacts/) | Developer/agent writing code |
| **Walkthrough/Tour** | Demonstrate usage, educate | None (LLM analyzes output) | Permanent | Future agents |
| **Regression** | Freeze external contracts | Yes (boundaries only) | Permanent | CI/CD gate |

**Where knowledge lives**:
- Scratch → evidence persists in artifacts/ for review → not committed to git
- Walkthrough → executable documentation → no assertions
- Regression → "this must not change" contract → maintained forever


---

## Architectural Position Matters

| Code Type | Testing Strategy | Test Focus |
|-----------|------------------|------------|
| **Leaf nodes** (stable abstractions) | Freeze interface | "Would test make sense if class replaced?" |
| **Glue code** (orchestration) | Don't freeze HOW | E2e verifies integration |
| **External APIs** (can't coordinate) | Freeze contract | Shape, format, semantics |
| **Internal APIs** (control both sides) | Don't freeze | Update all call sites together |

---

## Two Failure Modes Tests Protect Against

| Mode | Example | Assert On | Response to Failure |
|------|---------|-----------|---------------------|
| **External change** | Library update changes format | Shape/contract | Informational — investigate |
| **Internal accident** | Developer mistakenly changes behavior | Specific behavior | Preventive — block change |

---

## Testing as Trade-off

| Too Few Tests | Too Many / Wrong Tests |
|---------------|------------------------|
| Quality risk, bugs slip through | Refactors blocked |
| Low confidence | Feature dev harder |
| Regressions undetected | Tests fight maintenance |

**Tests should assist development, not fight it.**

---

## Test Levels

| Level | Purpose | Freeze Target |
|-------|---------|---------------|
| **E2E** | User scenarios work | Full application behavior |
| **Integration** | Components work together | Boundary contracts |
| **Unit** | Single component correct | Leaf abstractions (interface only) |
| **Scratch** | Prove to self | Nothing (ephemeral) |

**Distribution**: Few e2e (critical paths), moderate integration (key boundaries), minimal unit (stable interfaces), many scratch (during dev).

---

## When You DO Freeze: Test Representativeness

Tests approximate production. Every divergence is a blind spot.

| Divergence Type        | What It May Hide                          |
|------------------------|-------------------------------------------|
| Mocked dependencies    | Integration failures, API contract drift  |
| Simplified/fake data   | Edge cases, encoding issues, scale        |
| Different topology     | Connection lifecycle, auth flow direction |
| Single-threaded        | Race conditions, deadlocks                |
| Local execution        | Network latency, timeouts, DNS            |
| Clean state each test  | State accumulation bugs                   |
| Deterministic ordering | Order-dependent failures                  |

**Not a rule—a lens.** Before taking a shortcut, ask: "What might this test NOT catch?"

Document known blind spots. You can't eliminate divergence; you can be aware of it.

---

## Common Anti-Patterns

| Anti-Pattern | Problem | Fix |
|--------------|---------|-----|
| Testing implementation details | Freezes "how" not "what" | Test interface |
| Redundant coverage | Same failure, multiple tests | One test at right level |
| Defensive tests for own code | DbC violation | Assert at boundaries only |
| Scratch tests committed | Pollutes regression suite | Keep gitignored |
| Tests without assertions | No governance value | Add assertions or make walkthrough |
| Over-mocking | See Test Representativeness | Use real dependencies when feasible |

---

## When Tests Fight Development

### Test blocks principled refactor

1. **Validate**: Is new design actually better?
2. **Check**: Why did test exist? External contract or implementation detail?
3. **Decision**:
   - External contract → adapt refactor OR version/deprecate
   - Implementation detail → **delete test**, proceed
   - Internal boundary → update both sides, no backwards compat

**NEVER compromise architecture for test suite.**

### Adding feature requires changing many tests

**Diagnosis**: Tests coupled to implementation.

**Fix**: Rewrite to test interface, consolidate redundant tests, or accept intentional breaking change.

---

## Evidence Requirements

**Before committing regression test**:
- [ ] Freeze decision justified (external contract OR high harm)
- [ ] Not redundant with higher-level test
- [ ] Tests interface, not implementation
- [ ] Clear failure message

**Before deleting test**:
- [ ] Coverage exists elsewhere
- [ ] Not guarding external contract
- [ ] Deletion rationale documented

---

## Summary: Core Rules

**MEMORIZE THESE:**

1. **Every `assert` is a FREEZE DECISION** — be deliberate
2. **External contract?** → FREEZE (commit test)
3. **Control both sides?** → DON'T FREEZE (DbC violation)
4. **Test blocks principled refactor?** → DELETE THE TEST
5. **One test at right level** > multiple tests at wrong levels
6. **Tests approximate production** — every divergence is a blind spot (document what you're NOT testing)

**Test lifecycle determines commitment:**
```
Scratch    → prove correctness → KEEP in artifacts/ (gitignored, evidence for review)
Regression → freeze contracts  → COMMIT (maintain forever)
Walkthrough → demonstrate usage → COMMIT (no assertions)
```

---

## References

- [Design by Contract](https://softwareengineering.stackexchange.com/questions/125399/differences-between-design-by-contract-and-defensive-programming)
- [Spike Solutions (Agile/XP)](https://www.jamesshore.com/v2/books/aoad1/spike_solutions)
- [The Practical Test Pyramid](https://martinfowler.com/articles/practical-test-pyramid.html)
- See: the `programming` standard shipped alongside this one (Design by Contract)
