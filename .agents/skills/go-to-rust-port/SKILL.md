---
name: go-to-rust-port
description: >
  Hard-won knowledge for porting, rewriting or migrating a Go project (or a Go package) to Rust:
  semantic traps a straight port silently changes, Go library to Rust crate choices with their
  pitfalls, concurrency and shutdown differences, and tooling, lint and static-packaging traps.
  Use when planning or implementing any part of a Go-to-Rust port, and when reviewing such a port
  for behaviour drift.
---

# Porting Go to Rust

Only knowledge that took a spike, a test, a source read or a failed build to find. Each entry is
**trap → fix → (evidence)**, versioned where the version matters, so it can be re-checked. Not a
Rust tutorial. Seen in: awtrix-controller (a small MQTT broker and automation controller).

## 1. Approach

- Keep the Go code shipping until one breaking cutover commit; until then the Rust code lands
  beside it and the shipped artifact does not change.
- Port leaf modules first, each together with its Go tests. Give every ported test a doc comment
  naming the Go test it replaces (`/// Go: TestX`), and justify every Go test you do not port.
- Capture Go reference values (timestamps, encoded payloads) with a throwaway Go program outside
  the module, rather than reasoning about what Go "probably" returns.
- List every intended behaviour change up front. Any other difference from Go is a defect.
- Use non-release commit types until the cutover, so a release-from-commits tool does not ship a
  half-port.

## 2. Semantic traps

### Time

- Go's `time.Date` normalises out-of-range fields (Feb 31 → Mar 3, month 13 → next January), and
  its DST gap/fold resolution is unspecified → decide clamping and gap/fold handling explicitly,
  route every local→instant conversion through one helper, and golden-test both DST days in two
  zones. (Go docs; spike)
- A time of day stored as "duration since local midnight" is an hour off on DST days → store civil
  wall-clock times and resolve them per date. (spike)
- Daily windows resolved to instants can be **empty** on a DST day: a gap pushes the start past
  the end → derive the active state from instants, and make the next-transition search walk
  forward past empty days; a fixed D-1..D+1 horizon stalls until restart. (plan review)
- jiff 0.2's `Disambiguation::Compatible` (gap → shift forward by the gap, fold → earlier) and its
  refusal of `Timestamp + days` are useful guards → keep calendar arithmetic on civil types and
  convert once. (jiff 0.2 docs, spike)
- `jiff::Zoned::now()` / `Timestamp::now()` ignore tokio's paused clock → keep a `Clock` trait;
  the test implementation is a base timestamp plus tokio's elapsed time. (spike)
- Go's `time.Parse("15:04", "7:00")` succeeds (one-digit hour, two-digit minute only) → a strict
  `%H:%M` parser rejects configs Go accepted; parse leniently. `time.Parse("01-02", …)` needs two
  digits each and parses into year 0, a leap year: `02-29` is valid, `02-30` is not. (spike;
  throwaway Go program)
- jiff 0.2's global time zone database prefers the host's zoneinfo even with
  `tzdb-bundle-always`; a trimmed host copy lacks the `backward` aliases (`US/Eastern`,
  `Europe/Kiev`), and jiff matches names case-insensitively where Go does not → look names up in
  `TimeZoneDatabase::bundled()` and require `iana_name() == Some(name)`; special-case `""`, `UTC`
  and `Local` as `time.LoadLocation` does. (probe on a slim host)
- go-solar computes sunrise/sunset for the **UTC** date of the instant → the `sunrise` crate 3
  matches it within ±1 s; decide deliberately whether to use the local date instead. (spike)

### JSON and YAML

- `encoding/json` escapes `<`, `>` and `&` as `\u003c`, `\u003e`, `\u0026`; serde_json writes
  them literally → byte golden tests differ there; decide and record it. (spike)
- Go decodes JSON `null` into the zero value silently; serde errors on `null` for `f64` → use
  `Vec<Option<T>>` and decide what a missing value means. (spike)
- `encoding/json` also matches object keys **case-insensitively**, treats `null` for a non-pointer
  field as "leave it", and decodes a whole-payload `null` into a struct as a no-op success; a
  derived serde struct does none of these. Duplicate keys resolve last-in-document-wins, and
  `serde_json::Map` is key-sorted, so folding it loses that order → for a payload a device sends,
  collect the entries in order with a `MapAccess` visitor, fold keys to lowercase, then decode
  with `#[serde(default)]` and a null-as-default field helper. (throwaway Go program)
- A Go named string type with constants (`type Effect string`) is **open**: any string converts
  to it and is sent verbatim. A closed Rust enum silently narrows that (user-supplied values get
  rejected) → give it an `Other(String)` variant with hand-written serde, and keep `omitempty`'s
  "empty string is omitted" in the skip predicate. (review of a port)
- A Go interface with a custom `MarshalJSON` maps to an enum (untagged or tuple variants);
  embedded structs map to `#[serde(flatten)]`; `omitempty` maps to `skip_serializing_if`, and a
  field Go always emits (even `null`) must not get one. (spike)
- The "zero means unset, apply default" config pattern makes an explicit `0` impossible → use
  `Option<T>` with defaults applied in one place. (spike)
- yaml.v3 ignores unknown keys; serde does too unless `deny_unknown_fields`. serde_yaml is
  archived → use serde_yaml_ng 0.10, whose errors carry the field path and line. (spike) It
  still parses with `unsafe-libyaml`, which RustSec flags unmaintained (RUSTSEC-2024-0449), so
  cargo-deny fails until it is ignored with a reason; serde-saphyr is the safe-Rust alternative.
  (cargo-deny)
- yaml.v3 is lenient where serde_yaml_ng 0.10 is not: `yes`/`no`/`on`/`off`/`y`/`n` into a bool,
  a float into an int (truncated), `1_000` and `010` (octal) into an int, `null` (also as a whole
  section or a list element) leaving the zero value. serde_yaml_ng does read an unquoted `9597`
  or `true` into a `String` as written. → decode a hand-written config through
  `deserialize_with` visitors that mirror yaml.v3; serde cannot see quoting, so a quoted
  `"1883"` gets accepted where yaml.v3 refused it. (throwaway Go program; serde_yaml_ng probe)

### Logging

- `slog.Level.UnmarshalText` reads a level name in any case plus an optional `+N`/`-N` offset
  (`info+2`), and slog's levels are -4/0/4/8 → port the parser, then map the numeric threshold to
  the tracing filter that passes the same records (`info+1` lets warn through, not info).
  (slog source)

### CLI

- Go's `flag` accepts `-long` as well as `--long`; clap 4 does not → list it as a breaking
  change, or add explicit aliases. (spike)

## 3. Ecosystem map

- **Embedded MQTT broker (comqtt, mochi):**
  - rumqttd 0.20: no disconnect event, no publisher ID on publishes, local links publish at QoS 0
    only → unusable as a controller broker. (spike: failing ported tests)
  - rmqtt 0.24 has comqtt-like hooks, with traps:
    - listeners default to `allow_anonymous(true)`, which accepts a username-less CONNECT
      **before any auth hook runs** → build every listener with `allow_anonymous(false)` through
      one helper. (rmqtt 0.24 source)
    - retained messages need the `rmqtt-retainer` plugin; without it core warns and drops them.
      (spike)
    - each connection is a detached task → run the broker on its own runtime so shutdown can stop
      them. (rmqtt 0.24 source)
    - `kick()` can wait 5 s per session → kick concurrently under one deadline. (rmqtt 0.24
      source)
    - `MqttServer::start` calls `process::exit` on error → use `run()`. (rmqtt 0.24 source)
    - it always pulls QUIC, hence `ring`'s C build; MSRV 1.94. (spike: build)
  - rumqttc 0.25 as a test client keeps its socket open after DISCONNECT until the next poll →
    drop the event loop explicitly in tests that assert on disconnects. (spike)
- **HTTP (net/http):** reqwest's default rustls backend builds `aws-lc-sys` (C), which hurts musl
  cross-builds → ureq 3 with rustls + ring + webpki-roots is lighter (28 vs 86 crates, 3.3 vs
  7.0 MB) and needs no CA files in a static image; run it in `spawn_blocking`. (spike)
- **Logging (slog):** tracing + tracing-subscriber; bridge crates that log through `log` with
  tracing-log. (spike)
- **YAML (yaml.v3):** serde_yaml_ng, see §2.

## 4. Concurrency and shutdown

- Dropping a tokio runtime waits for its `spawn_blocking` tasks, so one blocked HTTP call holds
  the process open → build the runtime explicitly (not `#[tokio::main]`) and end it with
  `shutdown_timeout`. (spike)
- A scoped tracing subscriber (`with_default`) does not reach tasks spawned on a multi-thread
  runtime → assert on logs from a `current_thread` runtime or install the subscriber globally.
  (spike)
- Give shutdown one overall deadline and hand each step only what remains; never a fresh grace
  period per step, never sequential per-session waits. (plan review)

## 5. Tooling, lints and packaging

- **Lints:** golangci-lint maps to clippy: exhaustive → `wildcard_enum_match_arm`, varnamelen →
  `min_ident_chars`, forbidigo → `print_stdout`/`disallowed_methods`, `//nolint` →
  `#[expect(lint, reason = "...")]` with `allow_attributes` and
  `allow_attributes_without_reason` denied. (spike)
- `[lints]` is not inherited by workspace members → declare `[workspace.lints]`, put
  `[lints] workspace = true` in every member (and test that it is there), and run clippy with
  `--workspace`. (spike)
- clippy's `allow-expect-in-tests` / `allow-unwrap-in-tests` cover `#[test]` functions and
  `#[cfg(test)]` modules, **not** plain helper functions in an integration-test file under
  `tests/` → avoid `expect` in those helpers (`unwrap_or_else(|error| panic!(...))`) or move them
  into a `#[cfg(test)]` module. (clippy 1.98: failed build)
- cargo-deny 0.20: an `[advisories] ignore` entry written as `{ crate = "name" }` only ignores
  **yanked** versions of that crate, not its advisories → ignore an unmaintained or vulnerable
  crate by advisory ID (`{ id = "RUSTSEC-…", reason = "…" }`). (cargo-deny: `yanked-not-detected`
  warning)
- **Static builds:** cargo-zigbuild produces static musl binaries for amd64 and arm64; its stock
  image can lag the MSRV. Alpine's distro rust uses host triple `*-alpine-linux-musl` and links
  dynamically; `-C target-feature=+crt-static` breaks proc-macros unless `--target` is passed.
  (spike: failing proc-macro build)
- **Calibration** (one small MQTT controller): Go 12 MB binary and 18 MB RSS; Rust 4.9–6.3 MB
  static binaries and about 5 MB RSS. Clean release builds take 36–110 s. (spike)

Verified against: Rust 1.98, jiff 0.2, rmqtt 0.24, rumqttc 0.25, rumqttd 0.20, serde_yaml_ng
0.10, ureq 3, sunrise 3, clap 4, cargo-deny 0.20.
