<!-- Concern: governs frontend app architecture, framework-neutral | Non-concern: picking the frontend stack or per-framework API idiom | IO: none -->
# Frontend Project Standards

## Three Pillars

**UDF (Unidirectional Data Flow)**: state mutations flow through defined, traceable paths — props down, events up, mutate only at the owner. Breaks: mutating a prop or a shared object at a distance, reaching for a parent/root instance. A store gives UDF tools (mutation functions, subscription hooks) without mandating them.

**LoD (Law of Demeter)**: components talk only to immediate neighbors — parent via events, children via props, own shared-logic modules, explicitly typed injected deps. Violations: prop drilling >2 levels, reaching for parent/root or child-instance handles, ambient (unscoped) dependency provision, store train-wrecks (`store.user.settings.theme.color`).

**DIP (Dependency Inversion)**: lower tiers never import higher tiers; depend on abstractions. Import DAG: `apps/ → features/ → components/ → ui/`, all converging on `shared/ → types/`. No upward or cross-feature arrows. Enforced via slots, typed injection keys, module return-type contracts, props/events — not a DI container.

## Auto-Reject (-∞)

General: fire-and-forget async · global state without justification · missing lifecycle cleanup (teardown for subscriptions/timers/resources) · component file missing its first-line annotation · prop drilling >2 levels · circular imports · type-escape hatch without a justification comment.

UDF: shared mutable state exported and mutated by multiple consumers (use a store, or read-only state + mutators) · state set before server confirmation with no rollback (an optimistic update with snapshot+rollback is allowed, just scored lower) · bidirectional sync with no loop-prevention guard · mutating injected shared state directly.

LoD: calling a child component's methods via an instance handle (raw DOM handles for focus/play/canvas are fine) · parent/root instance access · untyped/bare-string dependency-provision keys · reaching siblings/cousins via chained handles or component-addressed events (lift state to the common ancestor, or use a store).

DIP: upward tier import · cross-feature direct import (route through `/types/` or a store) · a `/shared/**` module with a hidden global dependency instead of parameter injection (feature-internal modules are exempt unless re-exported from the barrel) · business/service/store logic inside `ui/` primitives.

### Named anti-patterns (FATAL=-∞, SEVERE=-9/-8, MODERATE=-7/-6)

Phantom Async (FATAL,UDF) async with no `await` · Premature Optimist (MODERATE,UDF) state set before an awaited POST/PUT resolves, no rollback · Derived-State Side Effect (SEVERE,UDF) mutation/API calls inside a derived-state computation · Prop Mutator (FATAL,UDF) pushing into or reassigning a received prop.

Broken Bridge (FATAL,UDF) state copied from a prop once, no observer for later changes · Prop Telephone (FATAL,LoD) one prop threaded through 4+ files · Untyped Event (MODERATE,LoD) component event with no payload type · Provision Junkyard (MODERATE,LoD) >3 subtree dependency provisions from one component.

Circular Ouroboros (FATAL,DIP) module A imports B imports A · Escape Hatch (FATAL) type-escape hatch with no comment · Zombie Subscription (FATAL) resource acquired at setup, no teardown at unmount · Invisible Observer (SEVERE) async observer callback with no cancellation.

Headless Chicken (FATAL) component file's first line isn't a responsibility annotation · State Hoarder (MODERATE) store/module with >10 independent state pieces · God Component (SEVERE) component file >300 lines or >5 async functions · then-Chain Spaghetti (MODERATE) `.then().then().catch()` instead of async/await · Stale Error Swallower (SEVERE) async function with no try/catch · Memory-Only State (SEVERE,LoD) >3 visual states, no URL query-param mapping.

## Component-Driven Development

Build bottom-up, each layer independently renderable/testable via a component workbench or URL query params: `ui/` (headless primitives) → `components/` (app composites, in `display/forms/layout/media` subdirs) → `features/` (domain modules) → `apps/` (entry points), alongside `shared/` (reusable state+logic) and `types/`. One component per file, one case convention, file annotation on line 1.

## Lifecycle

Resource lifetime = component lifetime: every subscription/timer/WebSocket/listener acquired at mount is torn down in the unmount hook — not via a manually invoked cleanup function.

## Async

Every async call is `await`ed with `try/catch`; independent calls run in parallel (e.g. `Promise.all`), dependent calls sequentially; show a loading state during the await. No `.then()` chains, no discarding a promise.

## State Management

Default to component-local state; escalate only as sharing needs grow: 2-3 components in one tree → shared-logic module; deep tree → scoped dependency provision; cross-feature → store (needs a comment); whole-app → justified global (needs a comment).

Shared-logic modules return observed state + mutator functions (never classes/static methods), are stateless factories, and live with their feature unless cross-feature (then `/shared/`). A store's state is read-only to components — mutate only via defined actions; direct assignment into store state bypasses traceability.

**Server-state ownership** — one owner per piece, picked by uniqueness + who reads it: an app-global singleton (read outside component setup, e.g. route guards) exposes read-only state + operations; per-resource state is owned by the component that provides it at the feature root and disposed on unmount; subtree-shared UI state uses scoped provision (read-only); cross-feature coordination uses a justified store.

**Confirmed mutation + reconciliation**: when both real-time push (WebSocket/SSE) and user mutations (POST/PUT) touch the same state, treat the 200 response as certainty — apply its payload, or the expected state on a bare 200 — and let the push channel carry only *other* sources' changes (other users, background jobs, hardware, server events). Setting state before the POST resolves, with no rollback path, is an auto-reject.

## Props, Events & Two-Way Binding

Props are immutable and typed against an interface; events are typed by payload. A two-way-bound input is still UDF underneath (value in as a prop, change out as an event) — default to **stateless** (bind the prop directly, emit on change; covers most cases: simple wrappers, pass-through, read-only derived display).

Go **stateful** only when justified (debouncing, composition/transformation, complex internal lifecycle), and then all three parts are required: initialize from the incoming prop, observe the prop for later external changes, emit on internal change. Internal state with no observer on the prop silently ignores external updates (Broken Bridge).

## Isolated Rendering & Testing

UI state belongs in the URL query string, not only in memory: every component variant should be reachable via `?view=X&state=Y` params. This makes agent testing script-free (open URL, observe), makes the URL itself the bug report, and enables scripted screenshot sweeps over param combinations. Which variants earn a *committed* test: see the `testing` standard.

## Routing

Route components lazy-loaded, params typed, auth via guards asserting *before* entry, navigation by named route (not hardcoded path strings), state-relevant params kept in the query for testability. Manual query parsing (bypassing the router) and state-based page switching (no deep link) are violations. Exception: a multi-app architecture may justify manual per-app routing.

## Types

Every component file is fully typed (no untyped/`any`-escape without a justification comment); props/events typed against interfaces, not validated only at runtime. Shared interfaces live in `/types/` (which imports nothing); component-local types co-locate (`BadgeTypes`); one ambient declaration file for globals. Split type files over ~500 lines by domain.

**Logging**: always `logger.error("msg", error)` with the full error object attached — interpolating an error into a string loses the stack trace. No `console.log` in production code.

## Assets

Global CSS/tokens in `/assets/styles/`, images organized by type (not flat); theme via CSS custom properties, semantic utility classes bound to tokens; component styles co-located and scoped. Non-dynamic inline styles are a violation.

## Proposed (not yet enforced)

- **Build-mode commitment**: known lifecycle/ownership shape → build FULL (identity, sessions, state ownership, routing-as-state-machine, persistence, done properly). Unknown → build LIGHT (throwaway scaffold, CDD-portable components, documented replace-trigger). Never leave it undecided/drifting.

- **Indeterminate-state UX**: no raw loading-flag-to-spinner wiring. Show nothing for a grace delay; once shown, hold a minimum-visible floor; prefer inline-busy-on-the-control or stale-while-revalidate (keep+dim) over a blocking overlay; skeletons match final layout to avoid shift. Owned by the async-region shell — leaves stay oblivious.

- **Visual coherency**: extend the existing design-token system (roles, not raw hex) before inventing new UI; motion durations are tokens too, owned by scaffolding, not leaves.

- **Navigation integrity**: a view is reachable only through its prerequisite parent context (nested route + a guard that redirects *up* on miss), fed only by that entered context, with an explicit exit from every mode/takeover. Being authenticated doesn't prove the parent context exists.
