# awtrix-controller — core

Self-contained MQTT broker + automation controller for Awtrix3 LED clocks. Devices connect to the
embedded broker; the controller pushes day/night themes and settings, runs an energy-saving window,
fires scheduled notifications and overlays the Open-Meteo forecast.

AGENTS.md (repo root) is the authoritative contract: layout, commands, validation matrix, coding
rules, commit types. Read it before any change; memories below only add what it does not say.

## Source map

- `cmd/awtrix-controller/` — flags, wiring (`run.go`: `run()` + testable `runWithContext(ctx, args,
  getenv)`), debug-only test-notification mode.
- `internal/broker/` — embedded comqtt broker; `ControllerHook` (auth, connect/disconnect, publish
  routing). `mqtt.New()` is serialised by a package mutex (comqtt data race on shared defaults).
- `internal/clientstate/` registry; `internal/config/` YAML + defaults + validation;
  `internal/daynight/`, `internal/energysaving/`; `internal/settings/`, `internal/notification/`,
  `internal/weather/` (what is published); `internal/scheduler/` + `internal/clock/` (injectable
  time); `internal/model/` Awtrix JSON payloads; `internal/logger/` slog singleton.
- `test/` — repo meta-tests run by plain `go test ./...` (toolchain pins, Dockerfile, docs links,
  license headers, suppression ratchet).
- User-facing contract: README + `deployments/config.sample.yaml`.

## Invariants

- Device identity = MQTT client ID; settings are published retained QoS 1 to `{clientID}/settings`.
- A disconnect only unregisters a client if it belongs to the current session (reconnect race guard
  in the hook).
- Time for scheduling/windows comes from `internal/clock`, never `time.Now` (only
  `internal/weather/simulate.go` reads the wall clock).

Further: tooling and versions in `mem:tech_stack`; commands in `mem:suggested_commands`; Go style
and lint traps in `mem:conventions`; done-criteria in `mem:task_completion`.
