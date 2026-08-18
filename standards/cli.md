<!-- Concern: specifies the CLI surface agents invoke - predictable, parseable, safe | Non-concern: measuring how agents fare with it | IO: none -->
# CLI Interface Standards for AI Agent Consumption

Design CLI tools that machines can reliably invoke. Predictable. Parseable. Safe.

---

## AUTO-REJECT (Stop Work Immediately)

**Universal Blockers** (-∞):

- **Interactive prompts**: Blocks agent execution → Use `--confirm` flag or `--yes`
- **Secrets as CLI arguments**: Visible in process list, shell history → Environment variables only
- **Human-only output**: "Successfully created!" → Structured JSON with machine-parseable status
- **Mixed stdout/stderr for data**: JSON to stderr → All data to stdout, debug only to stderr

---

## PART 1: Command Structure

### Subcommand Pattern (Docker/Git Style)

**Structure**: `tool <verb> <resource> [--flags]`

```bash
# Pattern
<tool> <verb> <resource> [--flags]

# Examples
work-plan show task-123 --format json
status-tracker add --title "New task" --priority high
stripe licenses --customer cus_123
docker container ls --all
git branch delete feature-x --force
```

**Standard Verbs** (use consistently across all tools):

| Verb | Purpose | Idempotent |
|------|---------|------------|
| `list` | Collection retrieval | Yes |
| `show` | Single resource details | Yes |
| `add` / `create` | New resource | No* |
| `update` | Modify existing | No* |
| `delete` / `remove` | Remove resource | Yes |
| `validate` | Check without mutation | Yes |

*Can be made idempotent with `--idempotency-key`

**Anti-pattern**: Inconsistent verbs across tools

```bash
# BAD: Mixed conventions
work-plan get task-123      # "get" vs "show"
status-tracker new          # "new" vs "add"
config fetch                # "fetch" vs "show"

# GOOD: Consistent verbs
work-plan show task-123
status-tracker add
config show
```

---

### Version Flag (Mandatory)

**Every CLI tool must implement `--version` / `-V`.**

```bash
mytool --version
# stdout: {"status": "success", "data": {"name": "mytool", "version": "1.2.3"}}
# exit: 0

mytool -V
# Same output (short form)
```

**Output specification**:

```json
{
  "status": "success",
  "data": {
    "name": "toolname",
    "version": "1.2.3"
  }
}
```

**Version format**: Semantic versioning (major.minor.patch)

**Behavior**:
- Works anywhere in command: `mytool --version`, `mytool task add --version`
- Overrides all other arguments (displays version, ignores rest)
- Exit code 0 on success
- Exit code 1 if version unavailable/corrupted

**Error case** (version file missing/corrupted):

```json
{
  "status": "error",
  "error": {
    "code": "internal_error",
    "message": "Version information unavailable"
  }
}
```

**Why**:
- Agent compatibility checks before invocation
- Debugging environment mismatches
- CI validation of installed versions

**Exception to subcommand pattern**: `--version` and `--help` are global flags, not subcommands.

```bash
# Both valid
mytool --version
mytool task add --version  # Still shows version, doesn't add task
```

**Note**: `-V` and `--help` are the ONLY short flags in these standards. All other flags use long form (`--flag`, not `-f`).

---

### Named Arguments Over Positional

**Prefer `--identifier` over positional arguments.**

```bash
# BAD: Positional ambiguity
mytool process report.csv output/ 2024-01-15

# GOOD: Named and clear
mytool process --input report.csv --output-dir output/ --date 2024-01-15
```

**Exception**: Single obvious resource (file path, ID)

```bash
# OK: Single obvious positional
cat /path/to/file
git show abc123
mytool show task-123
```

**Rule**: If >1 positional would be needed, use named arguments for all.

---

## PART 2: Output Format

### Unified Output Envelope

**All tools return consistent JSON structure.**

```json
{
  "status": "success",
  "data": { ... },
  "meta": {
    "request_id": "req_abc123",
    "timestamp": 1704825600
  }
}
```

**Error response**:

```json
{
  "status": "error",
  "error": {
    "code": "validation_error",
    "message": "Invalid date format",
    "details": {
      "field": "start_date",
      "expected": "YYYY-MM-DD",
      "received": "01-15-2024"
    }
  },
  "meta": {
    "request_id": "req_abc123",
    "timestamp": 1704825600
  }
}
```

**Standard Error Codes**:

| Code | Exit Code | Meaning |
|------|-----------|---------|
| `validation_error` | 3 | Input failed validation |
| `not_found` | 24 | Resource doesn't exist |
| `auth_error` | 34 | Authentication/authorization failed |
| `conflict` | 4 | Resource state conflict |
| `rate_limited` | 5 | Too many requests |
| `internal_error` | 1 | Unexpected server error |

---

### Diagnostics (Findings About Evaluated Input)

**Two failures, two shapes.** The flat `error` envelope describes an **operational** failure of the
*invocation* — bad args, not found, auth, rate limit. But a tool whose job is to **evaluate input** —
a validator, compiler, linter, type-checker — can run perfectly and still find **many located
problems in the subject**. Report those as a `diagnostics[]` array, not one `internal_error` string
that throws away count, location, severity, and remediation.

**Shape** — `diagnostics` lives in `data`, present on `success` OR `error`:

```json
{
  "status": "error",
  "error": { "code": "validation_error", "message": "1 error, 1 warning" },
  "data": {
    "diagnostics": [
      {
        "code": "schema.unknown_field",
        "severity": "error",
        "message": "Unknown field 'retires'",
        "location": {
          "file": "config.yaml",
          "span":  { "offset": 142, "length": 7 },
          "start": { "line": 12, "column": 3 },
          "end":   { "line": 12, "column": 10 }
        },
        "docs_url": "https://docs.example.com/errors/schema.unknown_field",
        "help": "Did you mean 'retries'?",
        "fix": {
          "applicability": "maybe_incorrect",
          "edits": [ { "file": "config.yaml", "span": { "offset": 142, "length": 7 }, "replacement": "retries" } ]
        }
      }
    ]
  },
  "meta": { "timestamp": 1704825600 }
}
```

**Per diagnostic** (only `code`, `severity`, `message` required):

| Field | Required | Purpose |
|-------|----------|---------|
| `code` | yes | Stable, namespaced **dispatch key**. Agents branch on this — never on message text. |
| `severity` | yes | `error` \| `warning` \| `advice`. Orthogonal to the verdict (below). |
| `message` | yes | One-line human summary. Not the dispatch key. |
| `location` | when locatable | `file` + byte `span` (`offset`,`length` — machine-exact) AND/OR `start`/`end` `line`:`column` (1-based). Provide both when you can. |
| `docs_url` | recommended | Stable URL for the `code`. Cacheable; ends string-matching. |
| `help` | recommended | Remediation prose — what to change. |
| `fix` | when known | Structured edit + `applicability` gate (below) — lets an agent *apply* instead of *infer*. |
| `related` | optional | Nested diagnostics (a cause chain). |

**`fix` carries an applicability gate** so the agent knows whether it may apply unattended:

```json
"fix": {
  "applicability": "machine_applicable",   // machine_applicable (apply) | maybe_incorrect (review) | has_placeholders (fill in)
  "edits": [ { "file": "config.yaml", "span": { "offset": 142, "length": 7 }, "replacement": "retries" } ]
}
```

**Severity is orthogonal to the verdict.** Diagnostics appear on `success` (warnings/advice on
accepted input → exit 0) and on `error` (an `error`-severity finding rejected the input →
`validation_error`, exit 3). Drive `status`/exit from the **verdict** (did the tool reject the
input?), never from "are there any diagnostics?".

**Dual-render from one object.** The same diagnostic renders as a colored, spanned terminal view and
as this JSON, selected by `--format` / TTY detection — never two code paths. The JSON form is
reliably parseable and far cheaper per debug-retry than a rendered traceback, which compounds across
retry loops.

**Schema.** In the spirit of RFC 9457 Problem Details (an HTTP-API error format, adapted here for
CLI) plus the `location`/`fix` extensions. Render from a mature diagnostic/span library in your
language rather than hand-rolling span math.

---

### Stdout for Everything

**JSON output always to stdout. Including errors.**

```bash
# Correct: All structured output to stdout
mytool show task-123
# stdout: {"status": "success", "data": {...}}

mytool show nonexistent
# stdout: {"status": "error", "error": {"code": "not_found", ...}}
```

**stderr only for**:
- Debug/verbose logging (`--verbose`, `--debug`)
- Progress indicators (`--progress`)
- Warnings that don't affect output

```bash
# stderr for progress (when --progress flag used)
mytool process large-file.csv --progress
# stderr: Processing... 50%... 100%
# stdout: {"status": "success", "data": {...}}
```

**Why**: Agents parse stdout. Mixing data streams breaks parsing.

---

### Pagination Metadata

**Collections must include pagination info.**

```json
{
  "status": "success",
  "data": {
    "items": [...],
    "pagination": {
      "count": 25,
      "has_more": true,
      "next_cursor": "eyJpZCI6MTAwfQ=="
    }
  }
}
```

**Cursor-based, not offset-based**:

```bash
# BAD: Offset pagination (fragile with mutations)
mytool list --offset 100 --limit 25

# GOOD: Cursor pagination (stable)
mytool list --cursor eyJpZCI6MTAwfQ== --limit 25
```

**Why cursors**:
- Stable under concurrent mutations
- No "skip N rows" performance penalty
- Opaque to client (server controls format)

---

### Handling Empty Results

**Never ambiguous empty states.**

```json
// Empty collection - explicit
{
  "status": "success",
  "data": {
    "items": [],
    "pagination": {
      "count": 0,
      "has_more": false,
      "next_cursor": null
    }
  }
}

// Not found - error, not empty
{
  "status": "error",
  "error": {
    "code": "not_found",
    "message": "Task task-999 not found"
  }
}
```

**Anti-pattern**: Ambiguous responses

```bash
# BAD: Is this empty or error?
mytool list  # outputs: null
mytool list  # outputs: (nothing)
mytool show task-999  # outputs: {}
```

---

## PART 3: Safety and Reliability

### Safety Confirmation Pattern

**Dangerous operations require explicit confirmation.**

```bash
# Default: Preview mode (safe)
mytool delete --filter "status=archived"
# stdout: {"status": "success", "data": {"preview": true, "would_delete": 47}}

# Explicit confirmation required for execution
mytool delete --filter "status=archived" --confirm
# stdout: {"status": "success", "data": {"deleted": 47}}
```

**Dangerous operations**:
- `delete` / `remove` (bulk especially)
- `update` with `--all` or filters
- Destructive migrations
- Production deployments

**Pattern**:

```bash
# Step 1: Preview (default)
mytool dangerous-operation [args]
# Returns: what WOULD happen

# Step 2: Execute (explicit)
mytool dangerous-operation [args] --confirm
# Executes the operation
```

---

### Meaningful Exit Codes

**Consistent exit codes across all tools.**

| Exit Code | Meaning | When |
|-----------|---------|------|
| 0 | Success | Operation completed |
| 1 | General error | Unexpected failure |
| 2 | Bad arguments | Invalid CLI usage |
| 3 | Validation error | Input failed validation |
| 4 | Conflict | Resource state conflict |
| 5 | Rate limited | Too many requests |
| 24 | Not found | Resource doesn't exist |
| 34 | Auth error | Authentication failed |

**Implementation**:

```bash
# Agent can branch on exit code
if mytool show task-123; then
  # Process success
elif [ $? -eq 24 ]; then
  # Handle not found
elif [ $? -eq 34 ]; then
  # Handle auth error
fi
```

**Note**: Exit code AND JSON error should be consistent.

---

### Idempotency

**Same input should produce same output.**

```bash
# Idempotent by nature
mytool show task-123      # Always same result for same state
mytool validate input.json # Always same validation result

# Idempotent with key
mytool create --title "Task" --idempotency-key "create-task-abc123"
# First call: creates task
# Second call: returns existing task (no duplicate)
```

**External calls require idempotency keys**:

```bash
# Payment processing
stripe charge create \
  --amount 1000 \
  --idempotency-key "order-12345-charge"
```

**Why**: Network retries, agent restarts shouldn't cause duplicates.

---

### Environment-Based Secrets

**Never pass secrets as CLI arguments.**

```bash
# BAD: Visible in ps, shell history
mytool --api-key sk_live_abc123 list

# GOOD: Environment variable
export MYTOOL_API_KEY=sk_live_abc123
mytool list

# GOOD: File reference
mytool --api-key-file ~/.mytool/credentials list
```

**Convention**: `TOOLNAME_API_KEY`, `TOOLNAME_SECRET`

**Why**:
- CLI args visible in process list (`ps aux`)
- Shell history persistence
- Log capture risks

---

## PART 4: Help and Documentation

### Comprehensive Help Text

**Help must enable agent self-correction.**

```bash
mytool task add --help

USAGE:
  mytool task add --title <string> [--priority <low|medium|high>] [--due <YYYY-MM-DD>]

DESCRIPTION:
  Create a new task in the current project.

ARGUMENTS:
  --title       (required) Task title, 1-200 characters
  --priority    (optional) Priority level. Default: medium
  --due         (optional) Due date in YYYY-MM-DD format

EXAMPLES:
  # Basic task
  mytool task add --title "Review PR #123"

  # Full options
  mytool task add --title "Deploy v2.0" --priority high --due 2024-01-20

OUTPUT:
  {
    "status": "success",
    "data": {
      "id": "task-abc123",
      "title": "Review PR #123",
      "priority": "medium",
      "due": null,
      "created_at": 1704825600
    }
  }

EXIT CODES:
  0   Success
  2   Invalid arguments
  3   Validation error (title too long, invalid date)
  34  Authentication error

SEE ALSO:
  mytool --version    Show version information
  mytool task list    List all tasks
  mytool task show    Show task details
  mytool task update  Modify existing task
```

**Required help sections**:
1. Usage pattern with types
2. All arguments with defaults
3. Realistic examples with actual values
4. Output JSON structure
5. Exit codes
6. Related commands

---

## PART 5: Anti-Patterns

### Patterns That Block AI Agents

| Anti-Pattern | Problem | Fix |
|--------------|---------|-----|
| Interactive prompts | Agent can't respond | `--confirm` / `--yes` flags |
| Human-readable only | Can't parse "Created task #123!" | JSON envelope |
| Findings lumped into one error | N located problems → one `internal_error` string | `diagnostics[]`: code + location + `fix` per finding |
| Located problem, no location | "type error somewhere" | `location` with byte span AND line:col |
| Fix as prose only | Agent must infer the edit | `fix.edits` + `applicability` gate |
| Ambiguous empty | `null` vs `[]` vs `{}` vs nothing | Explicit empty collections |
| Mixed null handling | `null` vs `"null"` vs missing key | Consistent: key present, value null |
| Paged without metadata | No way to get next page | Include `has_more`, `next_cursor` |
| Secrets as args | Security risk | Environment variables |
| Non-zero exit on warnings | Breaks automation | 0 for success with warnings in output |
| Progress to stdout | Corrupts JSON | Progress to stderr only |

### Null Handling Consistency

**Pick one convention. Document it. Never mix.**

```json
// Convention: Missing means not requested, null means no value
{
  "name": "Task",
  "description": null,     // Explicitly has no description
  // "metadata" not present - wasn't requested/relevant
}
```

**Anti-pattern**: Mixed conventions

```json
// BAD: What does each mean?
{"description": null}
{"description": ""}
{"description": "null"}
// "description" key missing entirely
```

---

## Summary

| Principle | Essence | Violation Signal |
|-----------|---------|------------------|
| **Version Flag** | `--version` / `-V` → JSON with semver | Missing version implementation |
| **Subcommand Pattern** | `tool verb resource --flags` | Inconsistent command structure |
| **Unified Envelope** | `{status, data, error}` | Raw output without wrapper |
| **Diagnostics** | Located, severity-tagged, fixable findings in a `diagnostics[]` array (RFC 9457-style) | N input findings collapsed to one error string |
| **Stdout for Data** | JSON to stdout, debug to stderr | Mixed output streams |
| **Named Arguments** | `--flag value` over positional | Multiple positional args |
| **Pagination** | Cursor-based with metadata | Offset pagination, missing `has_more` |
| **Safety Confirmation** | `--confirm` for dangerous ops | Auto-execute destructive commands |
| **Help Text** | Examples, output schema, codes | Minimal `--help` |
| **Exit Codes** | Meaningful, consistent codes | Exit 1 for everything |
| **Environment Secrets** | `TOOL_API_KEY` | Secrets as CLI args |
| **Idempotency** | Same input = same output | Duplicate creates on retry |

---

## Quick Reference

### Global Flags

```bash
mytool --version    # Show version (works anywhere)
mytool -V           # Short form
mytool --help       # Show help
```

### Command Template

```bash
mytool <verb> <resource> \
  --required-arg value \
  --optional-arg value \
  --format json
```

### Output Template

```json
{
  "status": "success|error",
  "data": {},
  "error": {"code": "", "message": "", "details": {}},
  "meta": {"request_id": "", "timestamp": 0}
}
```

### Exit Code Reference

```
0  = Success
2  = Bad arguments
3  = Validation error
4  = Conflict
5  = Rate limited
24 = Not found
34 = Auth error
```

### Environment Variables

```bash
export TOOLNAME_API_KEY=secret
export TOOLNAME_DEBUG=1
export TOOLNAME_CONFIG=/path/to/config.json
```
