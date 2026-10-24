# Conventions (beyond AGENTS.md and the go-style-guide skill)

Full rules: `.agents/skills/go-style-guide/SKILL.md` (load before any .go edit). Lint traps that
repeatedly bite:

- varnamelen: no short names in large scopes (`tz`→`timezone`, `wg`→`waitGroup`).
- No builtin shadowing (`copy`, `len`, `cap`, `new`, `close`, `delete`, `append`, `error`, also
  `set`).
- exhaustive: explicit `case` for every enum value. funcorder: exported methods before unexported.
- No inline `if err := ...; err != nil`; unique error names per call site (`hookErr`, `tcpErr`) —
  duplicate names trigger a linter auto-fix redeclaration bug.
- Stateless methods: unnamed receiver `func (*T) M()` (satisfies both staticcheck ST1006 and revive
  unused-receiver).
- forbidigo bans `fmt.Printf`: use `fmt.Fprintf(os.Stdout, ...)`.
- gocritic hugeParam/rangeValCopy: pass large structs by pointer, iterate by index.
- `atomic.Value` over `atomic.Int32` for small enums (avoids gosec G115).
- Every `Test*` starts with `t.Parallel()`.

Suppressions (`//nolint:<linter> // reason`, shellcheck/hadolint ignores) are ratcheted per rule
against `.suppressions-baseline`; a commit adding or moving one must update the baseline in the same
commit. The reason must say why the fix does not apply, not which rule fired.

Tests: fake timer factories fire immediately when delay <= 0 (set the clock past fireAt before
scheduling); capture logs via `logger.Set(slog.New(handler))`.

Commits: Conventional Commits with mandatory scope; type chosen by whether it should ship
(`fix`/`feat` release, others don't); avoid words ending in `fix`/`feat` before a colon in other
subjects (svu matches them).
