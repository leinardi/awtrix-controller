# awtrix-controller

Go embedded MQTT broker for Awtrix3 displays. Behavior spec: `SPEC.md`. Work packages: `PLAN.md`. Style guide: `CONVENTIONS.md`.

## Commands

```bash
go build ./...       # verify compilation
go test -race ./...  # run all tests with race detector
make check-stage     # run linter on staged files (golangci-lint + go test)
```

Run `go build` and `go test -race` and `make check-stage` before marking any WP done.

## Project Constraints

- **No HTTP, no Prometheus** — MQTT-only via `github.com/wind-c/comqtt/v2`
- **No pflag** — stdlib `flag` only; dual-register long+short flags; env-var fallback via `flag.Visit` after `flag.Parse()`
- **`main()`** — calls `os.Exit(run())` only; all logic and deferred cleanup live in `run() int`
- **MIT license header** on every `.go` file — copy from `cmd/awtrix-controller/version.go`
- **context7** — use `mcp__context7__resolve-library-id` + `mcp__context7__query-docs` before writing code against any third-party library

## WP Status

Follow PLAN.md order; never start a WP until its dependencies are complete and passing.

- [x] WP-01 — Module Manifest Reset
- [x] WP-02 — Data Models (`internal/model/`)
- [x] WP-03 — Logger + Clock (`internal/logger/`, `internal/clock/`)
- [x] WP-04 — Configuration (`internal/config/`)
- [x] WP-05 — Client State Registry (`internal/clientstate/`)
- [x] WP-06 — Scheduler (`internal/scheduler/`)
- [x] WP-07 — Day/Night Mode (`internal/daynight/`)
- [x] WP-08 — Energy-Saving Mode (`internal/energysaving/`)
- [x] WP-09 — Notifications (`internal/notification/`)
- [x] WP-10 — Settings Composer + MQTT Broker (`internal/settings/`, `internal/broker/`)
- [ ] WP-11 — Entry Point + Wiring (`cmd/awtrix-controller/`)

## Linter Gotchas

Fix these proactively — they fire on almost every WP:

- **`varnamelen`**: short names only in ≤3-line scopes. `cs`→`state`, `wg`→`waitGroup`, `tz`→`timezone`, `id`→`clientID`
- **`builtinShadow`**: never use a Go builtin as a variable name (`copy`, `len`, `cap`, `new`, `close`, `delete`, `append`, `error`)
- **`paralleltest`**: every top-level `Test*` must call `t.Parallel()` as its first statement
- **`funcorder`**: exported methods before unexported within a struct
- **`exhaustive`**: every switch on an enum needs an explicit `case` for every value; no falling back to `default` only
- **`noinlineerr`**: linter auto-splits `if err := foo(); err != nil` into two statements; use unique error variable names per call site (`hookErr`, `tcpErr`, …) to prevent the auto-fix from producing redeclarations
- **Unnamed receivers**: use `func (*Type) Method()` for stateless methods — satisfies both `staticcheck ST1006` (no `_` receiver) and `revive unused-receiver` at once
