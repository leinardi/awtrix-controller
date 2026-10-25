---
name: rust-style-guide
description: >
  Rust coding rules for awtrix-controller: the workspace lint set (clippy pedantic plus the
  restriction lints) and how to satisfy it, #[expect] with a reason as the only suppression,
  the maintainer's rules (descriptive names, exhaustive matches, public items first, no panics or
  printing outside tests), the helpers to reuse, and the controller's patterns (the injected
  clock, the scheduler, daylight-saving resolution, MQTT publishing through the broker, logging,
  concurrency and tests). Use when writing, editing or reviewing any .rs file in
  awtrix-controller; consult it before generating Rust code, not after clippy fails.
---

# Rust Style Guide — awtrix-controller

Rules from the root `Cargo.toml` (`[workspace.lints]`), `clippy.toml`, `rustfmt.toml` and the
existing code in `src/`, `tests/` and `meta/`. §1–§4 are what the lints enforce, §5–§7 are the
maintainer's rules, §8–§13 are this controller's patterns, §14–§15 are review rules.

Every lint and test command runs workspace-wide, so both packages (`awtrix-controller` and
`meta`) are held to the same set: `cargo clippy --workspace --all-targets --locked -- -D warnings`,
`cargo test --workspace --locked`, `cargo fmt --all --check`. Never commit with `--no-verify`;
fix the underlying issue instead.

---

## 1. The lint set

`[workspace.lints]` is declared once, and every member opts in with `[lints] workspace = true`
(a member does not inherit it otherwise; `meta/tests/workspace_lints.rs` fails when one does not).
`clippy.toml` sits at the workspace root, so its settings apply to both packages.

| Lint | Level | What to do |
| --- | --- | --- |
| `unsafe_code` | forbid | No `unsafe`, anywhere |
| `clippy::pedantic` | warn (priority -1), an error under `-D warnings` | Satisfy it; the usual ones below |
| `allow_attributes`, `allow_attributes_without_reason` | deny | `#[expect(lint, reason = "...")]` only (§2) |
| `unwrap_used`, `expect_used` | deny | Return an error; tests may use both (§3) |
| `print_stdout`, `print_stderr` | deny | Log through `tracing`; `writeln!` to a handle when output is the point (`--version`) |
| `wildcard_enum_match_arm` | deny | Name every variant (§6) |
| `min_ident_chars` | deny | Descriptive names; only `x` and `y` are allowed (§5) |
| `disallowed_methods` | deny | No wall clock outside `src/clock` (§8) |

The pedantic lints that come up most:

- `must_use_candidate`: put `#[must_use]` on a pure function or constructor that returns a value.
- `missing_errors_doc`, `missing_panics_doc`: a public function returning `Result` has an
  `# Errors` section; one that can panic has `# Panics` (better: make it not panic).
- `doc_markdown`: identifiers in doc comments go in backticks (`` `QoS` ``, `` `client_id` ``).
- `too_many_lines` (100): split the function into named steps, as `run` does.
- `needless_pass_by_value`, `trivially_copy_pass_by_ref`: borrow what you only read; pass small
  `Copy` types by value.
- `cast_possible_truncation`, `cast_precision_loss`, `cast_sign_loss`: use `try_from` or
  `From`; where the cast is the point (a float temperature to an integer for display), `#[expect]`
  it with the reason the range is safe.
- `struct_excessive_bools`, `too_many_arguments`: these mirror the firmware's payloads and Go's
  constructors in a few places, each behind an `#[expect]`; a new one needs the same justification.

`rustfmt.toml` sets `edition = "2024"` and `max_width = 100`. The `rustfmt-on-edit.sh` hook
formats a `.rs` file after every edit in Claude Code, and the `cargo-fmt` pre-commit hook checks.

## 2. Suppressions: `#[expect]` with a reason, counted

- Only `#[expect(clippy::lint, reason = "...")]`. `#[allow]` is denied, and so is either
  attribute without a reason. Unlike `#[allow]`, an `#[expect]` fails the build once the lint
  stops firing, so a stale suppression cannot linger.
- Put it on the smallest item that needs it: the statement or the function, not the module. A
  crate-level `#![expect(...)]` is for an integration-test file whose helpers all share the
  reason (`tests/run.rs`: `expect_used` in helpers that are not `#[test]` functions, which
  clippy's test allowance does not cover).
- The reason says why the fix does not apply here, not which lint fired:
  `reason = "the payload is the firmware's: one boolean per display toggle"`, not
  `reason = "too many bools"`.
- Suppressions are ratcheted: `scripts/check-suppressions.sh` counts every `#[expect]` and
  `#[allow]` per lint against `.suppressions-baseline` and refuses a commit that adds one without
  changing the baseline. After adding, moving or removing one: stage it, run
  `scripts/check-suppressions.sh --update`, and commit the baseline with it. Raising a count
  needs a reviewer's agreement.

## 3. Errors

- One `thiserror` enum per module (`ConfigError`, `BrokerError`, `PushError`, `FetchError`),
  with `#[source]` on the underlying error and the context in the variant's fields. There is no
  `anyhow`.
- The message starts with the module and names the operation and its subject:
  `#[error("run: publish notification to {topic}: {source}")]`,
  `#[error("settings: publish to {topic}: {source}")]`. The config errors name the offending key.
- No `unwrap`/`expect` outside tests. A `Mutex` that may be poisoned is read with
  `.lock().unwrap_or_else(PoisonError::into_inner)`: the data is still consistent for this
  code, and a panic elsewhere must not take the controller down.
- A failure the controller can carry on from (one device's push, one fetch) is logged with the
  error as a field and the work continues; a startup failure returns an `Exit` code from `run`
  (`1` flags or configuration, `2` startup). `main` is the only place that exits the process.

## 4. Documentation and comments

- Every module starts with a `//!` comment saying what it owns; every public item has a `///`
  comment starting with what it is or does.
- Every `.rs` file starts with the MIT license block (`Copyright (c) 2026 Roberto Leinardi`, the
  year of the file's first commit), then the module comment.
- A comment says **why the code is the way it is** (the hook's comment on why a disconnect
  checks the session key); history and "changed X to Y" belong in the commit body.
- Where the Rust deliberately differs from the Go implementation (Q1–Q10 in the README's
  migration section), the module comment names the difference (`src/daynight` on Q3,
  `src/model` on Q7).
- `TODO` is allowed; a comment on code you did not otherwise change is not.

## 5. Names

- `min_ident_chars` refuses single-character names except `x` and `y` (the draw commands'
  coordinates). Beyond that, name a binding for what it holds at any distance: `index`, `state`,
  `zone`, `client_id`, `connections`, never `i`, `s`, `tz`, `id`. Closure parameters too
  (`|connection| ...`, not `|c| ...`).
- Error bindings name their source when several are in scope: `zone_error`, `parse_error`.
- An unused parameter is `_`, or `_name` when the name documents it.

## 6. Matches, item order, types

- **Exhaustive matches.** A `match` on an enum names every variant (`wildcard_enum_match_arm`);
  group them with `|` when they share an arm (`Some(_) | None => false`). A new variant is then a
  compile error everywhere it matters. Matching on integers or strings may keep a `_` arm.
- **Item order.** Within an `impl`, public items come before private ones (`Scheduler`: `new`,
  `schedule`, `cancel`, `stop`, then the private helpers). Not linted: checked in review.
- **Constructors.** A controller has `new` for production and a `with_…` constructor taking the
  seam the tests replace (`DayNight::with_sun_function`, `weather::Controller::with_fetch_function`).
- **Open firmware strings.** An Awtrix value that is an open set (an effect name) is an enum
  with an `Other(String)` variant and hand-written serde, so an unknown value still reaches the
  device; a closed set (text case, push icon) is a plain enum.
- **Payload shape.** `src/model` mirrors the firmware's keys exactly (`#[serde(rename = ...)]`),
  skips a field when Go's `omitempty` would, and keeps the fields Go always wrote (`OVERLAY` as
  `null`) unskipped. A new `skip_serializing_if` on a field whose zero value matters changes what
  the device does.

## 7. Reuse before writing

| Need | Use | Not |
| --- | --- | --- |
| The current time | the injected `Arc<dyn Clock>`; `TokioClock` (follows the paused tokio clock) or `FakeClock` in tests | `Timestamp::now()`, `Zoned::now()`, `SystemTime::now()`, `Instant::now()` |
| A local date and time as an instant | `clock::resolve_local` | `to_zoned`/`to_timestamp` with your own disambiguation |
| The next local midnight | `clock::next_local_midnight` | adding 24 hours |
| A timer, one-shot or recurring | `Scheduler::schedule(name, first, action, reschedule)` | `tokio::spawn` with a `sleep` loop |
| The next notification time | `next_daily_occurrence`, `next_weekly_occurrence`, `next_monthly_occurrence`, `next_yearly_occurrence` (`src/notification`) | a second date calculation |
| A weekday name | `notification::parse_weekday` | a second match on strings |
| The energy-saving window | `energysaving::Window` (`for_date`, `is_active`, `next_transition`) | comparing times of day |
| The devices to send to | `Registry::connected_ids()` (a sorted copy) | iterating the broker's sessions |
| The settings payload | `settings::Builder::build()`, published with `settings::push` | assembling `model::Settings` at the call site |
| A notification to one device | the `Devices` publisher in `src/run` (`{client_id}/notify`, QoS 1, not retained) | a second publish path |
| Notification text | `TextContent::plain` | building the enum variant by hand |
| Captured log output in a test | `test_support::Captured::install` | a per-test `set_default` alone (it loses records, §13) |
| A broker in a test | `tests/broker.rs`: `start`, `connect`, `eventually` | a per-test setup |
| A whole run in a test | `tests/run.rs`: `drive`, `device`, `wait_for`, `free_port` | spawning the binary unless the process is the point |

## 8. Time, time zones and scheduling

- **Clock discipline.** `clippy.toml`'s `disallowed-methods` bans `jiff::Timestamp::now`,
  `jiff::Zoned::now`, `std::time::SystemTime::now` and `std::time::Instant::now`. Only
  `src/clock` (`SystemClock`, `TokioClock`) and the debug weather simulation read them, each
  behind an `#[expect(clippy::disallowed_methods, reason = "...")]`. Everything else takes an
  `Arc<dyn Clock>`. (`Broker::shutdown` measures its deadline with `tokio::time::Instant`, which
  follows the paused clock and is not banned.)
- **Daylight saving.** Every "local date and wall time to instant" conversion goes through
  `clock::resolve_local`: a time in a gap moves forward by the gap's length (02:30 becomes
  03:30), a time in a fold takes the earlier instant. No other module calls jiff's
  disambiguation directly. Do calendar arithmetic on civil types (`Date`, `Time`) and convert
  once; never add a duration to local midnight, which is an hour off on a transition day.
- **Clamping.** A monthly day past the month's end is the last day (Q4); `02-29` is February 28
  outside leap years (Q5). Code relying on a different rule says why.
- **Time zone.** Day/night, energy saving and notifications use the configured zone, or the
  system's with a warning; the weather controller uses `UTC` when none is configured, as Go did.
  Zone names are looked up in jiff's bundled database (`config::load_time_zone`), so the static
  image needs no zoneinfo.
- **Scheduler.** `Scheduler::schedule` runs one task per job: sleep until the fire time, run the
  action, ask `reschedule(fired)` for the next one (strictly later), repeat until cancelled.
  `Scheduler::stop` cancels and waits for the jobs; `cancel` only cancels, for shutdown paths
  that must not wait on an action in flight. A timer outside the scheduler escapes shutdown.
- **One job per controller.** Day/night computes its mode in `start` and then toggles on every
  fire, as Go did, so it must have exactly one job; a second job or a skipped fire inverts the
  mode until restart. Energy saving instead derives its state from the window's instants on every
  fire, so continuous operation and a restart agree, and its next-transition search walks past
  empty windows.
- **Firing late.** A job that re-derives state when its timer fires evaluates at
  `max(clock.now(), due)`, as `EnergySaving::update` does: the tokio timer is monotonic, and a
  wall clock stepped back would otherwise read just before the boundary and skip the change.

## 9. MQTT publishing through the broker

- The broker (rmqtt) runs on its own runtime and thread, owned by `Broker`; the controller
  publishes with `BrokerHandle::publish(topic, payload, retain)`, which crosses to the broker's
  runtime and publishes at QoS 1 as the server.
- Settings go to `{client_id}/settings`, **retained** (`settings::push`). Notifications go to
  `{client_id}/notify`, **not retained**: a retained notification replays on every reconnect.
- The broker hook (`ControllerHook`) is called on the broker's runtime: it never blocks and
  never publishes. A connect or a first `/stats` calls a `ClientCallback`, which only queues an
  `Event` (`Connected`, `Ready`, `PushAll`) for `src/run` to serve on the controller's runtime.
  Controllers do the same through their `on_change` callbacks.
- The hook is the only authentication authority: `ClientAuthenticate` stops the chain with
  `Allow` for exact credentials and `BadUsernameOrPassword` otherwise, never `NotFound`, which
  rmqtt reads as "ask the next handler". Every listener is built through the one `listener`
  helper, with `allow_anonymous(false)` (rmqtt-net defaults it to true, which admits a
  username-less CONNECT before any hook runs) and `reuseaddr(Some(true))`.
- A disconnect unregisters a device only when its `SessionKey` is still the current one: a
  takeover's new session connects before the old one's disconnect arrives.

## 10. Logging

- `tracing` macros only, through the global subscriber `logger::init` installs (text or JSON,
  Q8). Crates logging through `log` (rmqtt) are bridged with tracing-log.
- Structured fields, never `format!` in the message:
  `warn!(client_id, error = %push_error, "settings: push failed")`. Use `%` for `Display`,
  `?` only when the `Debug` form is what a reader needs.
- Messages start with the module (`"broker: client connected"`, `"weather: fetch failed"`). The
  device key is `client_id`.
- Never log the MQTT password; `Config::debug_view` (`ConfigDebugView`) omits it on purpose.
- The logger is installed before the configuration is loaded: tracing drops every event before a
  subscriber exists, so the loader's warnings would vanish. `config::peek_log_format` reads only
  `log_format` for that.

## 11. Concurrency and shutdown

- Shared state sits behind a `std::sync::Mutex` (held briefly, never across an `.await`) or an
  atomic; readers get copies (`Registry::connected_ids`, `Registry::snapshot`).
- Cancellation is a `tokio_util::sync::CancellationToken` tree from `run`. The scheduler's loop
  prefers cancellation over a due timer (`biased` select) and checks it again after the action,
  before rearming; a new loop does the same.
- Blocking I/O runs in `spawn_blocking` (the ureq fetch, under a 10 s global timeout) and is never
  awaited at shutdown.
- Shutdown has one budget, `SHUTDOWN_BUDGET` (2 s) from the signal: each step gets only what
  remains, sessions are kicked concurrently, and `main` ends its hand-built runtime with
  `shutdown_timeout(remaining)` so a detached blocking task cannot hold the process open.

## 12. Tests

- Unit tests sit beside the code: a `#[cfg(test)] mod tests` at the end of the file, or a
  `tests.rs` in the module directory. Integration tests over real sockets and whole runs are in
  `tests/`; repository meta-tests are in `meta/tests/`.
- A test ported from Go carries a doc comment naming the Go function it replaces:
  ``/// Go: `TestJobFiresOnceThenReschedules` ``.
- Time-dependent tests run on a paused runtime (`#[tokio::test(start_paused = true)]`) with a
  `TokioClock`, so a day of schedule takes no wall time. Never `std::thread::sleep`.
- Waiting, by class:
  - **Positive eventual:** poll with `eventually`/`wait_for` or await a channel, under
    `tokio::time::timeout`, and fail naming what never happened. Under a paused clock a wait with
    no timeout spins forever instead of failing, because a recurring job keeps the clock moving.
  - **Negative assertion:** give the wrong behaviour a bounded window, then assert it did not
    happen, and say so in a comment.
  - **Real elapsed time:** only when the duration is the point (the shutdown budget), as a named
    constant.
- Logs: assert on records captured with `test_support::Captured::install`, which installs a
  global catch-all subscriber first; a scoped `set_default` alone loses records under the
  parallel test runner (tracing caches callsite interest across threads).
- A test that cancels a hung blocking call builds its runtime by hand and calls
  `shutdown_background()`; a `#[tokio::test]` runtime waits for the blocking task on drop.
- A meta-test gated on a tool (hadolint) returns early when the tool is missing, unless
  `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1`, which turns it into a failure (CI sets it).

## 13. What to avoid

- `unwrap`/`expect`, `println!`/`eprintln!`, `unsafe`, or a wall-clock read outside the places
  above.
- `#[allow]`, or an `#[expect]` whose reason restates the lint (§2).
- A `_ =>` arm on an enum (§6).
- A second date calculation, publish path or test harness when §7 has one.
- Blocking or publishing inside the broker hook (§9).
- Designing for hypothetical requirements: no configurability or abstractions for features that
  do not exist yet.
- Skipping or suppressing pre-commit hooks (`--no-verify`).

## 14. Quick checklist before submitting Rust code

- [ ] `cargo fmt --all` clean; clippy clean with `--workspace --all-targets -- -D warnings`
- [ ] No `unwrap`/`expect`/printing outside tests; errors are the module's `thiserror` enum
- [ ] Each `#[expect]` has a reason saying why the fix does not apply; baseline updated (§2)
- [ ] Descriptive names; every enum variant matched by name
- [ ] Public items before private ones in each `impl`
- [ ] `#[must_use]`, `# Errors` and backticked identifiers where pedantic asks
- [ ] Checked §7 for an existing helper; time from the `Clock`, timers from the `Scheduler`,
      local times through `resolve_local`
- [ ] Settings retained, notifications not; nothing blocks in the hook
- [ ] Comments say why, not what changed; module comments name any deliberate Go difference
- [ ] Ported tests carry their `Go:` comment; no unbounded wait under a paused clock

## 15. Lint and test loop

1. `make rust-fmt` (or let the edit hook do it).
2. `make verify`: `rust-build`, `rust-fmt-check`, `rust-clippy` and `rust-test` over the
   workspace, the meta-tests included.
3. Fix each report and re-run until clean. For a single test:
   `cargo test --locked -p awtrix-controller --lib <module>::tests::<name>`.
4. `make check-stage` (or `make check`), then `git status`: hooks may rewrite files in place. If
   you added, moved or removed a suppression, run `scripts/check-suppressions.sh --update` and
   stage `.suppressions-baseline`.
