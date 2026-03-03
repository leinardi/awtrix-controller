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

## WP Status

- [x] WP-01 — Module Manifest Reset
- [x] WP-02 — Data Models (`internal/model/`)
- [x] WP-03 — Logger + Clock (`internal/logger/`, `internal/clock/`)
- [x] WP-04 — Configuration (`internal/config/`)
- [ ] WP-05 — Client State Registry (`internal/clientstate/`)
- [ ] WP-06 — Scheduler (`internal/scheduler/`)
- [ ] WP-07 — Day/Night Mode (`internal/daynight/`)
- [ ] WP-08 — Energy-Saving Mode (`internal/energysaving/`)
- [ ] WP-09 — Notifications (`internal/notification/`)
- [ ] WP-10 — Settings Composer + MQTT Broker (`internal/settings/`, `internal/broker/`)
- [ ] WP-11 — Entry Point + Wiring (`cmd/awtrix-controller/`)
