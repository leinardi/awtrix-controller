---
name: adversarial-review
description: >
  Adversarial code review of changes to awtrix-controller — working tree, staged
  diff, a branch vs main, a commit range, or a PR — in any language (Rust, bash,
  Dockerfile, Makefile, YAML, docs). Uses user-level Rust skills for Rust paths, hunts for
  real defects and violations of the controller's invariants (MQTT broker auth and
  hooks, client state across reconnects, scheduling and timezones, config
  validation, the settings and notifications pushed to devices), then reports
  ranked findings with a verdict. Use when the user asks to review changes, a diff,
  a PR or a branch, to "check my work before committing", whether it "is ready to
  merge", or to "poke holes in this" — even if they don't name a language or say
  the word "review".
---

# Adversarial Review — awtrix-controller

You are a hostile reviewer. Assume the change is **wrong until proven right**: it hides a
bug, breaks an invariant, or drifts from a contract. Your job is to find the specific
input, state, or path where it fails — not to praise it, not to restyle it. A review that
finds nothing is only credible after you have actively tried to break the code and failed.

Copy this checklist and tick items as you go:

```text
Review progress:
- [ ] 1. Scope chosen; diff, stated intent and every changed file read in full
- [ ] 2. User-level Rust skills and AGENTS.md consulted for the Rust paths
- [ ] 3. Repo invariants checked
- [ ] 4. Adversarial passes run; every candidate confirmed or dropped
- [ ] 5. Always-on passes (a)–(e) run
- [ ] 6. Gates run; `git status` checked for hook rewrites; skipped gates marked unverified
- [ ] 7. Report written: findings, open questions, verdict, gates run and not run
```

---

## 1. Establish the diff (what am I reviewing?)

Never review from memory or from the user's description of the change — read the actual
diff. Pick the scope from what the user said:

| User intent | Command |
| --- | --- |
| "my work" / "before I commit" / uncommitted | `git status --short`, then `git diff HEAD`; read untracked files too, which no diff shows |
| staged changes only | `git diff --staged` |
| a branch / "this PR" / "ready to merge" | `git diff main...HEAD` (merge-base diff; `main` is this repo's default branch) |
| a specific commit range | `git diff <base>..<head>` |
| a GitHub PR number | `gh pr view <n>` for intent, then `gh pr diff <n>` |

If the user names no scope, review the uncommitted work (first row); if the tree is
clean, review the branch against `main` (third row).

Also read `git log --oneline` for the range and any linked issue/PR body — the stated
**intent** is what you check the code against. A change that works but does something other
than what it claims is a finding, and so is a commit type that ships the wrong version: `svu`
derives the release from the Conventional Commits since the last tag, and PRs land as merge
commits, so every commit counts. A change users should receive in the binary or the image typed
`chore`, `docs`, `refactor` or `perf` never ships; tooling, CI or docs typed `feat` or `fix`
cuts a release that changes nothing; a stray `!` cuts a major; and a word ending in `feat` or
`fix` right before a colon anywhere in a subject counts as that type (`AGENTS.md`,
`docs/release.md`).

Read every changed file in full, not just the hunks. For non-trivial changes, also read the
callers, implementations, tests and docs of what changed — found with a reference search, not
assumed from the diff: a signature or behavior change is only safe if every call site agrees.

## 2. Load the domain skill

For any `**/*.rs` path (and `Cargo.toml`, `clippy.toml`, `rustfmt.toml`), load
user-level Rust skills and read `AGENTS.md` before judging it; project rules are findings
even when clippy is green. Every other path (Dockerfile, compose,
`Makefile`, `.mk/*.mk`, workflows, other YAML, bash, Markdown) gets the passes in §4 plus the
invariants in §3 with the **same rigor** — an unmatched language is not a lighter review.

## 3. Repo invariants — check these on every review, whatever changed

These are the ways this controller breaks that generic reviewers miss.

### The broker admits only configured credentials

- `config` rejects an empty `mqtt.username` or `mqtt.password`, and
  `ControllerHook::authenticate` returns `Allow` only when both match exactly, otherwise
  `BadUsernameOrPassword` (and `NotAuthorized` once the broker is closing).
- rmqtt asks its `ClientAuthenticate` handlers in turn; a `NotFound` result, or a handler that
  lets the chain continue, hands the decision to the next one or to rmqtt's default. The hook must
  stay the only handler registered for it and must stop the chain with its result (check the
  `register` calls in `broker::set_up`). A second auth handler, a `NotFound` path, or a result
  that allows on any other path opens the broker: **critical**.
- Every listener is built by the one `listener` helper with `allow_anonymous(false)`: rmqtt-net
  defaults it to true, and rmqtt then accepts a username-less CONNECT **before any hook runs**. A
  listener built another way, or the flag dropped, opens the broker: **critical**.
- There is no ACL hook: every authenticated client may publish and subscribe to every topic, by
  design, since all devices share one credential. A change that relies on one device being
  unable to read or publish another's topics is wrong.
- The listeners are plain TCP (and optional WebSocket) with no TLS; the credentials cross the
  network in clear. Never log the password (`Config::debug_view` omits it) or echo it in an
  error.

### Client state survives reconnects

- `ControllerHook::connected` stores the session's `SessionKey` (create time and remote address)
  per client ID and registers the client; `disconnected` unregisters only when the stored key is
  the disconnecting session's, because rmqtt connects a takeover session before the old one's
  disconnect arrives. Dropping that check removes a live device from the registry
  (`takeover_does_not_unregister_the_new_session` in `tests/broker.rs`).
- `Connection::ready` makes `on_device_ready` fire once per connection, on the first `…/stats`
  publish of the current session, and a reconnect starts a new `Connection`, so it fires again.
- The hook runs on the broker's runtime and must not block or publish: its callbacks only queue
  an `Event` for `src/run`, which pushes settings and replays notifications on the controller's
  runtime. Blocking I/O or an `.await` on the controller inside the hook stalls the broker.
- The registry is keyed by MQTT client ID, and the code uses that ID as the device's topic
  prefix (`{client_id}/settings`); entries are removed on disconnect. `published` routes by
  topic suffix and records state under the publishing session's client ID, never the topic's
  prefix.

### What is pushed to devices is the firmware's contract

- Settings go to `{client_id}/settings`, **retained**, QoS 1 (`settings::push`), as a partial
`model::Settings`: only the managed fields, each skipped when unset except `OVERLAY`, whose `null`
clears the overlay. `BRI`/`ABRI` are `Option`s so `0`/`false` are sent: energy saving active is
`BRI=1, ABRI=false`, inactive is `ABRI=true` with no `BRI`. Settings are pushed on connect and to
every connected
  device on each day/night, energy-saving or overlay change.
- Notifications go to `{client_id}/notify`, **not retained**, QoS 1. A retained notification
  would replay on every reconnect: a finding.
- The serde names in `src/model` are the firmware's exact keys, in Go's field order, and the
  JSON is Go's except that `<`, `>` and `&` are written literally (Q7). A renamed key, an
  `Option` turned into a plain value, a new `skip_serializing_if` on a field whose zero value
  matters, or an open name (an effect) narrowed to a closed enum changes what the device does.

### Scheduling and timezones

- Every recurring timer goes through `Scheduler::schedule`, whose loop prefers cancellation
  over a due timer and checks it again after the action, before rearming. `Scheduler::stop`
  waits for the actions in flight; `cancel` does not. A timer outside the scheduler survives
  shutdown.
- Day/night computes its mode in `start` and then **toggles** on each fire, so it must have
  exactly one job: a second job, a skipped fire or a double fire inverts the mode until restart.
  Energy saving re-derives its state from the window's resolved instants on every fire (at
  `max(now, due)`), and its next-transition search walks past empty windows; a change that
  compares times of day, or bounds the search to D-1..D+1, breaks restart parity.
- Every local date and wall time becomes an instant through `clock::resolve_local` (a gap moves
  forward by its length, a fold takes the earlier instant); another conversion is a finding.
  Monthly days past the month's end clamp to its last day, `02-29` to February 28 outside leap
  years. Fire times are strictly after `from`.
- No wall clock outside `src/clock` and the debug weather simulation (`clippy.toml`
  `disallowed-methods`); a new `#[expect(clippy::disallowed_methods)]` elsewhere is a finding.
- With no `timezone`, day/night, energy saving and scheduled notifications use the system zone
  but the weather controller uses `UTC`, as Go did.
- Polar night or midnight sun (the sun function returns `None`) means Night and a retry at the
  next local midnight, never a spin.

### Config fails closed where it matters

`config::load` rejects missing MQTT credentials, a missing latitude or longitude, an unknown
IANA time zone, a malformed theme color, an unknown `log_format` and an out-of-range weather
threshold; `run` exits `1` on a flag or config error and `2` when startup fails. What does
**not** fail (verify before relying on it), and must not be assumed to:

- An invalid scheduled notification is skipped with a warning, not rejected.
- `energy_saving.start`/`end` are only defaulted by the loader; a malformed value fails when the
  energy-saving controller starts, exit `2`.
- Unknown keys are ignored, as yaml.v3 did, so a misspelled key silently becomes its default.
- The decoder reads YAML the way yaml.v3 did (`yes`/`no` booleans, octal and `_` in integers,
  `null` as absent); a change to it that narrows what loads breaks existing configs.

The config is loaded once; there is no reload. `AWTRIX_CONFIG` and `AWTRIX_LOG_LEVEL` apply only
when the matching flag was not given; `log_format` is read before the full load so the loader's
warnings reach the configured format.

### Weather is optional and outbound only

Disabled unless `weather.enabled`. Polls `api.open-meteo.com` through the scheduler with ureq
in `spawn_blocking`, each fetch under a 10 s global timeout; shutdown never waits for a fetch in
flight. A `null` in the forecast is a missing value (Q1), skipped by the checks that need it. A
failed fetch clears the overlay and keeps event state; the state manager decides what to notify
(new, more severe, changed fingerprint, or older than the repeat interval), and
`on_device_connected` replays the last batch while it is younger than
`notification_repeat_minutes`. `--weather-wmo` replaces the fetch with a simulation for
debugging.

### Shutdown is bounded

`SHUTDOWN_BUDGET` (2 s) runs from the signal: the controllers' token and the scheduler are
cancelled, the broker stops accepting and kicks every session concurrently under the remaining
time, and `main` ends its hand-built runtime with `shutdown_timeout(remaining)`. A new step that
waits on its own grace period, awaits a fetch, or kicks sessions one by one breaks the budget.

### Image, release and CI

- The image (`deployments/docker/Dockerfile`) pins every base by tag (not by digest: dhi.io
  republishes its tags with security fixes), pins the `# syntax` frontend by digest, carries the
  seven OCI labels `droast.toml` requires, and runs on `dhi.io/static`, relying on its non-root
  default user (there is no explicit `USER`). The build stage cross-compiles a static musl binary
  with cargo-zigbuild, and every download in it (zig, `rust-std`) is checked against a sha256.
  Adding root, a shell or a package manager to the runtime stage, a base on `latest` or with no
  tag, or an unchecked download, is a finding; `meta/tests/dockerfiles.rs` pins these.
- `docs/release.md` is the release contract: the `guard` job refuses any ref but `main`; the
  mode comes from fail-closed lookups (only "not found" reads as absent); the image is built
  once, scanned by digest, copied with `skopeo copy --all --preserve-digests`, attested and
  signed before it is tagged `:<version>`. Trivy exceptions live only in `.trivyignore`, with a
  reason and an `exp:` date.
- Every workflow starts at `permissions: contents: read`; actions are pinned to a full commit
  SHA.
- The commit type decides the release bump (`svu`): check that it matches whether the change
  should ship.

## 4. Adversarial passes — language-agnostic

Do not skim for style. Run these passes, each with a "how would I make this fail" framing:

- **Generic passes**: correctness and logic, boundaries and nil/empty, aliasing, error
  handling, concurrency, resources and security. For each, name one concrete failing input and
  trace it end to end rather than asserting "looks fine".
- **Time**: a boundary at midnight, a DST gap or fold (Berlin and New York differ), a date that
  does not exist every month or year, a timer that fires late or twice, a wall-clock read
  instead of the injected `Clock`.
- **Contract drift**: does the code do what the commit message / PR / issue claims? A flag,
  environment variable, config key, topic, payload field, exit code or error text changed
  without updating every consumer and the docs (§5 (e)).
- **Meta-tests**: a test under `meta/tests/` (pins, vacuous flags, Dockerfile invariants, docs links,
  the license and suppression scripts) guards a claim about the repository rather than the
  product. Make the guarded thing go wrong and check the test notices: a test that passes because
  its input is empty, its file list matched nothing, its rule never ran, its tool was skipped
  (`AWTRIX_CONTROLLER_REQUIRE_TOOLS` unset) or an allowlist entry swallows the new line is a
  false pass, and a meta-test that can no longer fail is a finding.
- **Tests**: does the diff add or change a test for the behavior it introduces? A test that
  passes against the *old* code, that asserts on a fake's recorded calls instead of the
  behavior they produced, or that was weakened/deleted to make the change pass — all findings.
  A bug fix with no regression test is a gap worth flagging.

For each candidate defect:

1. Reproduce it with a focused test, or trace one concrete input through the code to the wrong
   result.
2. Confirmed: it is a finding. Record the input and the wrong behavior.
3. Not confirmed: dig once more (callers, tests, config path). Still not confirmed: drop it.
   A vague "consider" is not a finding.

## 5. Always-on passes

These run on **every** review, whatever changed.

### (a) What reaches the broker and the devices

Ask: does this change let something new in — a CONNECT, a topic, a payload that becomes state
or a callback — or change what goes out: a topic, a retain flag, a payload field, the moment a
push happens? If it does, walk the broker, client-state and device-contract items in §3.

### (b) Deletion smell

A diff that removes a user-visible surface — a flag, an environment variable, a config key, a
topic, a payload field, a documented behavior — and in the same breath rewrites that surface's
test to assert it is *absent* must cite the specification line that retired it. The
specification here is the README and `deployments/config.sample.yaml`. A commit message is not a
specification.

A test flipped from "X happens" to "X does not happen" is not evidence that X should go. Ask:
which spec line retires this surface, and does it change in this diff? If none, this is a
**critical** finding whatever the diff's stated intent was.

### (c) A new suppression has to show its work

Any new `#[expect(...)]`, `# shellcheck disable=` or `# droast ignore=` must explain why the
fix does not apply, and arrives with `.suppressions-baseline` updated in the same commit
(`scripts/check-suppressions.sh --update`); a baseline raised without a reason in the diff is a
finding too: for `too_many_lines`, what broke when the split was tried; for a cast, why the range
is safe. A reason that restates the lint, or no reason, is a finding; so is an `#[allow]`
anywhere (the workspace lints deny it in Rust code). The same holds for a lint relaxed in
`[workspace.lints]` or `clippy.toml`, and for a member without `[lints] workspace = true`.

### (d) Cross-file duplication

Before accepting a new helper, search `src/**`, `tests/**` and `meta/src/**` for the one that
already exists, by *behavior*. Check the clock and `resolve_local`, the scheduler,
the next-occurrence helpers, the energy-saving
window, the registry, settings builder and push, the notification publisher, the log capture,
and the broker and run test harnesses.

### (e) Docs drift

- A flag in `src/cli` (the clap `Cli` and `TestNotificationArgs`), or an environment variable,
  needs its row in the README's CLI flag tables.
- A config key in `src/config` needs its entry in `deployments/config.sample.yaml` and, when
  user-facing, the README's Configuration overview, with the default from
  `src/config/defaults.rs`.
- A behaviour that differs from 1.x needs its row in the README's migration section.

A surface the code has and the docs do not mention is a finding; so is a documented one
nothing implements, and a default in the docs that differs from the code. Known gap (verify
before relying on it): `weather.data_stale_ttl_minutes` is parsed, defaulted and documented,
and nothing reads it; a diff that touches it either implements it or removes it with the docs.

## 6. Verify before you trust (don't hand-wave the gates)

Use focused tests while investigating (`cargo test --locked -p awtrix-controller --test broker
disconnect_unregisters`), then run the gates the change owes and treat a failure it caused as a
confirmed finding:

| Diff touched | Run |
| --- | --- |
| any `**/*.rs` | `make verify` (build, rustfmt check, clippy with `-D warnings`, tests), then `make check` |
| `Cargo.toml`, `Cargo.lock`, `deny.toml` | `make verify` and `make rust-deny` (cargo-deny; network required) |
| `deployments/docker/Dockerfile`, `droast.toml` | droast via `make check`, `cargo test --locked -p meta --test dockerfiles --test toolchain`, plus `make docker-build` |
| `.github/scripts/**` | `bash .github/scripts/binary-attestation-status.test.sh` |
| `meta/**`, `scripts/check-*.sh`, `.suppressions-baseline` | `cargo test --locked -p meta` with `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1` |
| `.github/workflows/release.yaml` | actionlint via `make check`; only a release dry run exercises it end to end, so say which steps you could not run |
| anything else | `make check` (pre-commit on all files) |

The broker tests start a real rmqtt broker on a free port and connect with rumqttc, so they are
part of `make verify`. Several hooks rewrite files (prettier, markdownlint): check `git status`
afterwards and report a rewrite as a finding. If a gate is impractical here (no Docker, no
network for cargo-deny), say so and mark that risk unverified.

## 7. Report

Rank by severity, worst first. An open broker, a leaked credential, or a device left in the
wrong mode until restart is normally **critical** or **high**. Skip pure formatting the linters
already catch unless it changes meaning or breaks a required gate. For each finding:

```text
<path>:<line> — <severity: critical | high | medium | low>: <one-line defect>
  Failure: <the concrete input/state → the wrong result or broken invariant>
  Fix: <the specific change>
```

Findings first, then open questions or assumptions, then a one-line verdict: **block**,
**approve with nits**, or **approve** — plus which verification gates you actually ran and which
you couldn't. If you found nothing, state what you tried to break so the "no findings" is
credible. Be blunt; do not soften a real defect to be polite, and do not invent findings to look
thorough.
