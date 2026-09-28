---
name: go-style-guide
description: >
  Project Go style rules enforced by golangci-lint v2 (all linters) + pre-commit, the
  maintainer's own rules (descriptive names, no builtin shadowing, t.Parallel, method
  order, exhaustive enum switches, no inline errors, unnamed stateless receivers), and
  this controller's patterns (clock and scheduler injection, timezones, MQTT publishing,
  concurrency). Apply when writing, editing, or reviewing any .go file in this repo.
  Consult before generating Go code, not after lint fails.
---

# Go Style Guide — awtrix-controller

Rules derived from `.golangci.yaml` (golangci-lint v2, `default: all`) and verified
against the existing codebase in `cmd/` and `internal/`. §1–§15 are what the linters
enforce, §16 are the maintainer's rules, §17–§19 are review rules, and §20–§23 are this
controller's own patterns.

golangci-lint is not on `PATH` in every environment; run it through pre-commit
(`pre-commit run golangci-lint-full --all-files`). Never commit with `--no-verify` —
fix the underlying issue instead.

Linters that are **disabled** in `.golangci.yaml`, so their rules do not apply:

| Disabled linter | Reason |
| --- | --- |
| `exhaustruct`, `exhaustruct_v5` | Requires every struct field to be set; too noisy for short-lived structs |
| `gomodguard` | Replaced by `gomodguard_v2`, which is enabled |
| `gochecknoglobals` | Package-level state is allowed (`newServerMu`, the logger singleton) |
| `nonamedreturns` | Named returns are allowed (`SunriseFunc`'s `rise, set, ok`) |
| `wsl` | The deprecated v4 linter; its successor `wsl_v5` stays enabled |

Beyond the generic test-file exclusions (§4, §5, §8, §11, §13, §15), three are specific to
this repo: `paralleltest` for `internal/logger/logger_test.go`, `tagliatelle` for
`internal/config/config.go` (YAML keys are snake_case on purpose) and `gosec` G117 for the
same file (the MQTT password field is read from the user's config, not hardcoded).

The formatters (`gci`, `gofmt`, `gofumpt`, `goimports`, `golines`) run in the
`golangci-lint-fmt` hook and in CI.

---

## 1. Import grouping

Three groups, separated by blank lines — enforced by both `gci` (explicit
`sections: standard, default, prefix(github.com/leinardi/awtrix-controller)`) and
`goimports` (`local-prefixes: github.com/leinardi/awtrix-controller`) simultaneously, and
they must agree. From `internal/daynight/daynight.go`:

```go
import (
    "sync"
    "time"

    solar "github.com/mstephenholl/go-solar"

    "github.com/leinardi/awtrix-controller/internal/clock"
    "github.com/leinardi/awtrix-controller/internal/config"
    "github.com/leinardi/awtrix-controller/internal/logger"
    "github.com/leinardi/awtrix-controller/internal/scheduler"
)
```

comqtt is always imported as `mqtt "github.com/wind-c/comqtt/v2/mqtt"`; the paho client in
the broker tests as `paho`.

---

## 2. Error handling

### 2a. No inline error assignment in `if` (`noinlineerr`)

Separate statements, and a name unique to the call (§16). From `broker.New`:

```go
// Wrong
if err := server.AddHook(hook, nil); err != nil {

// Right
hookErr := server.AddHook(hook, nil)
if hookErr != nil {
    return nil, fmt.Errorf("broker: add controller hook: %w", hookErr)
}

tcpErr := server.AddListener(tcpListener)
```

### 2b. Wrap errors with `%w` (`errorlint`)

Wrap with a short lowercase prefix naming the operation; most packages outside `config` start
it with the package name (`"broker: add TCP listener on %s: %w"`, `"settings: publish to %s: %w"`,
`"weather: decode response: %w"`). Compare with `errors.Is`/`errors.As` (or Go 1.26's
`errors.AsType[T]`), never `==`. Prefer early returns; no `else` after a `return`
(`revive`'s `indent-error-flow`).

### 2c. Errors are sentinels, detail is wrapped (`err113`)

`err113` flags every `errors.New` inside a function body and every `fmt.Errorf` without a
`%w` verb. Declare a package-level sentinel and wrap the detail:

```go
return nil, fmt.Errorf("weather: HTTP %d: %w", response.StatusCode, errHTTPError)
return fmt.Errorf("calendar_accent %q: %w", colors.CalendarAccent, ErrInvalidHexColor)
```

Export it (`ErrMQTTUsernameRequired`) when callers need `errors.Is`. The
`//nolint:err113 // dynamic message includes …` directives in `internal/model/draw.go`
predate this rule: dynamic detail is exactly what wrapping a sentinel carries, so do not
copy them.

### 2d. Aggregating multiple errors

Use `errors.Join` over wrapped errors. No code here aggregates today: `config.Validate`
returns the first violation.

### 2e. Ignoring errors explicitly

The `std-error-handling` exclusion preset already accepts an unchecked `Close`, `Flush` or
print (`defer response.Body.Close()` in `weather.Fetch`). Anything else that genuinely
cannot be acted upon is assigned to `_`.

---

## 3. `nolint` directives

`nolintlint` enforces three things:

- **Specific**: name every linter — no bare `//nolint`
- **Explanation required**: every directive needs `// reason`
- **No unused**: remove directives when the code no longer triggers that linter

The explanation says why the fix does not apply here, not which rule fired (§18).

```go
// Inline, for a statement:
timezone = time.Local //nolint:gosmopolitan // intentional system-TZ fallback per SPEC §7

// Preceding line, for a declaration:
//nolint:gocritic // hugeParam: packets.Packet value type is required by the mqtt.Hook interface
func (h *ControllerHook) OnConnectAuthenticate(_ *mqtt.Client, packet packets.Packet) bool {

// Several linters, comma-separated, no spaces:
//nolint:cyclop,funlen,gocognit,gocyclo,maintidx // startup wiring is inherently long; splitting would obscure the sequence
```

`nolintlint` cannot see a directive for a disabled linter, so it never reports one as
unused: do not write them (`//nolint:gochecknoglobals` on `newServerMu` is one).

---

## 4. Complexity limits

| Linter | Threshold | Note |
| --- | --- | --- |
| `gocyclo` | 15 | Cyclomatic complexity |
| `cyclop` | 15 | Same metric, different linter — both fire together |
| `gocognit` | 35 | Cognitive complexity |
| `funlen` | 50 statements | Lines are disabled (`lines: -1`) |

Prefer extracting helpers over suppressing. Test files are exempt from `funlen`,
`gocognit`, `gocyclo`, `maintidx` and the `cyclop` "calculated cyclomatic complexity" check.

---

## 5. Magic numbers (`mnd`)

Numbers 0, 1, 2, 3 are allowed everywhere. Any other literal in an `argument`, `case`,
`condition`, or `return` position needs a named constant; so does any literal used more than
once or that needs explaining. Config defaults live in `internal/config/defaults.go`
(`DefaultMQTTPort = 1883`, `DefaultWeatherPollIntervalMinutes = 15`, …), protocol values next
to their use (`wmoThunderstorm = 95`, `fetchTimeout = 10 * time.Second`, `daysInWeek = 7`).

`strings.SplitN` is excluded from mnd checks. Test files are fully exempt from `mnd`.

---

## 6. Type aliases

Use `any` instead of `interface{}`; `gofmt`'s rewrite rule changes it anyway.

---

## 7. Struct size (`gocritic hugeParam`)

Structs over ~80 bytes passed by value trigger `hugeParam`. Pass by pointer — or suppress
with the reason when an interface fixes the signature (the comqtt hook methods take
`packets.Packet` by value). The same applies to `rangeValCopy`: iterate large slices by index
(`for entryIdx := range entries`, then `&entries[entryIdx]`).

---

## 8. Line length (`lll`)

Max 140 characters. `golines` wraps automatically. Test files are exempt.

---

## 9. Forbidden packages (`depguard`)

| Forbidden | Use instead |
| --- | --- |
| `github.com/sirupsen/logrus` (rule `logger`; allowed only in `internal/logger`) | `github.com/leinardi/awtrix-controller/internal/logger` (`logger.L()`, backed by `log/slog`) |
| `github.com/pkg/errors` (rule `forbidden-forks`) | stdlib `errors` + `fmt.Errorf(...%w...)` |
| `github.com/instana/testify` (rule `forbidden-forks`) | `github.com/stretchr/testify` |

---

## 10. Comments and `godox`

- `FIXME` is flagged by `godox`. `TODO` is allowed.
- gocritic's `whyNoLint` check is disabled, but every `//nolint` still needs an explanation.
- Every exported symbol has a doc comment beginning with its name, and every package has a
  package comment (`// Package clock defines the Clock interface …`). Keep it that way.
- Large files are divided with `// --- Section name ---` separators (`// --- validation
  helpers ---`, `// --- RealClock ---`).
- Do not add comments to code you did not otherwise change.

---

## 11. Duplication (`dupl`)

Avoid copy-pasting blocks longer than ~100 tokens (`threshold: 100`). Test files are exempt.

---

## 12. Shadowing (`govet shadow`)

`govet` shadow detection is enabled. Errors are named after their source (§16):
`hookErr`, `tcpErr`, `wsErr`, `dnStartErr`, `esStartErr`, `pushErr`, `publishErr`.

---

## 13. Variable naming (`varnamelen`, `predeclared`)

`varnamelen` flags a name shorter than 3 characters whose last use is more than 5 lines from
its declaration (defaults: `min-name-length: 3`, `max-distance: 5`); test files are exempt.
Receivers are exempt (`(ctrl *Controller)`, `(sched *Scheduler)`, `(h *ControllerHook)`).
`predeclared` flags any name that shadows a builtin. §16 goes further than both.

```go
// Wrong (energysaving.parseHHMM today; rename it when you touch it)
func parseHHMM(s string) (time.Duration, error)

// Right
func parseWeekday(weekdayStr string) (time.Weekday, error)
```

---

## 14. `modernize` — no pointer-boxing helpers

The `modernize` linter (`newexpr` check) flags any function whose sole purpose is to return a
pointer to its argument — the generic `func ptr[T any](v T) *T` included. Go 1.26's `new`
takes an expression:

```go
if entry.Enabled == nil {
    entry.Enabled = new(true)
}
```

Taking the address of a local is fine too (`result.Bri = &brightnessValue` in `settings.Build`).

---

## 15. Constant strings (`goconst`)

String literals appearing 3+ times with length ≥ 2 become a named constant. Test files are
exempt. Examples: the icon IDs (`iconThunderstorm`, …) in `internal/weather/controller.go`,
the defaults in `internal/config/defaults.go`.

---

## 16. Maintainer rules

These hold for all Go code here, tests included. Where a linter enforces one it is named;
the rest are review rules.

| Rule | Enforced by |
| --- | --- |
| Descriptive names | review (`varnamelen` only catches the long-distance case) |
| No builtin shadowing | `predeclared` |
| `t.Parallel()` first in every top-level `Test*` | review: `paralleltest` is excluded for every `_test.go`; `tparallel` checks subtests |
| Exported methods before unexported, per type | `funcorder` (within a file) |
| An explicit `case` for every enum value | `exhaustive` (a `default` alone does not count) |
| No inline `if err := …; err != nil` | `noinlineerr` |
| Stateless methods have an unnamed receiver | `revive` `unused-receiver` catches a named unused one |

- **Descriptive names.** Name a variable for what it holds, at any distance: `index`,
  `state`, `waitGroup`, `timezone`, `clientID` — not `i`, `s`, `wg`, `tz`, `id`. Idiomatic
  `ok`, `ctx` and receivers are fine.
- **Never shadow a builtin** as a variable or parameter: `copy`, `len`, `cap`, `new`,
  `close`, `delete`, `append`, `error`, and the rest of the predeclared set (`min`, `max`,
  `clear`, …).
- **`t.Parallel()` is the first statement of every top-level `Test*`.** The one exception is
  `internal/logger/logger_test.go`, whose tests swap the global logger with `logger.Set`; any
  other test that must touch process-global state says so in a comment instead of silently
  skipping `t.Parallel()`.
- **Exported methods first.** For each type, exported methods come before unexported ones
  (`daynight.Controller`: `Start`, `CurrentMode`, then `onTransition`, `reschedule`,
  `nextEventAfter`). `funcorder` only sees one file at a time; across files it is on review.
- **Enum switches name every value.** A switch on an enum type (`daynight.Mode`,
  `weather.EventType`, the `model` enums) has a `case` for each constant, as `iconForEvent`
  and `isNotifyEnabled` do; a fallback after the switch is allowed, a `default` instead of
  cases is not. A new constant means a new `case` in every such switch.
- **Unique error names**, never an inline error (§2a): `hookErr`, `tcpErr`, `parseErr`.
- **Stateless methods use an unnamed receiver**, not `_`: `func (*ControllerHook) ID() string`,
  `func (*StateManager) OnFetchFailure() {}`, `func (RealClock) Now() time.Time`.

---

## 17. Reuse before writing

| Need | Use | Not |
| --- | --- | --- |
| A logger | `logger.L()`; `logger.Init` in `run`, `logger.Set` only in `internal/logger` tests | `slog.Default()`, a package-level `slog.New`, a logger parameter |
| The current time | the injected `clock.Clock`; `clock.NewFakeClock` in tests | `time.Now()` in a controller |
| A timer, one-shot or recurring | `scheduler.Scheduler.Schedule` with a `reschedule` func | `time.AfterFunc`, a ticker or a sleeping goroutine |
| The next wall-clock occurrence | `nextDailyOccurrence`, `nextWeeklyOccurrence`, `nextMonthlyOccurrence`, `nextYearlyOccurrence` (`internal/notification`), `nextMidnight` (`internal/daynight`) | a second date calculation |
| The devices to send to | `registry.ConnectedIDs()` (a sorted copy) | iterating comqtt's clients |
| The settings payload | `settings.Builder.Build()`, published with `settings.Push` | assembling `model.Settings` at the call site |
| A notification to one device | the `publishNotification` closure in `run.go` (`{clientID}/notify`, QoS 1, not retained) | a second publish path |
| Notification text | `model.NewPlainText` | a raw string in `model.AppContent.Text` |
| A weekday name | `parseWeekday` | a second switch |
| A `#RRGGBB` check | `isValidHexColor` (`internal/config`) | a regexp |
| A scheduler in a test | `scheduler.NewWithFactory` with the package's fake factory (`makeFakeFactory`, `makeControllableFactory`, `makeCapturingFactory`, `immediateFactory`) | real timers; and not a new copy of a factory the package already has |
| Waiting in a test | `waitForCondition` (`internal/weather`), `waitForBroker` (`internal/broker`) | `time.Sleep` or another deadline loop (§19) |
| A broker in a test | `startBroker`, `pahoConnect`, `freePort` (`internal/broker/broker_test.go`) | a per-test setup |

---

## 18. Comments carry rationale; history goes in the commit

A comment says **why the code is the way it is**; history belongs in the commit body.

```go
// Bad — history in the code.
// Added the pointer check after the reconnect bug report.

// Good — rationale in the code (ControllerHook.OnDisconnect).
// The unregister is guarded by a pointer check: if the client reconnected before
// this disconnect callback fired (comqtt calls OnConnect for the new session
// before OnDisconnect for the expired old one), the activeClients map already
// holds the new session's pointer.
```

The same rule makes `//nolint` explanations useful: say why the fix does not apply here.

---

## 19. Waiting in tests: classify before you write a sleep

Controllers take a `clock.Clock` and a `scheduler.TimerFactory` so tests control time
without sleeping; use them. When a test still has to wait, decide the class first:

- **Positive eventual — never a sleep.** Wait on a channel against a timeout, or poll with
  `waitForCondition`; fail naming what never happened. A hand-rolled deadline loop in a test
  body is this class too.
- **Negative assertion — bounded and commented.** Give the wrong behavior a bounded window,
  then assert it did not appear, and say so in a comment (`TestBrokerOnDeviceReadyFiredOnce`:
  "Give the second publish time to (incorrectly) trigger a second callback").
- **Real elapsed window — the duration is the point.** Name it as a constant.
- **Ordering barrier with no quiescence signal** — say why no seam exists.
- **Poll tick inside a wait helper** (`waitForCondition`, `waitForBroker`) — already correct.

Known follow-up, not to be added to: the hand-rolled deadline loops in
`TestBrokerDisconnect` and `TestBrokerStaleDisconnectDoesNotUnregister`; the cleanup sleep in
`startBroker`, where `Serve` returning is observable; and the 10–20 ms sleeps in
`TestControllerPollFetchError`, `TestControllerOnDeviceConnectedStale` and
`TestControllerOnDeviceConnectedNoPendingEvents`, which do not say which class they are.

---

## 20. Project layout, naming, license header

```text
awtrix-controller/
├── cmd/awtrix-controller/    # main.go, run.go (numbered wiring steps), testnotification.go, version.go
├── internal/
│   ├── broker/               # comqtt server, ControllerHook (auth, ACL, client lifecycle, topic routing)
│   ├── clientstate/          # Registry of connected clients
│   ├── clock/                # Clock, RealClock, FakeClock
│   ├── config/               # YAML config, Validate, defaults.go
│   ├── daynight/, energysaving/   # mode controllers driven by the scheduler
│   ├── logger/               # slog singleton
│   ├── model/                # Awtrix3 JSON payloads and enums (firmware keys)
│   ├── notification/         # scheduled notifications
│   ├── scheduler/            # recurring jobs over an injectable timer factory
│   ├── settings/             # settings payload builder and Push
│   └── weather/              # Open-Meteo fetch, event detection, overlay, notifications
└── deployments/              # config.sample.yaml, docker/
```

- `cmd/` wires; all behavior is in `internal/`. A controller exposes `New` (production) and a
  `New…With…` constructor for tests (`NewWithSunriseFunc`, `NewWithFetchFunc`,
  `NewWithFactory`).
- Every `.go` file starts with the MIT license block (`Copyright (c) 2026 Roberto
  Leinardi`), then the package comment and `package` line.

---

## 21. Logging

- `log/slog` only, through `logger.L()`; `run` calls `logger.Init(level)` once.
- Structured key/value fields, never `fmt.Sprintf` in a log call:
  `logger.L().Warn("failed to push settings to client", "clientID", clientID, "err", pushErr)`.
- Messages are prefixed with the package (`"broker: client connected"`, `"weather: fetch
  failed; clearing overlay"`). The client key is `"clientID"` (the scheduled notifier's
  `"client_id"` is the odd one out; do not copy it).
- Never log the MQTT password: `config.NewConfigDebugView` omits it on purpose.
- There is no fatal level: `runWithContext` returns an exit code (1 config, 2 startup).

---

## 22. Time, timezones and scheduling

- Controllers read time from their `clock.Clock` and arm timers only through
  `scheduler.Scheduler`; `Scheduler.Stop` cancels pending timers and waits for in-flight
  callbacks, so a timer outside it escapes shutdown.
- Compute fire times in the configured `*time.Location` and build them with `time.Date(…,
  hour, minute, …, loc)`, as the `next…Occurrence` helpers do; the result is strictly after
  `from`. Adding a `time.Duration` to local midnight (`energysaving.nextOccurrence`) is off
  by an hour on DST-change days; do not copy it.
- `time.Date` normalizes overflow: day 31 in a 30-day month or `02-29` in a non-leap year
  lands in the next month. Code that relies on it says so.

---

## 23. Concurrency

- Shared state sits behind a `sync.Mutex`/`RWMutex` with the unlock deferred or on the next
  line, and readers get copies (`Registry.ConnectedIDs`; `Registry.Snapshot` is shallow, so the
  `*model.Stats` it returns is replaced on update, never mutated in place).
- A comqtt hook never blocks the broker: work triggered by a connect or a first `/stats`
  runs in a goroutine (`OnConnect`'s settings push recovers panics and logs them).
- `weather.StateManager` is not goroutine-safe; only the scheduled `poll` calls `Process`.
- `newServerMu` serializes `mqtt.New`, which races in comqtt when called concurrently (the
  broker tests do); keep every `mqtt.New` behind it.

---

## What to avoid

- logrus or `pkg/errors` (§9).
- `log.Fatal` or `os.Exit` outside `main`: `main` calls `os.Exit(run())` once.
- `time.Now()`, `time.AfterFunc` or `time.Sleep` in controller code (§17, §22).
- A short or builtin-shadowing name (§16).
- `interface{}` (§6) and pointer-boxing helpers (§14).
- A `//nolint` for a disabled linter, or one whose reason restates the rule (§3).
- Designing for hypothetical requirements: no configurability or abstractions for features
  that do not exist yet.
- Skipping or suppressing pre-commit hooks (`--no-verify`).
- Adding comments to code you did not change (§10).

---

## Quick checklist before submitting Go code

- [ ] Imports in 3 groups: stdlib / third-party / local, alphabetical within each
- [ ] No `if err := f(); err != nil`; each error has a name unique to its source
- [ ] All errors wrapped with `%w`; no `errors.New` or `%w`-less `fmt.Errorf` in a function body
- [ ] Descriptive names everywhere; no builtin shadowed
- [ ] `t.Parallel()` first in every top-level `Test*` (logger tests excepted)
- [ ] Exported methods before unexported ones for each type
- [ ] A `case` for every value in each enum switch
- [ ] Stateless methods use an unnamed receiver
- [ ] `any` not `interface{}`; `new(expr)`, not a pointer-boxing helper
- [ ] Numbers other than 0–3 extracted to named constants (non-test code)
- [ ] Each `//nolint` names specific linters and explains why the fix does not apply
- [ ] Function statement count ≤ 50 (non-test code); no `FIXME`
- [ ] Checked §17 for an existing helper; time from `clock.Clock`, timers from the scheduler
- [ ] Fire times built with `time.Date` in the configured location
- [ ] Comments say why, not what changed (§18); no guessed `time.Sleep` in tests (§19)
