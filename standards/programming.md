<!-- Concern: states the universal code-design principles every language-specific standard specializes | Non-concern: per-language idiom and syntax | IO: none -->
# Language-Agnostic Programming Standards

## AUTO-REJECT (-∞)

Circular imports · failing tests at commit time · hardcoded secrets (use env vars) · force push to main/master. Declare per-language standards as additional `--doc`s.

## Decision-Making

**Evidence-based**: measure, don't cargo-cult ("framework X is best", "NoSQL is faster") — benchmark or profile for *this* use case before choosing. Default to boring technology; optimize only once proven necessary.

**KISS**: ship the simplest approach that works; complexity needs a benchmark, a demonstrated failure, or an explicit requirement behind it — never "future-proofing." Three duplicated lines beat a premature abstraction.

**YAGNI**: build what today's single format/user-type/deployment/config needs, not a pluggable/generic framework for a hypothetical one. Exception: design for extension (but implement only one) when extensibility is an explicit requirement.

## Architecture

**Separation of Concerns**: one question at every scale (package, file, class, function, variable) — *what is this thing's one job, and what is explicitly not its job?* Layering (presentation → business logic → data access, depend only downward) is SoC applied to runtime dependencies, not the whole principle.

A SoC violation resurfaces downstream as defensive code (DbC), a leaking API (Minimal API), or a multi-concern monolith (File Size) — fix it at the source.

*Refactor lens* — for any unit, ask in order: what was it **intended** to own, what does it **actually** do now, and is that concern still **live**? Extra beyond its job → split the extra off. Different job than its name claims → move/rename. Job fine but concern dead, or owned elsewhere → delete — a unit can do its job perfectly and still deserve deletion once nobody needs that job.

**Dependency Inversion**: high-level policy depends on an abstraction (interface/trait/protocol); the volatile concrete implementation depends on that abstraction too, never the reverse — SoC applied to coupling direction. An abstraction with only one implementation that leaks through it isn't an abstraction, it's a rename. (SOLID mapped to first principles: SRP→SoC, LSP→DbC, ISP→Minimal API, OCP→DIP+Composition, DIP→this.)

**Minimal API surface**: expose only what consumers need; keep everything else private and freely changeable. Smaller blast radius, harder to misuse — from the consumer's side, this is interface segregation (depend only on the slice you use).

**File size**: keep files at a size an agent can hold and edit confidently — past a point, re-reads, exact-match edits, and diffs all get harder. Split only when a file outgrows its budget (~1.5-2k lines, a heuristic not a hard line) *and* has a natural seam (phases, construct-families, strands) already in the code, gated by a behavior-preserving contract/test proving equivalence.

A multi-concern monolith is a SoC problem: split by concern first, then apply size discipline within each piece. Forcing a split with no seam fragments one concern across files and is worse than leaving it whole.

**Remove-then-Replace** (rewrites): delete the old implementation and its internal/structural tests, keep only boundary/contract tests (they define *what*, not *how*) as the new implementation's spec. Keeping old code "for reference," or keeping internal tests during the rewrite, constrains the new implementation to the old structure.

## Code Design

**Design by Contract**: for code you own both sides of, know the contract and fail fast — no defensive code. Defensive code is only for external APIs, user input, library boundaries, or migration compatibility with production consumers genuinely outside your control (a shim between two sides you both control, e.g. inside one monorepo, is a violation — update the call sites instead).

Red flag: `a or b or c`, `x?.y?.z || default`, `isinstance(x, (A,B,C))` on your own types — you control the producer, trace it and pick one shape. Subtypes must honor the base type's contract (Liskov) — a subtype doing a materially different job is a broken contract. DbC is DRY for validation: validate once at the boundary, trust internally.

**Canonical representation**: one internal form per quantity (UTC epoch for time, integer minor units for money, normalized text) used throughout the core; convert to/from local or display forms only at I/O boundaries. Two representations coexisting internally is an authority ambiguity; a boundary value stored without normalizing drifts and breaks comparisons.

**Fail fast**: validate and raise at the point an invariant is violated, not downstream where the failure surfaces as confusion far from its cause. Explicit error beats a silent fallback beats runtime confusion.

**Async/await as the universal I/O contract**: every I/O boundary (network, disk, IPC, FFI) is implicitly "wait for external" — make it explicit with `await` rather than hiding it behind threads/polling/callbacks. One mental model across the whole stack; stack traces read as logical flow; sequential vs. parallel is explicit (`await a(); await b()` vs `await all([...])`).

Unavoidable blocking (CPU-bound work, legacy libs with no async API, hardware/drivers, one-time startup reads) must be wrapped immediately behind an async interface and isolated — it must never leak to callers. Blocking I/O inside an async context, thread-per-request, polling when a push channel exists, and mixed sync/async at the same layer are all violations.

**DRY**: one source of truth for a piece of business knowledge or behavior — eliminate duplication once it's the *same* knowledge repeated, not merely similar-looking code. Duplication is fine when it's accidental similarity between different concepts, needed for decoupling, or it's too early to know the right abstraction (rule of three: duplicate once, refactor at the third occurrence). A wrong abstraction is worse than the duplication it replaced.

**Composition over inheritance**: reach for inheritance only for genuine "is-a" substitutability; default to "has-a" plus composed behavior otherwise.

**Documentation — comments aren't free, default deny**: a comment earns its place only by carrying what code, names, types, and ordinary convention can't — the non-obvious. The test isn't "is this explained in the file" but "can a competent reader (human or agent) *derive* it" — if inference reaches it, the comment is noise.

A docstring that restates the signature is a DRY violation (two places to update); write one only when an external consumer parses it (OpenAPI, framework decorators), it's a public library interface, or a genuinely complex algorithm needs domain/math justification.

A stale comment describing old behavior is worse than none — it's a lie the reader can't detect, and it erodes trust in every other comment in the file. "Was X, now Y" history narration belongs in git, not a comment — it rots on the next edit.

**File-level annotations**: every file's first non-shebang/non-empty line names its responsibility: `# [Role]: [what it does]. NOT concerned with [Y]. | I/O: (inputs) → outputs`. The goal: file names + folder structure + these first lines describe the whole codebase's purpose without reading implementation, extractable project-wide by the `annotated-tree` tool.
