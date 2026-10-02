---
name: adversarial-review
description: >
  Adversarial code review of changes to awtrix-controller — working tree, staged
  diff, a branch vs main, a commit range, or a PR — in any language (Go, bash,
  Dockerfile, Makefile, YAML, docs). Loads go-style-guide for Go paths, hunts for
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
- [ ] 2. `go-style-guide` loaded for the Go paths
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
than what it claims is a finding.

Read every changed file in full, not just the hunks. For non-trivial changes, also read the
callers, implementations, tests and docs of what changed — found with a reference search, not
assumed from the diff: a signature or behavior change is only safe if every call site agrees.

## 2. Load the domain skill

For any `**/*.go` path, load `go-style-guide` before judging it; its rules (the maintainer's
rules included) are findings even when lint is green. Every other path (Dockerfile, compose,
`Makefile`, `.mk/*.mk`, workflows, other YAML, bash, Markdown) gets the passes in §4 plus the
invariants in §3 with the **same rigor** — an unmatched language is not a lighter review.

## 3. Repo invariants — check these on every review, whatever changed

These are the ways this controller breaks that generic reviewers miss.

### The broker admits only configured credentials

- `config.Validate` rejects an empty `mqtt.username` or `mqtt.password`, and
  `ControllerHook.OnConnectAuthenticate` accepts a CONNECT only when both match exactly.
- comqtt ORs its hooks: a connection is accepted if **any** hook providing
  `OnConnectAuthenticate` returns true, and a publish or subscribe if any hook providing
  `OnACLCheck` does. `ControllerHook` must stay the only hook that provides them (check the
  `AddHook` calls in `broker.New`). Adding a hook that allows (comqtt's allow-all auth hook
  included), or making `OnConnectAuthenticate` return true on any other path, opens the
  broker: **critical**.
- `OnACLCheck` allows every topic to every authenticated client, by design: all devices share
  one credential. A change that relies on one device being unable to read or publish another's
  topics is wrong.
- The listeners are plain TCP (and optional WebSocket) with no TLS; the credentials cross the
  network in clear. Never log the password (`NewConfigDebugView` omits it) or echo it in an
  error.

### Client state survives reconnects

- `OnConnect` stores the session pointer in `activeClients` and registers the client;
  `OnDisconnect` unregisters only when the stored pointer is the disconnecting one, because
  comqtt runs `OnConnect` for a takeover session before `OnDisconnect` for the old one.
  Dropping that check removes a live device from the registry
  (`TestBrokerStaleDisconnectDoesNotUnregister`).
- `readyClients` makes `onDeviceReady` fire once per connection, on the first `…/stats`
  publish, and is cleared on disconnect so a reconnect fires it again.
- Hook work runs off the broker's goroutine: the connect-time settings push in a goroutine
  that recovers panics, `onDeviceReady` in a goroutine. Known gap (verify before relying on
  it): the `onDeviceReady` goroutine does not recover panics. Blocking I/O inside a hook stalls
  the broker.
- The registry is keyed by MQTT client ID, and the code uses that ID as the device's topic
  prefix (`{clientID}/settings`); entries are removed on disconnect. `OnPublish` routes by topic
  suffix and records state under the publishing client's ID, never the topic's prefix.

### What is pushed to devices is the firmware's contract

- Settings go to `{clientID}/settings`, **retained**, QoS 1, as a partial `model.Settings`: only
  the managed fields, `omitempty` everywhere except `OVERLAY`, whose `null` clears the overlay.
  `BRI`/`ABRI` are pointers so `0`/`false` are sent: energy saving active is `BRI=1, ABRI=false`,
  inactive is `ABRI=true` with no `BRI`. Settings are pushed on connect and to every connected
  device on each day/night, energy-saving or overlay change.
- Notifications go to `{clientID}/notify`, **not retained**, QoS 1. A retained notification
  would replay on every reconnect: a finding.
- JSON tags in `internal/model` are the firmware's exact keys (`//nolint:tagliatelle` says
  so). A renamed tag, a dropped pointer or a new `omitempty` on a field whose zero value
  matters changes what the device does.

### Scheduling and timezones

- Every recurring timer goes through `scheduler.Scheduler`: `arm` adds to the wait group before
  creating the timer, `Stop` balances it when `Stop()` wins the race, and callbacks check the
  context before the action and before rearming. A change that breaks that accounting hangs or
  panics shutdown; a timer outside the scheduler survives it.
- Day/night and energy saving compute their state once in `Start` and then **toggle** on each
  fire, so each must have exactly one job. A second `Schedule` for the same controller, a
  skipped fire or a double fire inverts the mode until restart.
- Fire times are wall-clock times in the configured location, strictly after `from`. Known
  gaps (verify before relying on them): `energysaving.nextOccurrence` adds a duration to local
  midnight, so on a DST-change
  day the boundary is an hour off; monthly days 29–31 and yearly `02-29` roll into the next
  month through `time.Date`. With no `timezone`, day/night, energy saving and scheduled
  notifications use the system zone (`time.Local`) but the weather controller uses `UTC`.
- Polar night or midnight sun (`sunFunc` returns `ok=false`) means Night and a retry at the
  next local midnight, never a spin.

### Config fails closed where it matters

`config.Load` → `Validate` rejects missing MQTT credentials, a missing latitude or longitude,
an unknown IANA timezone and a malformed theme color; `run` exits `1` on a config error and `2`
when startup fails. What does **not** fail (verify before relying on it), and must not be
assumed to:

- An invalid scheduled notification is skipped with a warning, not rejected.
- `energy_saving.start`/`end` are only defaulted by `Validate`; a malformed value fails in
  `energysaving.Start`, exit `2`.
- Weather numbers: `0` means "use the default", negatives are not rejected.
- `yaml.Unmarshal` ignores unknown keys, so a misspelled key silently becomes its default.

The config is loaded once; there is no reload. `AWTRIX_CONFIG` and `AWTRIX_LOG_LEVEL` apply only
when the matching flag was not given.

### Weather is optional and outbound only

Disabled unless `weather.enabled`. Polls `api.open-meteo.com` through the scheduler, each fetch
under a 10 s timeout from `context.Background()` (so shutdown can wait for one). A failed fetch
clears the overlay and keeps event state; `StateManager` decides what to notify (new, more
severe, changed fingerprint, or older than the repeat interval), and `OnDeviceConnected`
replays the last batch while it is younger than `notification_repeat_minutes`. `--weather-wmo`
replaces the fetch with a simulation for debugging.

### Image, release and CI

- The image (`deployments/docker/Dockerfile`) pins every base by digest and runs on
  `dhi.io/static`, relying on its non-root default user (there is no explicit `USER`). Adding
  root, a shell or a package manager to the runtime stage, or an unpinned base, is a finding.
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
- **Time**: a boundary at midnight, a DST change, a date that does not exist every month or
  year, a timer that fires late or twice, a clock read with `time.Now()` instead of the
  injected `clock.Clock`.
- **Contract drift**: does the code do what the commit message / PR / issue claims? A flag,
  environment variable, config key, topic, payload field, exit code or error text changed
  without updating every consumer and the docs (§5 (e)).
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

Any new `//nolint:`, `# shellcheck disable=` or `# hadolint ignore=` must explain why the fix
does not apply: for a complexity rule, what broke when the extraction was tried; for
`varnamelen`, why the name has to be short (the *Maintainer rules* in `go-style-guide` say it
should not be). A
reason that restates the rule, or no reason, is a finding; so is a directive for a disabled
linter. The same holds for a new exclusion in `.golangci.yaml`, and an exclusion, enable or
setting there that matches nothing in this repo.

### (d) Cross-file duplication

Before accepting a new helper, search `internal/**` and `cmd/**` for the one that already
exists, by *behavior*. The *Reuse before writing* table in `go-style-guide` lists them (clock, scheduler, next-occurrence
helpers, the registry, settings builder and push, the notification publisher, test factories
and waits). The test timer factories already exist as per-package copies; a new test uses its
package's one.

### (e) Docs drift

- A flag in `runWithContext` or `registerTestNotificationFlags` (`cmd/awtrix-controller`), or
  an environment variable, needs its row in the README's CLI flag tables.
- A config key in `internal/config/config.go` needs its entry in
  `deployments/config.sample.yaml` and, when user-facing, the README's Configuration overview,
  with the default from `defaults.go`.

A surface the code has and the docs do not mention is a finding; so is a documented one
nothing implements, and a default in the docs that differs from the code. Known gap (verify
before relying on it): `weather.data_stale_ttl_minutes` is parsed, defaulted and documented,
and nothing reads it; a diff that touches it either implements it or removes it with the docs.

## 6. Verify before you trust (don't hand-wave the gates)

Use focused tests while investigating (`go test ./internal/broker -run TestBrokerDisconnect
-v`), then run the gates the change owes and treat a failure it caused as a confirmed finding:

| Diff touched | Run |
| --- | --- |
| any `**/*.go` | `make go-build`, `make go-vet`, `make go-test` (race detector on), then `make check` |
| `go.mod` / `go.sum` | `make go-tidy` and `make audit-deps` (govulncheck; network required) |
| `deployments/docker/Dockerfile` | hadolint via `make check`, plus `make docker-build` |
| `.github/scripts/**` | `bash .github/scripts/binary-attestation-status.test.sh` |
| `.github/workflows/release.yaml` | actionlint via `make check`; only a release dry run exercises it end to end, so say which steps you could not run |
| anything else | `make check` (pre-commit on all files) |

The broker tests start a real comqtt broker on a free port and connect with paho, so they are
part of `make go-test`. golangci-lint may not be on `PATH`; run it through pre-commit. Several
hooks rewrite files (prettier, markdownlint, the golangci formatters): check `git status`
afterwards and report a rewrite as a finding. If a gate is impractical here (no Docker, no
network for govulncheck), say so and mark that risk unverified.

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
