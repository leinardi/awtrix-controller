# awtrix-controller — core

Self-contained MQTT broker + automation controller for Awtrix3 LED clocks, in Rust. Devices connect
to the embedded broker; the controller pushes day/night themes and settings, runs an energy-saving
window, fires scheduled notifications and overlays the Open-Meteo forecast.

AGENTS.md (repo root) is the authoritative contract: layout, commands, validation matrix, coding
rules, commit types. Read it before any change; memories below only add what it does not say.

## Source map

- `src/main.rs` (hand-built runtime, shutdown budget), `src/cli/` (clap flags, Go-compatible value
  parsing, test-notification flags), `src/run/` (wiring: `run_with`/`run_with_options`, device
  event queue, signals).
- `src/broker/` — embedded rmqtt broker on its own runtime/thread; `ControllerHook` (auth,
  connect/disconnect by `SessionKey`, publish routing). Listeners only via the `listener` helper
  (`allow_anonymous(false)`).
- `src/clientstate/` registry; `src/config/` YAML (yaml.v3-compatible decoding) + defaults +
  validation; `src/daynight/`, `src/energysaving/`; `src/settings/`, `src/notification/`,
  `src/weather/` (what is published); `src/scheduler/` + `src/clock/` (injectable time,
  `resolve_local`); `src/model/` Awtrix JSON payloads; `src/logger/` tracing subscriber.
- `tests/` integration (broker over sockets, whole run); `meta/` workspace member with the repo
  meta-tests (toolchain pins, Dockerfile, docs links, license headers, suppression ratchet,
  workspace lints).
- User-facing contract: README (incl. "Migrating from 1.x") + `deployments/config.sample.yaml`.

## Invariants

- Device identity = MQTT client ID; settings are published retained QoS 1 to `{client_id}/settings`,
  notifications not retained to `{client_id}/notify`.
- A disconnect only unregisters a client if its session is the current one (takeover race guard).
- Time for scheduling/windows comes from the injected `Clock`; local date + time → instant only via
  `clock::resolve_local` (gap → forward, fold → earlier).

Further: tooling and versions in `mem:tech_stack`; commands in `mem:suggested_commands`; Rust style
and lint traps in `mem:conventions`; done-criteria in `mem:task_completion`.
