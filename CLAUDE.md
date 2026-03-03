# awtrix-controller

Go app embedding an MQTT broker for [Awtrix3](https://blueforcer.github.io/awtrix-light/) displays. No HTTP interface.

## Reference Documents

| File             | Role                                                                           |
|------------------|--------------------------------------------------------------------------------|
| `SPEC.md`        | Authoritative behavioral spec (MQTT topics, features, exit codes, TC-01–TC-21) |
| `PLAN.md`        | 11 work packages in dependency order — follow this sequence                    |
| `CONVENTIONS.md` | Go style rules — follow unless overridden below                                |

## Session Rules

1. Follow WP order from `PLAN.md`; do not start a WP until its dependencies are complete and passing.
2. Tests ship with the code — never defer to a later WP.
3. Use context7 (`mcp__context7__resolve-library-id` + `mcp__context7__query-docs`) before writing code against any third-party library.
4. MIT license header on every `.go` file (copy from `cmd/awtrix-controller/version.go`).
5. Run `make check-stage` after staging any project file changes; fix all linter issues before marking a WP done.
6. Verify `go build ./...` and `go test -race ./...` are green before marking a WP done.

## Conventions Overrides

- **No HTTP, no Prometheus** — skip CONVENTIONS.md §8 and §9 entirely.
- **Broker**: `comqtt` (`github.com/wind-c/comqtt/v2`); stdlib `errors.Is` instead of `containerd/errdefs`.
- **Internal packages**: `broker`, `clientstate`, `daynight`, `energysaving`, `notification`, `scheduler`, `settings` (no `collector`, no `labels`).
- **CLI flags**: dual-register long+short with stdlib `flag` (no pflag). Env-var fallback via `flag.Visit` after `flag.Parse()`.
- **Logger**: `slog.NewJSONHandler(os.Stdout, ...)` by default; no `plain_handler.go`.
- **`main()`**: calls `os.Exit(run())` only; all logic and deferred cleanup in `run() int`.

## Common Linter Violations to Avoid

### `varnamelen` — variable names too short for scope

Use descriptive names whenever a variable is used across more than ~3 lines or in a large function.

- `r` → `reg` (registry), `cfg` is fine in short constructors but not across a 30-line function
- `cs` → `state`, `n` → `concurrency`, `wg` → `waitGroup`, `i` → `idx`, `id` → `clientID`
- Short names (`ok`, `err`, `i`) are fine in truly tight scopes (2–3 lines).

### `builtinShadow` (gocritic) — shadowing a predeclared identifier

Never use Go builtin names as local variable names: `copy`, `len`, `cap`, `new`, `make`, `close`, `delete`, `append`, `error`, `panic`, `print`, `real`, `imag`, `complex`.

- `copy := *cs` → `snap := *state`

### `paralleltest` — test functions must call `t.Parallel()`

Every top-level `Test*` function must begin with `t.Parallel()`. This also satisfies the `unused-parameter` check for `t`.

```go
func TestFoo(t *testing.T) {
    t.Parallel()
    // ...
}
```

### `unused-parameter` (revive) — `t *testing.T` appears unused

Caused by test functions that never call any `t.*` method. Adding `t.Parallel()` (see above) resolves this.

## WP Status

- [x] WP-01 — Module Manifest Reset
- [x] WP-02 — Data Models (`internal/model/`)
- [x] WP-03 — Logger + Clock (`internal/logger/`, `internal/clock/`)
- [x] WP-04 — Configuration (`internal/config/`)
- [x] WP-05 — Client State Registry (`internal/clientstate/`)
- [ ] WP-06 — Scheduler (`internal/scheduler/`)
- [ ] WP-07 — Day/Night Mode (`internal/daynight/`)
- [ ] WP-08 — Energy-Saving Mode (`internal/energysaving/`)
- [ ] WP-09 — Notifications (`internal/notification/`)
- [ ] WP-10 — Settings Composer + MQTT Broker (`internal/settings/`, `internal/broker/`)
- [ ] WP-11 — Entry Point + Wiring (`cmd/awtrix-controller/`)
