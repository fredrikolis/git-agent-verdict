<!-- Concern: states the universal code-design principles every language-specific standard specializes | Non-concern: per-language idiom and syntax | IO: none -->
# Language-Agnostic Programming Standards

Universal principles. All languages. All paradigms.

---

## AUTO-REJECT (Stop Work Immediately)

**Universal Blockers** (-∞):

- **Circular imports**: Module A imports B, B imports A → Restructure
- **Failing tests**: All tests pass before commit
- **Hardcoded secrets**: API keys, passwords, tokens in code → Environment variables
- **Force push to main/master**: Never on protected branches

**Language-Specific**: declare your repo's own per-language standard as another `--doc`.

---

## PART 1: Decision-Making Philosophy

### Evidence-Based Decisions

**Measure → Decide. Not: Opinion → Decide.**

| Cargo Cult                   | Evidence Required                             |
| ---------------------------- | --------------------------------------------- |
| "Framework X is best"        | Benchmark for THIS use case                   |
| "Microservices scale better" | Measured current bottleneck? Need that scale? |
| "NoSQL is faster"            | Profiled for THIS query pattern?              |

**Default**: Boring technology. Optimize when proven necessary.

---

### KISS (Keep It Simple)

**Simplest working solution wins.**

**Decision filter**:

```
Problem → Simplest approach works?
  ├─ Yes → Ship it
  └─ No → Justify (benchmark/requirement/evidence)
```

| Justified Complexity                        | Unjustified Complexity          |
| ------------------------------------------- | ------------------------------- |
| Benchmark proves simple approach inadequate | Premature optimization          |
| Current solution demonstrably fails         | Framework for single use case   |
| Explicit requirement demands it             | "Future-proofing" hypotheticals |

**Three duplicate lines > premature abstraction.**

---

### YAGNI (You Aren't Gonna Need It)

**Build for today. Not tomorrow.**

| Situation            | Ship This          | Not This                        |
| -------------------- | ------------------ | ------------------------------- |
| Single output format | That format        | Pluggable format system         |
| One user type        | One implementation | Role-based permission framework |
| Local deployment     | Local setup        | Cloud-agnostic abstraction      |
| Fixed config         | Hardcoded values   | Dynamic config system           |

**Exception**: Extensibility explicitly required → Design for extension, implement one.

---

## PART 2: Architecture & System Design

### Separation of Concerns (SoC)

**Every unit does its job and stays out of every other unit's job.**

SoC is not a layering rule — it is the *ownership* rule, and it recurses at every scale. One question, asked of a package, a file, a class, a function, a variable:

**What is this thing's ONE job, and what is explicitly NOT its job?**

| Scale | Does its job | Stays out of others' jobs |
|-------|--------------|---------------------------|
| Package / crate | Owns one capability | No reaching into another package's internals |
| File / module | Owns one concern | No neighbor's work (see File Annotations `NOT concerned with`) |
| Class / type | One reason to change (→ SRP) | No knowledge of another type's internals |
| Function | One thing, one level of abstraction | No reaching across a call boundary to fix a caller's mistake |
| Variable / name | Holds one meaning | Not recycled for a second purpose (no reused `tmp`, no overloaded flag) |

**Layering is SoC applied to runtime dependencies** — one special case, not the whole principle:

```
Presentation (API/UI)  →  Business Logic  →  Data Access
```

Each layer depends only on the layer below. Never above.

**SoC governs several principles here.** A violation surfaces downstream as defensive code (DbC — doing a job you don't own), a leaking API (Minimal API — blast radius crossing a boundary), or a multi-concern monolith (File Size — split by concern *first*). Fix it at the source: give each unit one job.

#### Refactoring lens: keep vs move vs delete

Every refactor is an ownership audit — the *what* to do; Remove-then-Replace covers the *how* of a rewrite. Ask in order:

1. **Intended** — what was this supposed to own? (its name, its reason to exist — the contract)
2. **Actual** — what does it handle now? (the drift from #1)
3. **Live** — does anyone still care about that concern?

The verdict falls out — argue the concern, never the code:

| Intended vs Actual | Concern wanted? | Verdict |
|--------------------|-----------------|---------|
| Doing exactly its job | Yes | **Keep** |
| Its job **+ extra** | Yes | **Split** — extract the extra to its rightful owner |
| A **different** job than its name claims | Yes | **Move / rename** to where that concern lives |
| Its job fine, concern is **dead** | No | **Delete** |
| Its job, concern **already owned elsewhere** | Owned better elsewhere | **Delete, consolidate** (→ DRY) |

**"Should we just delete it?" is the most under-asked refactor question.** A unit can do its job perfectly and still deserve deletion — because no one owns, wants, or needs its concern anymore. Doing a dead job well is still waste.

---

### Dependency Inversion

**Depend on abstractions, not concretions. Point dependencies at stability.**

Ownership includes owning your dependency *direction*. High-level policy must not depend on low-level detail; both depend on an abstraction (interface, trait, protocol) — the volatile concrete depends on the stable abstract, never the reverse. This is SoC applied to the direction of coupling: the *what* (contract) and *how* (implementation) change independently, and the unit becomes testable, its collaborators swappable.

| Pattern | Score | Notes |
|---------|-------|-------|
| Policy depends on an abstraction; detail implements it | +10 | Stable core, swappable edges |
| Concrete injected behind an interface/trait | +9 | Testable, decoupled |
| High-level module imports a concrete low-level one | -8 | Volatile detail drags policy with it |
| Abstraction that leaks its single implementation | -6 | Not an abstraction — a rename |

*SOLID mapped to first principles: SRP → SoC (class scale) · LSP → DbC · ISP → Minimal API · OCP → Dependency Inversion + Composition · DIP → Dependency Inversion (here).*

---

### Minimal API Surface

**Expose minimum necessary interface.**

- Internal details: Private
- Public API: Minimal, stable
- Implementation: Changeable without breaking clients

**Result**: Smaller blast radius. Easier to understand. Harder to misuse.

**From the consumer's side** this is interface segregation: a client depends only on the slice it uses, never on capability it doesn't touch.

---

### File Size — Agent-Manageable Modules

**Keep files at a size an agent can hold and edit confidently — split ONLY at a natural seam.**

A large file taxes every agent operation — re-reads, ambiguous exact-match edits, hidden diffs, serialized parallel work.

Split when a file outgrows the budget **and** has a seam the code already has (phases, construct-families, strands). A behavior-preserving split is gated like any change — a contract/test proving equivalent behavior before/after.

**Heuristic (not a hard line)**: ~1.5–2k lines AND a clean seam → split; else leave it.

| Pattern | Score | Notes |
|---------|-------|-------|
| Cohesive module split at a natural seam when it outgrows the budget | +9 | Bounded, reviewable, parallelizable |
| Behavior-preserving split, gated by a contract/test | +9 | Equivalence proven before/after |
| Cohesive file left intact at size (no natural seam) | +5 | Correct — don't split for its own sake |
| Forced split fragmenting one concern across files | -9 | Worse than the monolith — scatters cohesion |
| Unbounded growth of a hot file (no size discipline) | -7 | Compounding tax; serializes parallel work |

A multi-concern monolith is a Separation of Concerns problem, not a file-size one — split by concern first (see SoC), then apply size discipline within each.

---

### Refactoring: Remove-then-Replace

**Delete old → Build new. Boundary tests are the spec.**

```
Phase 1: REMOVE              Phase 2: BUILD
──────────────               ─────────────
Delete old implementation    Implement new
Delete internal tests        Build to pass boundary tests
Keep boundary tests
```

| Delete | Keep |
|--------|------|
| Old implementation | Boundary/contract tests |
| Unit tests of internals | Integration tests at edges |
| Tests coupled to old structure | Tests that define WHAT, not HOW |

**Why delete internal tests**: They constrain new implementation to match old structure.

**Scoring**:

| Pattern | Score | Notes |
|---------|-------|-------|
| Remove old, keep boundary tests | +10 | Clean slate, tests = spec |
| Delete internal tests during rewrite | +9 | No structural constraint |
| Preserving old code "for reference" | -8 | Shapes new implementation |
| Keeping internal tests during rewrite | -6 | Constrains to old structure |

---

## PART 3: Code Design & Implementation

### Design by Contract (DbC)

**Own both sides → know contract → fail fast. No defensive code for own types.**

**Defensive code ONLY for**: External APIs | User input | Library boundaries | Migration compat*

**NOT for**: Own modules | Own data structures | Internal packages | Code you control

*\*Migration compat: ONLY when production consumers exist outside your control. In monorepos where you control both sides of a contract, backwards-compat shims are a MAJOR VIOLATION - update all call sites instead.*

**Red flags** (defensive ignorance):

| Pattern                                  | Problem                        | Fix                                 |
| ---------------------------------------- | ------------------------------ | ----------------------------------- |
| `x = a or b or c`                        | Which is it? You control it.   | Trace producer, pick ONE            |
| `value?.deeply?.nested \|\| default`     | Contract uncertainty           | Document structure, access directly |
| `if (response.success \|\| response.ok)` | What does YOUR API return?     | Pick one, document                  |
| `isinstance(x, (A, B, C))`               | Multiple types from YOUR code? | Unify contract, single type         |

```python
# GOOD: DbC
def handle_event(event: Event):
    return event.data['value']  # Precondition: event.data has 'value'. Fail fast if violated.
```

**Subtypes too**: a subtype must honor its base type's contract (Liskov substitution) — one that quietly does a different job is a broken contract, not a variant.

**DbC = DRY**: Validate once at boundary, trust internally.

---

### Canonical Representation at Boundaries

**One canonical internal form. Convert only at the edges.**

Pick a single representation for each quantity in the core (timestamps → UTC epoch; money → integer minor units; text → normalized form). Store, compare, and compute in that form exclusively; convert to/from local or display forms only at I/O boundaries. Never let two representations coexist in the interior.

| Pattern | Score | Notes |
|---------|-------|-------|
| One canonical form, converted at the edge | +10 | No ambiguity internally |
| Convert to local/display at I/O boundary only | +9 | Surface concern, not core |
| Two representations mixed in the core | -10 | Which is authoritative? |
| Boundary value stored without normalizing | -8 | Drift, comparison bugs |

Language-specific canonical forms live in a per-language standard of your own (timestamps, for instance).

---

### Fail Fast

**Detect errors at source. Not downstream.**

```python
def process_user(user):
    if 'name' not in user or 'email' not in user:
        raise ValueError(f"Invalid user: missing required fields. Got: {user.keys()}")
```

**Explicit error > Silent fallback > Runtime confusion**

---

### Async/Await as Universal I/O Contract

**All I/O = async. Threading/polling = implementation details hidden behind async interfaces.**

Every I/O boundary (network, disk, IPC, FFI) is "wait for external." Async makes this explicit, composable, debuggable.

```
User Click → await API → await Service → await DB → await Native
    ↓            ↓            ↓             ↓            ↓
 (pending)    (HTTP)      (internal)    (network)   (executor)
```

**One mental model. Entire distributed stack. API call = coroutine.**

| Benefit | Mechanism |
|---------|-----------|
| Debuggable | Stack trace = logical flow, not thread jumps |
| Predictable | Explicit `await` = explicit suspension, no hidden races |
| Composable | Sequential: `await a(); await b();` Parallel: `await all([a(), b()])` |
| Efficient | Thousands concurrent I/O, minimal threads |
| Universal | Same model: JS, Python, Rust, C#, Go, Swift |

#### Scoring

| Pattern | Score | Notes |
|---------|-------|-------|
| Async for all I/O | +10 | Network, disk, IPC, external |
| Explicit parallel (gather/all) | +9 | Clear concurrency intent |
| Blocking wrapped at boundary | +8 | Caller sees async only |
| Blocking I/O in async context | -10 | Defeats purpose |
| New blocking API | -9 | Should be async |
| Threading for I/O (not CPU) | -8 | Use async |
| Polling when push available | -7 | Wasteful, latency |

#### "Contain the Ugly"

**Blocking unavoidable?** (CPU-bound, legacy, drivers) Wrap immediately behind an async interface, isolate in a dedicated module, document why no async alternative, migrate when one exists. Blocking never leaks — callers see only async.

| Acceptable Exception | Justification |
|------|---------------|
| Startup config reads | One-time, before event loop |
| CPU-bound work | Wrap in executor/worker |
| Legacy libs (no async API) | Wrap at boundary |
| Hardware/drivers | Wrap at boundary |

#### Anti-Patterns

| Anti-Pattern | Problem | Fix |
|--------------|---------|-----|
| Blocking HTTP in async | Blocks event loop | Async client |
| Thread-per-request | O(N) threads | Async handlers |
| Polling for events | CPU waste, latency | Callbacks/push |
| Mixed sync/async same layer | Contract confusion | Pick one, wrap |
| Locks in async | Usually unnecessary | Rethink flow |

**Boundary = await point. Thread pool = implementation detail.**

---

### DRY (Don't Repeat Yourself)

**Single source of truth for knowledge/logic.**

**Eliminate duplication when**:

- Same knowledge (repeated business logic)
- Same behavior (identical algorithm, multiple locations)
- Single source (one change affects all)

**Duplication OK when**:

- Accidental similarity (looks similar, different concepts)
- Decoupling needed (independent modules)
- Premature abstraction (too early to know)

**Rule of Three**: Duplicate once (2 instances), refactor at third.

**Wrong abstraction > duplication.**

---

### Documentation: Self-Documenting Code

**Comments aren't free. Default deny — every comment is guilty until it proves it carries the non-obvious.**

Code shows WHAT; names and types show intent; convention fills the rest. A comment earns its space only by supplying what none of those can — the non-obvious. Everything else is pure cost: space, split attention, and a stale lie waiting for the next edit.

**"Non-obvious" ≠ "absent from this file."** A competent reader — human or agent — infers convention and common idiom. You don't explain that a decorator registers a handler, that a context manager releases its resource, or that a retry loop retries — every agent knows it by inference. The test is not *is it in the code* but *can a competent reader derive it*. If inference reaches it, the comment is noise.

**Docstrings — same bar.** A docstring restating the signature is a DRY violation. Write one ONLY when an external consumer parses it (API doc generators, decorators, frameworks), it's a public library interface, or a genuinely complex algorithm needs domain/math explanation. Never for internal functions, obvious implementations, or simple logic — refactor the unclear code instead.

**Rot compounds — score it as a structural defect.** One stale line taxes every future read, and one exposed lie makes the reader distrust every other comment in the file.

| Pattern | Score | Notes |
|---------|-------|-------|
| Carries the non-obvious WHY / an invariant / a safety rationale | +9 | Earns its keep — the reader can't derive it |
| Stale comment: describes code as it WAS, or lies about what it does now | -10 | Actively misleads; worse than no comment |
| History narration: "was X, now Y" / "previously" / a migration story | -8 | Git owns history; rots on the next edit |
| Docstring restating the name, signature, or types | -8 | DRY violation; changes twice on every edit |
| Restates WHAT the code, names, or convention already shows | -6 | Noise; delete, or fix the name |

**Anti-patterns**:

```python
# BAD: Docstring repeats obvious information
def calculate_total(items: list[Item]) -> float:
    """Calculate total price of items.

    Args:
        items: List of items to calculate total for

    Returns:
        Total price as float
    """
    return sum(item.price for item in items)

# GOOD: Self-documenting
def calculate_total(items: list[Item]) -> float:
    return sum(item.price for item in items)
```

**Exceptions requiring docstrings**:

```python
# GOOD: FastAPI uses docstrings for OpenAPI docs (external consumer)
@app.post("/users")
async def create_user(user: UserCreate) -> User:
    """Create new user account with email verification."""
    ...

# GOOD: Public library, complex algorithm
def optimized_levenshtein_distance(s1: str, s2: str) -> int:
    """Compute edit distance using Wagner-Fischer O(mn) algorithm.

    Uses space optimization: O(min(m,n)) instead of O(mn).
    See: Wagner & Fischer (1974) for proof of correctness.
    """
    ...
```

**Decision flow**:

```
Tempted to add a comment?
  ├─ Reader derives it from code / names / convention? → Delete, or fix the name
  ├─ Non-obvious WHY / invariant / safety? → Keep — the one justified case
  ├─ Framework/decorator parses the docstring? → Write docstring
  └─ Public library API or complex algorithm? → Write docstring
```

---

### Composition Over Inheritance

**Favor composition. Not inheritance.**

- **Inheritance**: True "is-a" (substitutability)
- **Composition**: "has-a" + behavior reuse

---

### File-Level Annotations (Codebase Discoverability)

**Every file's first line describes its responsibility.**

**The Goal**: Running Bash(annotated-tree) should describe the entire codebase's functionality. Between file names, folder structure, and first-line annotations, the app's purpose and organization should be clear _without reading any code_.

**Format** (first non-shebang, non-empty line):

```
# [Role]: [What it does]. [Responsible for X]. NOT concerned with [Y]. | I/O: (inputs) → outputs
```

Or as a docstring:

```python
"""[Role]: [What it does]. [Responsible for X]. NOT concerned with [Y]."""
```

**Why it matters**:

- **Quick orientation**: Understand codebase without reading implementation
- **Clear boundaries**: "NOT concerned with" prevents scope confusion
- **I/O contracts**: See how pieces connect without tracing imports
- **Tool support**: Bash(annotated-tree) extracts these for instant codebase snapshot

---

## Summary

| Principle                     | Essence                             | Violation Signal                          |
| ----------------------------- | ----------------------------------- | ----------------------------------------- |
| **Evidence-Based**            | Measure → decide                    | "Best practice" without context           |
| **KISS**                      | Simplest working solution           | Complexity without justification          |
| **YAGNI**                     | Build for today                     | Features for hypothetical future          |
| **SoC**                       | Every unit owns one job; fractal pkg→variable | Unit doing a neighbor's job; "just use layers" |
| **Dependency Inversion**      | Depend on abstractions; point at stability | High-level module depends on low-level detail |
| **Minimal API**               | Expose only necessary               | Leaking implementation details            |
| **Remove-then-Replace**       | Delete old, boundary tests = spec   | Keeping internal tests during rewrite     |
| **File Size**                 | Agent-manageable; split at natural seams | Multi-thousand-line multi-concern monolith |
| **DbC**                       | Own interface → know contract       | Defensive code for own types              |
| **Canonical Representation**  | One internal form, convert at edges | Two representations mixed in the core     |
| **Fail Fast**                 | Errors at source                    | Silent fallbacks masking problems         |
| **Async I/O Contract**        | All I/O async; blocking wrapped at edge | Blocking I/O in async context         |
| **DRY**                       | Single source of truth              | Duplicated business logic                 |
| **Documentation**             | Comments aren't free — default deny, non-obvious only | Comment restating code/convention; stale or history narration; docstring on internal fn |
| **Composition > Inheritance** | Flexible composition                | Deep inheritance hierarchies              |
| **File Annotations**          | First line describes responsibility | Files without purpose description         |

---

## References

- [Design by Contract vs Defensive Programming](https://softwareengineering.stackexchange.com/questions/125399/differences-between-design-by-contract-and-defensive-programming)
- [SOLID Principles](https://www.ultracodes.io/blog/principles-of-software-development)
- [Composition Over Inheritance](https://en.wikipedia.org/wiki/Composition_over_inheritance)
