<!-- Concern: specifies the CLI surface agents invoke - predictable, parseable, safe | Non-concern: measuring how agents fare with it | IO: none -->
# CLI Interface Standards for AI Agent Consumption

Design CLI tools that machines can reliably invoke. Predictable. Parseable. Safe.

## AUTO-REJECT (-∞)

- Interactive prompts (blocks agent execution) — use `--confirm`/`--yes`
- Secrets as CLI args (visible in `ps`, shell history) — env vars only
- Human-only output ("Success!") — structured JSON
- Data on stderr, or data split across stdout/stderr — all data goes to stdout

## Command Structure

- Pattern: `tool <verb> <resource> [--flags]` (Docker/Git style), consistent across every tool in a fleet.
- Standard verbs: `list`, `show`, `add`/`create`, `update`, `delete`/`remove`, `validate`. All idempotent except add/update (make those idempotent via `--idempotency-key`).
- `--version`/`-V` mandatory: valid anywhere in the command line, overrides all other args, prints `{"status":"success","data":{"name","version"}}` (semver) to stdout, exit 0 (exit 1 + `internal_error` if unavailable). `-V` and `--help` are the *only* short flags — everything else is long-form.
- Prefer named args (`--flag value`) over positional. Exception: a single obvious resource (path, ID). If more than one positional would be needed, name all of them.

## Output Envelope

- Every response: `{"status": "success"|"error", "data": {...}, "meta": {"request_id","timestamp"}}`; errors add `"error": {"code","message","details"}`.
- Standard error codes → exit codes: `validation_error`→3, `not_found`→24, `auth_error`→34, `conflict`→4, `rate_limited`→5, `internal_error`→1. Exit code and `error.code` must always agree.
- All JSON — success and error alike — goes to stdout. stderr is only for `--verbose`/`--debug` logging and `--progress` indicators, never data.
- Collections carry `pagination: {count, has_more, next_cursor}`, cursor-based not offset-based — offsets break under concurrent mutation and pay an O(N) skip cost; cursors are opaque and stable.
- No ambiguous empty state: an empty collection is `{"items": [], "pagination": {count:0, has_more:false, next_cursor:null}}`, never `null`/`{}`/nothing. A missing single resource is a `not_found` error, never an empty success.
- Null convention: pick one meaning for "key missing" vs "value null" and never mix (e.g. missing = not requested, `null` = explicitly no value). Never emit `""` or the string `"null"` for the same case elsewhere.

## Diagnostics (evaluator tools: validators, linters, compilers)

A tool whose job is to *evaluate input* can run correctly and still find many located problems in the subject — report those as `data.diagnostics[]`, not one `internal_error` string that discards count, location, severity, and remediation. Diagnostics appear on `success` (warnings/advice, exit 0) or `error` (an `error`-severity finding rejected the input, exit 3): drive `status`/exit code from the verdict ("did the tool reject the input"), never from "are there any diagnostics".

Per diagnostic:

| Field | Required | Purpose |
|---|---|---|
| `code` | yes | Stable, namespaced dispatch key — agents branch on this, never on `message` |
| `severity` | yes | `error`\|`warning`\|`advice` — orthogonal to the verdict |
| `message` | yes | One-line human summary |
| `location` | when locatable | byte `span` (offset,length) AND/OR `start`/`end` line:column (1-based) — give both when possible |
| `docs_url` | recommended | stable per-`code` URL |
| `help` | recommended | remediation prose |
| `fix` | when known | `{applicability: machine_applicable\|maybe_incorrect\|has_placeholders, edits:[{file,span,replacement}]}` — lets an agent apply instead of infer |
| `related` | optional | nested diagnostics (a cause chain) |

Dual-render the same diagnostic object as a colored terminal view and this JSON (selected by `--format`/TTY), one code path — never two. Modeled on RFC 9457 Problem Details plus the `location`/`fix` extensions; render from a span library in your language rather than hand-rolling offset math.

## Safety & Reliability

- Dangerous ops (bulk delete, `--all` update, destructive migration, prod deploy) default to preview mode (`{"preview": true, "would_delete": N}`); require `--confirm` to actually execute.
- Idempotency keys for anything with external/side-effecting consequences (payments, external API calls) so network retries and agent restarts don't duplicate effects.
- Secrets never as CLI args (visible in `ps`, shell history, logs) — env vars (`TOOLNAME_API_KEY`) or a `--*-file` reference.

## Help Text

`--help` must give an agent enough to self-correct without a blind retry: usage pattern with types, every argument and its default, realistic examples with real values, the output JSON shape, exit codes, and related commands.
