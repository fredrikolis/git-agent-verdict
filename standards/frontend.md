<!-- Concern: governs frontend app architecture, framework-neutral | Non-concern: picking the frontend stack or per-framework API idiom | IO: none -->

# Frontend Project Standards

**COMMUNICATION STYLE**: Expert-to-expert. Dense, scannable, no prose. Omit obvious.

---

## Three Pillars

Three architectural principles govern every rule in this document. Parts 1–9 are organized by feature for day-to-day lookup; this section teaches the **why** behind them.

### UDF — Unidirectional Data Flow

**State mutations flow through defined, traceable paths — never multi-origin.**

Props down, events up. Mutations happen at the owner, never at a distance.

Common UDF breaks regardless of framework:
- Mutating an object or array received as a prop: reference semantics let a child silently change parent state
- Handing a subtree a bare mutable shared object: any descendant can mutate ancestor state with no traceable path
- Reaching for a parent or root component instance directly: anti-pattern

A shared store provides UDF *tools* (defined mutation functions, subscription hooks) without *mandating* their use — be explicit about which patterns you rely on.

### LoD — Law of Demeter

**Components talk only to immediate neighbors via props/events.**

"Only talk to your friends": direct parent (via emitted events), direct children (via props), own shared-logic modules, explicitly typed injected dependencies from declared providers.

Compliant mechanisms:
- Props/events (maximum compliance)
- Content slots (consumer provides content, component provides layout — no coupling)
- Explicitly declared public component surface (still prefer props/events)
- Scoped dependency provision with typed keys and read-only exposure

Violations:
- Prop drilling >2 levels (already auto-reject)
- Direct parent/root instance access; holding handles to child component instances to call their methods
- Ambient dependency provision without scoping (entire subtree gets implicit dependency)
- Shared-logic module leaking internal state (returning mutable internals)
- Store train-wreck chains (`store.user.settings.theme.color`)

### DIP — Dependency Inversion Principle

**Lower tiers never import higher tiers; depend on abstractions.**

Without a DI container, DIP requires intentional architecture: content slots, typed injection keys, module return-type contracts, props/events interfaces, logic-only (renderless) components.

Import direction (the DAG):

```
apps/ → features/ → components/ → ui/
  ↓         ↓            ↓          ↓
  └─────────┴────────────┴──→ shared/ → types/
```

Concrete allowed imports:

| From | May Import |
|------|-----------|
| `ui/` | `shared/`, `types/` |
| `components/` | `ui/`, `shared/`, `types/` |
| `features/` | `components/`, `ui/`, `shared/`, `types/` |
| `apps/` | `features/`, `components/`, `ui/`, `shared/`, `types/` |
| `shared/` | `types/` |
| `types/` | nothing |

Forbidden: any arrow pointing upward or sideways between features.

### Cross-Reference Matrix

| Section | UDF | LoD | DIP | Primary |
|---------|:---:|:---:|:---:|---------|
| Auto-Reject | x | x | x | ALL |
| PT 1: CDD | - | xx | x | **LoD** |
| PT 2: Lifecycle | - | - | - | orthogonal |
| PT 3: Async | x | - | - | UDF |
| PT 4: State | xx | x | x | **UDF** |
| PT 5: Props/Events | xx | xx | x | **UDF + LoD** |
| PT 6: Testing | - | x | x | LoD + DIP |
| PT 7: Routing | - | - | - | orthogonal |
| PT 8: Types | - | - | x | DIP |
| PT 9: Assets | - | - | - | orthogonal |
`xx` = primary driver. `x` = relevant. `-` = not applicable. Orthogonal sections (Lifecycle, Routing, Assets) enforce independent quality properties — orthogonal does not mean less important.

---

## AUTO-REJECT (Stop Work Immediately)

Universal auto-reject patterns are in the `programming` standard.

### Frontend Auto-Rejects

- **Fire-and-forget async** (-∞) `[UDF]`: All async operations must be awaited
- **Global state without justification** (-∞) `[UDF]`: State must be local unless proven necessary
- **Missing lifecycle cleanup** (-∞): Teardown required for subscriptions/timers/resources acquired at component setup
- **Component without file annotation** (-∞): First line must describe responsibility
- **Prop drilling >2 levels** (-∞) `[LoD]`: Use shared-logic modules or scoped dependency provision
- **Circular imports** (-∞) `[DIP]`: Module A imports B, B imports A → Restructure
- **Type-system escape hatch without comment** (-∞): An untyped/`any`-style value disables type checking; requires a justification comment

### UDF (Unidirectional Data Flow) Auto-Rejects

- **Shared mutable state export** (-∞): Mutable state exported from a non-store module and mutated by multiple consumers. Use a store, or a module returning read-only state plus mutation functions.
- **State mutation before server confirmation without rollback** (-∞): State set before a POST/PUT returns, with no catch + rollback. Optimistic update with snapshot + rollback remains +6.
- **Bidirectional sync without loop prevention** (-∞): Two observers where each writes to the other's source with no update-in-progress guard. Use a single derived value with an explicit setter instead.
- **Mutating injected shared state** (-∞): Direct property mutation on state received via subtree dependency provision. Provider must expose read-only state + mutation functions.

### LoD (Law of Demeter) Auto-Rejects

- **Calling child-component methods via an instance handle** (-∞): Holding a handle to a child component and invoking its methods. Use props-down/events-up. *Exception: handles to raw DOM elements (focus, play, canvas context) are not violations.*
- **Parent/root instance access** (-∞): Reaching for the parent or root component instance in any form. Use emitted events or typed dependency provision.
- **Untyped dependency-provision keys** (-∞): Providing/injecting subtree dependencies under bare string keys. Use typed, collision-proof keys.
- **Non-neighbor component access** (-∞): Reaching through the tree to siblings/cousins via chained instance handles or component-addressed event-bus messages. Lift shared state to the common ancestor or use a store.

### DIP (Dependency Inversion) Auto-Rejects

- **Upward tier import** (-∞): `ui/` importing from `components/`/`features/`/`apps/`, or `components/` importing from `features/`/`apps/`. Dependency flows downward only: `apps/ → features/ → components/ → ui/`.
- **Cross-feature direct import** (-∞): `features/A/` importing from `features/B/`. Use shared types in `/types/` or a store for cross-feature coordination.
- **Shared module with hidden global dependency** (-∞): Router, store, or browser-storage access inside `/shared/**` without parameter injection. *Exception: feature-internal modules in `/features/*/` are exempt unless re-exported from the feature barrel.*
- **Business logic in ui/ tier** (-∞): Any feature, service, or store import in `ui/` primitives.

### Anti-Pattern Catalog

Named patterns for code review. Severity matches the scoring scale: FATAL = -∞, SEVERE = -9 to -8, MODERATE = -7 to -6.

#### UDF Anti-Patterns

**AP-01: The Phantom Async** · FATAL · `[UDF]`
Async call without `await` — fire-and-forget. State may update after unmount, errors swallowed silently.

**AP-02: The Premature Optimist** · MODERATE · `[UDF]`
State assigned immediately before an awaited POST/PUT — sets state before server confirmation with no rollback.

**AP-03: The Derived-State Side Effect** · SEVERE · `[UDF]`
State mutation or API calls inside a derived-state computation — derivations must be pure, not mutation sites.

**AP-04: The Prop Mutator** · FATAL · `[UDF]`
Pushing into or reassigning a received prop — direct prop mutation at a distance from the owner.

**AP-05: The Broken Bridge** · FATAL · `[UDF]`
Internal state copied from a prop at initialization only, with no observer syncing later external prop changes — external updates silently ignored.

#### LoD Anti-Patterns

**AP-06: The Prop Telephone** · FATAL · `[LoD]`
Same prop name threaded through 4+ component files — prop drilling in disguise. Use a shared-logic module or scoped dependency provision.

**AP-07: The Untyped Event** · MODERATE · `[LoD]`
Component events declared without payload types — no compile-time checking of the component contract.

**AP-08: The Provision Junkyard** · MODERATE · `[LoD]`
>3 subtree dependency provisions from a single component — over-providing creates invisible dependencies for the entire subtree.

#### DIP Anti-Patterns

**AP-09: The Circular Ouroboros** · FATAL · `[DIP]`
Module A imports B, B imports A. Causes undefined values at import time, unpredictable initialization order.

**AP-10: The Escape Hatch** · FATAL
Type-system escape hatch without justification comment — disables type checking, hides contract violations. Every such escape must have a comment explaining why.

#### Lifecycle Anti-Patterns

**AP-11: The Zombie Subscription** · FATAL
Component setup acquires a resource (WebSocket, timer, listener) with no matching teardown on unmount. Memory leak, stale callbacks.

**AP-12: The Invisible Observer** · SEVERE
Observer with an async callback that lacks cancellation (abort/cleanup hook) — the previous async operation completes after the observer re-fires, stale data written.

#### Architectural Anti-Patterns

**AP-13: The Headless Chicken** · FATAL
Component file whose first line is not an annotation comment describing its responsibility — unknown responsibility.

**AP-14: The State Hoarder** · MODERATE
Store or shared-logic module with >10 independent pieces of state — god object accumulating unrelated state. Split by domain.

**AP-15: The God Component** · SEVERE
Component file >300 lines or >5 async functions — doing too much. Split into a logic module + presentation component.

**AP-16: The then-Chain Spaghetti** · MODERATE
`.then().then().catch()` chains instead of `async/await` — harder to read, harder to debug, error handling scattered.

**AP-17: The Stale Error Swallower** · SEVERE
Async function without `try/catch` — errors propagate as unhandled promise rejections, no user feedback, no recovery.

#### Testing Anti-Patterns

**AP-18: The Memory-Only State** · SEVERE · `[LoD]`
Component with >3 visual states but no URL query-parameter mapping — states only accessible via in-app navigation. Cannot reproduce via URL; agent testing requires navigation scripting; bug reports cannot encode state.

---

## PART 1: Component-Driven Development (CDD) `[LoD, DIP]`

### Core Principle

**Every component must be independently renderable and testable in isolation.**

Build UIs bottom-up: primitives → composites → features → apps. Each layer testable via a component workbench or URL query params.

### Isolated Component Principle

| Pattern | Score | Notes |
|---------|-------|-------|
| Component renderable in isolation | +10 | Via component workbench or query params |
| All states accessible via props | +9 | Loading, error, empty, success variants |
| No hard dependencies on parent context | +8 | Self-contained `[LoD]` |
| Workbench story exists per component | +7 | Living documentation |
| Component requires full app context | -10 | Not isolated |
| Prop drilling >2 levels | -∞ | Auto-reject `[LoD]` |
| Hard-coded parent assumptions | -8 | Breaks isolation `[LoD]` |

### Component Organization (4-Tier Hierarchy)

**Layers**: `ui/` → `components/` → `features/` → `apps/`

```
project/
├── ui/              # Design system primitives (Button, Input, Select)
├── components/      # App composites (UserCard, DataTable)
│   ├── display/     # Display-oriented (CopyButton, DataTable)
│   ├── forms/       # Form inputs & editors
│   ├── layout/      # Layout primitives (Dialog, Page, Section)
│   └── media/       # Media components (VideoPlayer, ImageGallery)
├── features/        # Domain modules (auth, dashboard, billing)
├── apps/            # App entry points (main, admin, onboarding)
├── shared/          # Reusable state-and-logic modules
├── types/           # Shared type definitions
└── assets/          # Styles, images, constants
```

| Pattern | Score | Notes |
|---------|-------|-------|
| UI primitives in `/ui/` | +10 | Headless, no business logic |
| App composites in `/components/` | +9 | Semantic subdirectories |
| Feature modules in `/features/` | +8 | Self-contained domains |
| App entry points in `/apps/` | +7 | Separate deployments |
| Reusable logic modules in `/shared/` | +9 | Cross-feature reuse |
| Business logic in `/ui/` | -∞ | Auto-reject `[DIP]` — tier violation |
| Feature components outside `/features/` | -7 | Poor encapsulation `[DIP]` |
| Shared components in app-specific dirs | -8 | Move to `/components/` |
| No home for shared logic modules | -8 | Where does reusable logic go? |

### Component Naming & File Structure

| Pattern | Score | Notes |
|---------|-------|-------|
| One consistent case convention for component files | +10 | e.g. `UserProfile`, project-wide |
| Descriptive, domain-specific names | +9 | `UserDashboard`, not `Main` |
| One component per file | +10 | No multi-component files |
| Barrel exports for `/ui/` | +8 | Single import point per tier |
| File annotation (first line) | +10 | Describes responsibility |
| Numeric suffixes (`Component2`) | -8 | Refactoring debt |
| Abbreviated names without context | -6 | `PLCTags` → `PLCTagsPanel` |
| Mixed case conventions across component files | -7 | Pick one, apply everywhere |

---

## PART 2: Lifecycle Resource Management `[orthogonal]`

### Resource Acquisition Is Initialization (Web Edition)

**Resource lifetime = component lifetime. Acquire at mount, release at unmount.**

Every subscription, timer, WebSocket, or event listener must have paired setup/teardown.

| Pattern | Score | Notes |
|---------|-------|-------|
| Mount/unmount hooks paired | +10 | Acquire-and-release guarantee |
| WebSocket/subscription cleanup | +10 | No memory leaks |
| Timer/interval cleanup | +9 | Clear on unmount |
| Event listener cleanup | +8 | Remove what you added |
| Observer registered with a cleanup callback | +9 | Auto-cleanup pattern |
| Resource leak (no cleanup) | -∞ | Auto-reject |
| Cleanup in wrong hook | -8 | Must run at unmount |
| Manual cleanup calls (not lifecycle hooks) | -7 | Use lifecycle hooks |

---

## PART 3: Async-First Architecture `[UDF]`

### No Fire-and-Forget

**Every async operation must be awaited. No exceptions.**

See the `programming` standard: Async/Await as Universal I/O Contract.

| Pattern | Score | Notes |
|---------|-------|-------|
| `await` on all async calls | +10 | Including POST/PUT/DELETE |
| `try/catch` for error handling | +9 | Explicit error paths |
| Parallel await for independent operations | +8 | e.g. `Promise.all` |
| Sequential when dependent | +7 | Correct order |
| Loading states during async | +8 | UX feedback |
| Fire-and-forget POST | -∞ | Auto-reject `[UDF]` |
| Missing error handling | -9 | Unhandled rejections |
| `void`-prefix / discard for intentional ignore | -8 | Just await it |
| `.then()` chains | -7 | Use `async/await` |

**Example:**

```typescript
// GOOD: Fully awaited with error handling
async function saveData() {
  loading = true
  error = undefined
  try {
    const result = await api.post('/data', payload)
    notify.success('Saved!')
    return result
  } catch (e) {
    error = e.message
    notify.error('Failed to save')
    throw e  // Re-throw for caller awareness
  } finally {
    loading = false
  }
}

// BAD (-∞): Fire-and-forget
function badSave() {
  api.post('/data', payload)  // No await, no error handling
}
```

---

## PART 4: State Management `[UDF, LoD, DIP]`

### Minimal Global State Principle

**State is local by default. Global only with explicit justification.**

Keep state as local as possible, global only when necessary. Treat server data differently from UI state.

| Pattern | Score | Notes |
|---------|-------|-------|
| Component-local state | +10 | Default pattern `[UDF]` |
| Shared-logic modules for shared behavior | +9 | Dialog control, auth session |
| Server state via queries/WebSocket | +8 | Not in the client store |
| Store for justified global state | +7 | Multi-feature coordination |
| Subtree dependency provision for deep trees | +7 | Avoids prop drilling `[LoD]` |
| Global state without justification | -∞ | Auto-reject `[UDF]` |
| Prop drilling >2 levels | -∞ | Auto-reject `[LoD]` |
| Singleton classes for state | -8 | Use a store or shared-logic module |
| Everything in the global store | -7 | Over-globalization `[UDF]` |

### State Location Decision Tree

```
Need state?
  └─ Shared across components?
      ├─ No → Local state (+10)
      └─ Yes → How many components?
          ├─ 2-3 in same tree → Shared-logic module (+9)
          ├─ Deep tree → subtree dependency provision (+7)
          ├─ Cross-feature → store (+7, requires comment)
          └─ Entire app → Justified global (+5, requires comment)
```

### Server-State Ownership `[UDF, DIP]`

**One owner per piece of server state — pick by uniqueness + who reads it, not just sharing breadth.**

Extends the State Location tree above with the server-vs-UI and uniqueness branch:

```
Server state — who owns it?
  ├─ App-global, one-of-a-kind, read OUTSIDE component setup (route guards)
  │     → module singleton exposing read-only state + operations
  ├─ Per-resource (one record/session)
  │     → component-owned, provided at the feature root, disposed on unmount
  ├─ Shared UI state in a subtree → subtree provision (read-only)
  └─ Cross-feature client coordination → store (justified)
```

Per-resource owners are disposed on unmount (PT 2).

### Shared-Logic Module Pattern

| Pattern | Score | Notes |
|---------|-------|-------|
| Modules return observed state + functions | +9 | Not classes |
| Exposed state is read-only; mutation via returned functions | +9 | `[UDF]` |
| Stateless factory pattern | +8 | State created per call |
| Co-located with their feature | +7 | `/features/auth/…` |
| Cross-feature modules in `/shared/` | +9 | `[DIP]` |
| Class-based composition | -8 | Use functions |
| Static methods for state | -7 | Not the platform idiom |

### Store UDF Compliance

A store provides UDF tools without mandating their use. Be explicit about which patterns you rely on.

| Pattern | Score | Notes |
|---------|-------|-------|
| Mutations via defined store actions only | +10 | Traceable, debuggable `[UDF]` |
| Subscription hooks for observation | +8 | UDF-compliant read path |
| Action interceptors for side-effect coordination | +7 | Middleware pattern `[UDF]` |
| Store state exposed to components read-only | +8 | Prevents bypassing actions `[UDF]` |
| Direct assignment into store state from a component | -8 | Bypasses actions, breaks traceability `[UDF]` |

### Confirmed Mutation with Server Reconciliation

**Mutations return certainty, not predictions. WebSocket handles external changes.**

When you have real-time state (WebSocket/SSE) AND user mutations (POST/PUT):
- **Confirmed Mutation**: 200 = success (state changed). Payload optional.
- **Reconciliation**: WebSocket pushes all non-local state changes (other users, background jobs, hardware/device state, server-side events)

**Two valid confirmation patterns:**

| Case | Response | Client Action |
|------|----------|---------------|
| Simple | `200 OK` (no body) | Apply expected state - server confirmed it |
| Complex | `200 OK` + payload | Apply payload as new state |
| Failure | `4xx/5xx` | State unchanged, handle error |

| Pattern | Score | Notes |
|---------|-------|-------|
| Await POST, apply confirmed state + WebSocket for external | +10 | Certainty + real-time `[UDF]` |
| Await POST, apply from response payload | +9 | Certainty, explicit |
| Await POST, apply expected state on 200 | +8 | Certainty, implicit |
| Optimistic with snapshot + rollback | +6 | Complex, latency-critical only `[UDF]` |
| WebSocket-only (ignore POST response) | +5 | Unnecessary stale UI window |
| Set state BEFORE POST returns (with rollback) | +6 | Optimistic, requires snapshot `[UDF]` |
| Set state BEFORE POST returns (no rollback) | -∞ | Auto-reject — shows unconfirmed state `[UDF]` |
| Mutation + subscription race (no coordination) | -9 | Flickering |
| Fire-and-forget POST | -∞ | Auto-reject |

**Quick Reference:**

```typescript
// Simple: 200 = confirmation, apply expected state
async function handleToggle() {
  const newValue = !enabled
  await api.post('/toggle', { enabled: newValue })  // 200 = confirmed
  enabled = newValue  // Certainty
}

// Complex: apply state from response payload
async function handleSave() {
  const result = await api.post('/save', data)  // 200 + payload
  state = result.data  // Certainty
}

// WebSocket: all non-local changes (other users, hardware, server events)
ws.onmessage = (event) => { state = event.data }
```

---

## PART 5: Props, Events & Communication `[UDF, LoD]`

### Props Down, Events Up

Props flow down (data), events flow up (behavior). Never mix.

| Pattern | Score | Notes |
|---------|-------|-------|
| Props for data down | +10 | Treated as immutable `[UDF]` |
| Props declared against a typed interface | +9 | Compile-time contract |
| Events declared with typed payloads | +9 | `[UDF]` |
| Subtree dependency provision for deep trees | +7 | Avoid prop drilling `[LoD]` |
| Mutating props | -∞ | Auto-reject `[UDF]` |
| Untyped/undeclared component events | -6 | Declare every event with its payload type |

### Two-Way-Bound Inputs: Stateless vs Stateful

**Two patterns: stateless (default) vs stateful (when justified).**

A two-way-bound input component is still UDF: value flows in as a prop, changes flow out as an event. Most implementations need no internal state.

#### When to Use Each Pattern

| Use Case | Pattern | Internal State? |
|----------|---------|-----------------|
| Simple wrapper (input, select) | Stateless | No |
| Pass-through to child component | Stateless | No |
| Data transformation/composition | Stateful | Yes |
| Debouncing/throttling | Stateful | Yes |
| Complex editor with lifecycle | Stateful | Yes |
| Validation (before emitting) | Derived value | No |

#### Scoring

| Pattern | Score | Notes |
|---------|-------|-------|
| Stateless: prop bound directly, change event emitted | +10 | Default pattern, 90% of cases `[UDF]` |
| Stateless: derived value for read-only transform | +9 | Derived display values |
| Stateful: internal state + observer on the incoming prop | +8 | Only when justified |
| Stateful: debounced emission | +7 | Common legitimate use |
| Loop-prevention guard when sync is bidirectional | +9 | Required for stateful `[UDF]` |
| Internal state WITHOUT an observer on the prop | -∞ | Auto-reject, external changes ignored `[UDF]` |
| Bidirectional observers without justification | -7 | Code smell, prefer stateless |

**Stateful requires three parts:**
1. Initialize internal state from the incoming prop
2. Observe the prop for external changes (parent → child)
3. Emit an event when internal state changes (child → parent)

**Common mistake (-∞):** Initialization-only — internal state copied from the prop once, no observer — external changes are silently ignored (AP-05).

---

## PART 6: Isolated Rendering & Testing `[LoD, DIP]`

### Query-Driven State

**UI state lives in the address bar. Every component variant must be accessible via URL query params.**

State in the route query = reproducible state. A URL like `?view=AgentCard&variant=loading&state=error` encodes exact UI state without navigation scripting. Consequences:

- **Agent testing**: URL = test input. No click-sequences, no navigation scripts. Agent opens URL, observes result.
- **Bug reports**: URL IS the bug report. `?view=AgentCard&variant=error&agentId=42` reproduces the exact state.
- **Screenshot automation**: Iterate over query param combinations programmatically. Each combo = one screenshot.
- **Neutral context loading**: URL encodes state without requiring goal-directed interaction — any observer can load the same context independently.
- **Proof artifacts**: A URL proves what state was tested. The URL itself is the artifact.

| Pattern | Score | Notes |
|---------|-------|-------|
| Query params control component state | +10 | `?view=Profile&state=loading` |
| All variants accessible via URL | +9 | Shareable test links |
| Component workbench stories | +8 | Living documentation |
| Deep linking to any component | +7 | Testing + debugging |
| State only in memory, no query mapping | -8 | Not reproducible via URL |
| Hardcoded dev state (`if (DEV) show(...)`) | -7 | Not addressable, not automatable |
| Components require full app mount | -9 | Not isolated `[LoD]` |
| State only via global store | -8 | Can't test in isolation `[DIP]` |
| No test stories/URLs | -7 | Poor testability |

Which of these variants earn a committed test: see the `testing` standard.

---

## PART 7: Routing & Navigation `[orthogonal]`

| Pattern | Score | Notes |
|---------|-------|-------|
| Lazy-loaded route components | +10 | Route code loaded on demand |
| Typed route params | +8 | Compile-time checked |
| Route guards for auth | +8 | Assert before entry |
| Named routes | +7 | Navigate by name, not hardcoded path strings |
| Query params for component state | +7 | Testing support |
| Manual query param parsing | -8 | Use the router's parsed query |
| State-based page switching | -7 | No history/deep links |
| Hardcoded navigation paths | -6 | Use named-route navigation |

**Exception**: Multi-app architecture may use manual routing per app (+5 with justification)

---

## PART 8: Type Organization `[DIP]`

| Pattern | Score | Notes |
|---------|-------|-------|
| Typed language mode for all components | +10 | No untyped component files |
| Co-located component types | +9 | `/ui/badge/BadgeTypes` |
| `/types/` for shared interfaces | +8 | Domain-organized `[DIP]` |
| Ambient declarations in one env declaration file | +7 | Global types |
| Typed props against an interface | +10 | Not runtime validation |
| Typed event payloads | +9 | Compile-time contract |
| 500+ line type files | -7 | Split by domain |
| Types mixed in implementation | -6 | Extract to `*Types` files |

### Type Essentials

| Pattern | Score | Notes |
|---------|-------|-------|
| Circular imports | -∞ | Auto-reject. Restructure architecture `[DIP]` |
| Type escape hatch without comment | -∞ | Auto-reject. Requires justification |
| `unknown`-style top type for unknown values | +9 | Forces checking before use |
| Block-scoped declarations only | +10 | Never function-scoped `var`-style |
| Optional chaining `?.` | +8 | Safe navigation |
| Nullish coalescing `??` | +8 | Default values |
| Generics `<T>` | +8 | Type-safe abstractions `[DIP]` |
| Interfaces for objects | +9 | Not untyped escape hatches |
| Escape hatch with justification comment | -5 | Last resort only |
| Function-scoped `var`-style declaration | -10 | Hoisting hazards |

### Documentation & Logging

| Pattern | Score | Notes |
|---------|-------|-------|
| Doc comments for API/tool consumption | +10 | OpenAPI, public libs |
| Types + clear names | +9 | Self-documenting code |
| No doc comments on internal code | +7 | Less maintenance overhead |
| `logger.error("msg", error)` | +10 | Full error object preserved |
| Redundant doc comments | -7 | `/** Adds two numbers */` |
| Error interpolated into a string | -10 | Loses stack trace |
| `console.log` in production | -9 | Use logger |

---

## PART 9: Asset Organization `[orthogonal]`

| Pattern | Score | Notes |
|---------|-------|-------|
| `/assets/styles/` for global CSS | +8 | Utility framework config, variables |
| `/assets/images/` organized | +7 | Not flat |
| CSS custom properties for theming | +9 | `--color-primary` |
| Utility classes bound to design tokens | +8 | Semantic classes |
| Co-located, scoped component styles | +7 | Style lives with the component |
| Flat `/assets/` with all files | -6 | Organize by type |
| Inline styles (non-dynamic) | -7 | Use stylesheet classes |

---

## PART 10: ASCII UI Representations

ASCII representations are an effective way to communicate UI designs before building them. When proposing a new component or significant UI change, sketching the layout in ASCII helps ensure alignment between collaborators on the intended visual structure and behavior before code is written.

---

## Summary

### Quick Reference

| Section | What It Covers |
|---------|---------------|
| Three Pillars | UDF, LoD, DIP — the *why* behind every rule |
| Auto-Reject | Hard stops across general + UDF + LoD + DIP, plus 18 named anti-patterns |
| PT 1: CDD | Isolation, 4-tier hierarchy, naming |
| PT 2: Lifecycle | Resource acquire/release pairing |
| PT 3: Async | No fire-and-forget, error handling |
| PT 4: State | Local-first, shared-logic modules, store UDF compliance, confirmed mutations |
| PT 5: Props/Events | Props down, events up, two-way-bound input patterns |
| PT 6: Testing | Query-driven rendering, isolation |
| PT 7: Routing | Lazy loading, typed params, guards |
| PT 8: Types | Type organization, essentials, logging |
| PT 9: Assets | Style/image organization |
| PT 10: ASCII UI | Recommended for communicating UI designs before building |

### Component Checklist

- [ ] File annotation (first line describes responsibility)
- [ ] Typed language mode
- [ ] Props declared against a typed interface, events declared with typed payloads
- [ ] Teardown on unmount (if resources acquired)
- [ ] All async operations awaited with error handling
- [ ] Local state only (unless justified)

---

## References

- [Storybook Component-Driven Development](https://storybook.js.org/)
- [Flutter App Architecture Concepts](https://docs.flutter.dev/app-architecture/concepts) — principle-preamble precedent
- [Redux Style Guide](https://redux.js.org/style-guide/) — three-principles precedent

---

## Proposed Additions *(not yet strictly enforced but tentatively recommended)*

### Build-Mode Commitment

**Commit to one build mode on purpose. The cost is the undecided middle, not choosing light.**

```
Lifecycle/ownership shape known?
  ├─ Yes → FULL: build identity/sessions/scoping/state-ownership/
  │         routing-as-state-machine/persistence-seam properly
  ├─ No  → LIGHT: thinnest throwaway scaffold; log throwaway intent +
  │         replace-trigger; build components CDD-portable (re-hang later)
  └─ Undecided/drifting → STOP (the costly middle)
```

| Pattern | Score | Notes |
|---------|-------|-------|
| Explicit build mode recorded | +10 | The decision is the artifact |
| Light + documented throwaway intent + replace-trigger | +9 | Safe disposability |
| Light + CDD-portable components | +9 | Re-uses PT 1 `[DIP]` |
| Full locks foundations only once full-committed | +8 | No pre-emptive scaffolding |
| Component-first UI ahead of unbuilt lifecycle | +8 | The CDD opening move, not a violation |
| Feature UI wired to shared/persistent state, no decided owner | -8 | Coupling, not ordering `[UDF]` |
| Half-built lifecycle / no committed mode | -9 | Drift |
| Throwaway-shell assumptions leaking into component interfaces | -8 | Pollutes portable surface `[LoD]` |

Foundations are laid when evidence arrives, not pre-emptively (see the `programming` standard: YAGNI).

### Indeterminate-State UX

**Pending UX must never look frozen, never flicker. Owned by scaffolding; leaves stay oblivious.**

Show nothing for a grace delay; once shown, hold a minimum-visible floor (tune both to response-time tiers). Never wire a raw loading flag straight to a spinner. Contextual, not a global blocking overlay.

```
Pending treatment (stop at first match):
  ├─ Resolves under the grace delay → no indicator
  ├─ A control triggered it → inline busy on that control
  ├─ Region has content & refreshing → keep + dim (stale-while-revalidate)
  ├─ Region empty / first-load → skeleton matched to final layout
  └─ Measurable & long → determinate bar; else indeterminate (never fake)
```

| Pattern | Score | Notes |
|---------|-------|-------|
| Delayed pending (grace delay + min-visible floor) | +9 | No freeze, no flicker |
| Async-region shell owns it, leaf oblivious | +9 | `[LoD]` |
| Reserve the box / no layout shift | +8 | Skeleton matches final layout |
| Raw loading flag → spinner | -8 | Flickers on fast responses |
| Replace live content with spinner on refresh | -7 | Lose stale-while-revalidate |
| Leaf component animates/loads itself | -7 | `[LoD]` |

### Visual Coherency & Design Tokens

**One product, one hand. Extend the style guide; never improvise a one-off.**

| Pattern | Score | Notes |
|---------|-------|-------|
| Single design-token source of truth, roles not raw hexes | +9 | One vocabulary |
| Extend the project style guide before novel UI | +9 | Reuse before invent |
| Motion durations as tokens, transitions scaffolding-owned | +8 | Leaf-oblivious (see Indeterminate-State UX) |
| New raw hex / font stack / bespoke per-screen styling | -8 | Drift from the system |

### Navigation Integrity (NAV) `[refines PT 7]`

**A view is reachable only through its prerequisite context, fed only by that context, with a legitimate exit from every state. The route graph is a hierarchy, not a jump table.**

DIP's spirit on the navigation axis: where DIP forbids a lower module reaching *up* an import edge, NAV forbids a view being landed on *outside* its parent context. An auth requirement is necessary, not sufficient — being logged in does not prove a view's parent context exists. The scored rows below cover three facets: reachability (guarded entry), data-provenance (fed by entered context), exitability (every state has an exit).

| Pattern | Score | Notes |
|---------|-------|-------|
| Context guard asserts prerequisite, redirects up on miss | +10 | Reachability `[NAV]` |
| Child view nested under its parent route | +9 | Hierarchy structural, not flat `[NAV]` |
| Redirect-up targets the parent, not a generic page | +8 | Preserves intent `[NAV]` |
| View fed only by its entered context | +9 | Provenance-bound `[NAV]` |
| Every mode/takeover ships its exit control | +8 | Exitability `[NAV]` |
| Child view reachable with only an auth check / no context guard | -9 | Context-orphaned route `[NAV]` |
| Flat top-level route for a child concept | -8 | Nest or guard `[NAV]` |
| Context-bound view rendering a lateral/demo feed | -8 | Provenance break `[NAV]` |
| Redirect-on-missing-context to a generic landing | -7 | Redirect up, not sideways `[NAV]` |
| Mode toggle / takeover with no exit path | -8 | Strands the user `[NAV]` |

**Enforceability**: guard logic is runtime, so the rest is route-table review: *for every route, name its prerequisite context and the guard that asserts it.* On graduation: promote into PT 7 (retag `[orthogonal]` → `[NAV]`), add the matrix NAV column, and lift the three violations into named auto-rejects/anti-patterns.
