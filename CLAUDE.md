# awtrix-controller

**awtrix-controller** is a Go application that embeds an MQTT broker to manage one or more
[Awtrix3](https://blueforcer.github.io/awtrix-light/) LED matrix displays. It has **no HTTP
interface** — all communication with devices is via MQTT.

---

## Reference Documents

| File             | Role                                                                                                                                                                                                                                                                                        |
|------------------|---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `SPEC.md`        | **Authoritative behavioral specification.** What to build: MQTT topic contract, data models, all feature behaviors (Day/Night, Energy-Saving, Notifications, Client State, Button Events), CLI interface, exit codes, and 21 acceptance test cases (TC-01–TC-21). Treat as ground truth.    |
| `CONVENTIONS.md` | **How to write the code.** Go style rules: error wrapping, logging with `slog`, context usage, concurrency patterns (`sync.WaitGroup`, `sync.RWMutex`, `sync/atomic`), CLI flag registration, dependency injection, import grouping, linter config. Follow unless explicitly adapted below. |
| `PLAN.md`        | **Incremental implementation plan.** 11 work packages (WPs) in dependency order. Each WP lists exact files, dependencies, and the test cases it satisfies. Follow this order.                                                                                                               |

---

## Implementation Rules (Every Session)

1. **Follow the WP order.** The dependency graph is:

   ```
   WP-01 → WP-02, WP-03
              WP-03 → WP-04 → WP-05, WP-06 → WP-07, WP-08, WP-09
              WP-03 → WP-10 → WP-11
   ```

   Do not start a WP until all its dependencies are complete and passing.

2. **Tests ship with the code.** Every WP includes its own tests. Never defer tests to a
   later WP.

3. **Always run with the race detector.**

   ```
   go test -race ./...
   ```

   A WP is not complete until `go test -race` is green for the new packages.

4. **Use context7 for library docs.** Before writing code that uses `comqtt`, `go-sunrise`,
   `gopkg.in/yaml.v3`, or any other third-party library, resolve the library via context7
   (`mcp__context7__resolve-library-id`) and query its docs (`mcp__context7__query-docs`).
   Do not guess APIs.

5. **MIT license header on every `.go` file.** The header is in `cmd/awtrix-controller/version.go`;
   copy it verbatim.

6. **Every exported symbol has a doc comment** starting with the symbol name.

7. **No magic numbers.** Named constants in a `defaults.go` or `constants.go` file per package
   for every numeric literal that requires explanation or appears more than once.

8. **`main()` calls `os.Exit(run())` only.** All logic lives in `run() int`. Deferred cleanup
   in `run()` must execute before exit.

9. **Verify `go build ./...` and `go test -race ./...` are green** before marking a WP done.

---

## Conventions Adapted or Skipped for awtrix-controller

These deviations from `CONVENTIONS.md` were decided during planning and must not be reverted:

| Convention                                   | Adaptation                                                                                                                                                                                                                 |
|----------------------------------------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| **§8 HTTP Handlers & Server**                | **Skip entirely.** The application has no HTTP interface (SPEC §1). No `server` package, no `/metrics` or `/healthz`.                                                                                                      |
| **§9 Prometheus Metrics**                    | **Skip entirely.** No metrics exposition. No `metrics_ids.go`, no `Configure…()` functions, no `Reset()`/`gauge.Delete()` patterns.                                                                                        |
| **§14 Docker client / `containerd/errdefs`** | **Replace.** Use `comqtt` (`github.com/wind-c/comqtt/v2`) for broker; stdlib `errors.Is` for error classification. Remove all Docker imports.                                                                              |
| **`internal/labels` package**                | **Omit.** No Prometheus label sanitization needed; MQTT topics follow a fixed firmware schema.                                                                                                                             |
| **`internal/collector` package**             | **Replace with domain packages:** `broker`, `clientstate`, `daynight`, `energysaving`, `notification`, `scheduler`, `settings`. Same "single responsibility" principle, different names.                                   |
| **CLI flags (§10)**                          | **Dual registration.** SPEC requires short forms (`-c`, `-l`, `-v`). Register both long and short names pointing to the same variable: `flag.StringVar(&v, "config", ...)` AND `flag.StringVar(&v, "c", ...)`. No `pflag`. |
| **Log handler**                              | **JSON by default.** Use `slog.NewJSONHandler(os.Stdout, ...)` as the default (SPEC §9 recommends structured/JSON). The `plain_handler.go` pattern from the reference project is not needed.                               |
| **`go.mod` Go version**                      | Use the latest **released** stable Go version. The inherited `go.mod` listed `go 1.25.2` (a placeholder); replace it with an actual version.                                                                               |

---

## Current Implementation Status

- [x] **WP-01** — Module Manifest Reset (`go.mod`, `go.sum`)
- [ ] **WP-02** — Data Models (`internal/model/`)
- [ ] **WP-03** — Logger + Clock (`internal/logger/`, `internal/clock/`)
- [ ] **WP-04** — Configuration (`internal/config/`)
- [ ] **WP-05** — Client State Registry (`internal/clientstate/`)
- [ ] **WP-06** — Scheduler (`internal/scheduler/`)
- [ ] **WP-07** — Day/Night Mode (`internal/daynight/`)
- [ ] **WP-08** — Energy-Saving Mode (`internal/energysaving/`)
- [ ] **WP-09** — Notifications (`internal/notification/`)
- [ ] **WP-10** — Settings Composer + MQTT Broker (`internal/settings/`, `internal/broker/`)
- [ ] **WP-11** — Entry Point + Wiring (`cmd/awtrix-controller/main.go`, `run.go`)

Update this list by replacing `[ ]` with `[x]` as each WP is completed.

---

## Known Gotchas and Decisions

### Data Models

- `Settings.BRI` must be `*int` (pointer), not `int`. When energy saving is inactive, `BRI`
  must be **absent** from the JSON payload — `omitempty` only omits nil pointers, not zero
  integers. Same for `ABRI *bool`.
- JSON field names in `Settings` are **uppercase** (`CHCOL`, `BRI`, `ABRI`, etc.) matching the
  Awtrix3 firmware exactly. `Stats` fields are lowercase (`app`, `bat`, `bri`, etc.).

### Injectable Clock

- Every time-dependent package constructor takes `clk clock.Clock` as a parameter.
  `clock.RealClock` for production; `clock.FakeClock` for tests.
- The `go-sunrise` library takes a `time.Time` argument directly, so passing `clk.Now()` is
  sufficient — no additional sunrise function mocking needed (use a real polar lat/lon +
  winter date to trigger the polar fallback; or inject a `sunriseFunc` param on `daynight.Controller`).

### Scheduler Testability

- The `Scheduler` accepts an injectable `timerFactory func(d time.Duration, f func()) *time.Timer`.
  In tests, pass a factory that fires immediately (when `d == 0`, by setting the fake clock
  past the fire time before calling `Schedule`). This avoids `time.Sleep` in tests.

### Settings Push

- Settings are pushed **partially** — only the fields the application manages. Use `omitempty`
  on all `Settings` struct fields. Do not send a full settings object.
- Fields always sent: `CHCOL`, `CBCOL`, `WDCA`, `WDCI`, `TIME_COL`, `DATE_COL`.
- Fields conditionally sent: `BRI=1` + `ABRI=false` (energy saving active only);
  `ABRI=true` (energy saving inactive, no `BRI`).
- `OnConnect` hook calls `pushSettings(clientID)` in a **goroutine** (with panic recovery)
  to avoid blocking the MQTT CONNECT response. The retained message ensures the device
  receives settings on subscription regardless of timing.

### Notification Fan-Out

- Birthday and New Year notifications are fanned out by iterating `registry.ConnectedIDs()`
  and publishing one `{clientID}/notify` per device. The `clients` field in the `Notification`
  payload is left **empty** — it is a device-side firmware field, not the application's
  fan-out mechanism.

### Polar Night / Midnight Sun

- If `sunriseFunc` cannot compute sunrise (polar night), stay in Night mode; log `Warn` (not
  Error); schedule a retry at midnight of the **next calendar day** in the configured timezone.
  No spin-retry loop.
- Same pattern for missing sunset (midnight sun) → stay Day, retry next day.

### Birthday Age and New Year Year

- Both are computed **at the moment the alarm fires**, not at config load time or startup.
  This ensures correctness when the app runs continuously across year boundaries.
- Age = `fired.In(tz).Year() - birthYear`
- New Year year = `fired.In(tz).Year()` (the year being welcomed in)

### CLI Flag Precedence

- Resolution order (highest to lowest): CLI flag → env var → built-in default.
- Implementation: after `flag.Parse()`, call `flag.Visit` to collect explicitly-set flag names.
  For each flag **not** in that set, check its corresponding env var; if non-empty, override.
- Env vars: `AWTRIX_CONFIG` → `--config`, `AWTRIX_LOG_LEVEL` → `--log-level`.

### Exit Codes

- `0` — clean shutdown (SIGTERM/SIGINT)
- `1` — configuration error (file absent, invalid YAML, missing required field, invalid color,
  invalid IANA timezone)
- `2` — runtime startup error (MQTT port already in use)

### Graceful Shutdown Order

1. `scheduler.Stop()` — drains in-flight timers
2. `mqttServer.Close()` — disconnects clients (triggers `OnDisconnect` → `registry.Unregister`)
3. Log "shutdown complete"
4. Return `0`

### Dockerfile and docker-compose

- `deployments/docker/Dockerfile`: change `EXPOSE 8888` → `EXPOSE 1883`; update description
  label to "Awtrix3 MQTT Controller"; update license label to `MIT`.
- `deployments/docker/docker-compose.yaml`: port `1883:1883`; add config volume mount at
  `/etc/awtrix-controller/config.yaml`; remove the Docker socket options (not needed).

### go.mod

- The current `go.mod` contains Docker, Prometheus, and OpenTelemetry imports — all wrong.
  WP-01 replaces it entirely. Direct dependencies are only: `comqtt/v2`, `go-sunrise`,
  `gopkg.in/yaml.v3`.
