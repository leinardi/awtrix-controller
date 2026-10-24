# AGENTS.md

## What this is

A self-contained MQTT broker and automation controller for
[Awtrix3](https://blueforcer.github.io/awtrix3/) LED matrix clocks, written in Rust. Devices
connect to its embedded broker (rmqtt); the controller then pushes day/night themes and display
settings, runs an energy-saving window, fires scheduled notifications and overlays the Open-Meteo
weather forecast. The README and `deployments/config.sample.yaml` are the user-facing contract:
configuration keys, CLI flags and behaviour.

## Common commands

```bash
make rust-build    # cargo build --workspace --all-targets --locked
make rust-test     # cargo test --workspace --locked (the meta-tests in meta/ included)
make rust-fmt      # cargo fmt --all; rust-fmt-check only checks
make rust-clippy   # cargo clippy --workspace --all-targets --locked -- -D warnings
make verify        # verify-rust: rust-build, rust-fmt-check, rust-clippy and rust-test
make rust-run ARGS="--config config.yaml"   # run with AWTRIX_CONTROLLER_VERSION/_COMMIT/_DATE set from git
make rust-deny     # cargo deny check: advisories, licences, bans, sources (network required)
make audit-deps    # cargo deny check advisories (network required); cargo-deny also runs in CI and as a pre-commit hook on Cargo.toml/Cargo.lock/deny.toml changes
make check         # pre-commit on all files: rustfmt, clippy, tests, linters
make check-stage   # pre-commit on the staging area only
make docker-build
make docker-run
```

Before calling a change done, run the rows of the [validation matrix](#validation-matrix) that
match it.

Single test (a unit test is named by its module path, an integration test by its file):

```bash
cargo test --locked -p awtrix-controller --lib scheduler::tests::job_fires_once_then_reschedules
cargo test --locked -p awtrix-controller --test broker
cargo test --locked -p meta --test docs
```

The Makefile pulls shared snippets from `leinardi/make-common@v1` into `.mk/` on first run. To
refresh: `make mk-common-update`. Project targets live in the local `.mk/*.mk` files listed in
`MK_LOCAL_FILES` (`rust.mk`, `verify.mk`, `audit-deps.mk`, `docker-run.mk`), never as recipes in
the Makefile.

## Layout

A Cargo workspace: the root package `awtrix-controller` (a library plus the binary) and `meta`.
`rust-toolchain.toml` pins the toolchain, `rustfmt.toml` and `clippy.toml` sit at the root,
`deny.toml` configures cargo-deny, and the lint set is `[workspace.lints]` in the root
`Cargo.toml`.

- `src/main.rs` — the entry point: builds the runtime by hand and ends it within the shutdown
  budget. `src/lib.rs` holds the module tree.
- `src/cli/` — the clap flags, Go-compatible value parsing, and the debug-only test-notification
  flags.
- `src/run/` — wiring: config, logger, broker, controllers, the device event queue, signals and
  the 2 s `SHUTDOWN_BUDGET`. `src/version.rs` — the version strings.
- `src/broker/` — the embedded rmqtt broker: TCP and optional WebSocket listeners, and the hook
  that authenticates clients and reacts to connects.
- `src/clientstate/` — the thread-safe registry of connected clients.
- `src/config/` — loading and validating the YAML configuration, with defaults.
- `src/daynight/`, `src/energysaving/` — sunrise/sunset from the configured coordinates, and the
  nightly window.
- `src/settings/`, `src/notification/`, `src/weather/` — what is published to the devices:
  settings and themes, scheduled notifications, forecast overlays and severe-weather alerts
  (`src/weather/simulate/` is the debug `--weather-wmo` simulation).
- `src/scheduler/`, `src/clock/` — the recurring-job scheduler and the injectable clock the tests
  drive; `clock::resolve_local` is the one local-date-and-time-to-instant conversion.
- `src/model/` — the JSON payloads Awtrix3 accepts.
- `src/logger/` — the tracing subscriber (text or JSON).
- `src/test_support.rs` — test-only helpers, such as capturing the log output.
- `tests/` — integration tests: `broker.rs` (MQTT clients over real sockets) and `run.rs` (the
  whole run, shutdown and exit codes, and the built binary).
- `meta/` — the repository meta-tests, `publish = false`: `meta/tests/toolchain.rs` (one Rust
  version everywhere, the zig, cargo-zigbuild and hadolint pins, no vacuous flags),
  `dockerfiles.rs` (the image's stages, downloads, labels and frontend, and hadolint's label
  schema), `docs.rs` (Markdown links), `license_headers.rs` and `suppressions.rs` (the two scripts
  below), and `workspace_lints.rs` (every member opts into the workspace lints). `meta/src/` holds
  their helpers: `repo` reads tracked files, `gitenv` isolates scratch-repository git commands,
  `toolgate` finds hadolint, `dockerfile` parses the Dockerfile and `links` checks Markdown.
- `deployments/` — the Dockerfile (`dhi.io` bases pinned by tag, the `# syntax` frontend by
  digest; the build stage cross-compiles a static musl binary with cargo-zigbuild), a compose
  example and the sample config.
- `.github/actions/` — `setup-hadolint` and `setup-zigbuild`, pinned and sha256-checked.
- `docs/release.md` — how a release is cut and recovered.
- `scripts/` — `check-license-headers.sh`, `check-suppressions.sh`, `hooks/gofmt-on-edit.sh` (the
  Claude Code edit hook in `.claude/settings.json`) and make-common's `bootstrap-mk-common.sh`.

### Generated or vendored: never hand-edit

- `.mk/{docker,help,password,pre-commit}.mk` and `scripts/bootstrap-mk-common.sh`: fetched from
  make-common; refresh them with `make mk-common-update`, and port a change upstream.
- `.suppressions-baseline`: written by `scripts/check-suppressions.sh --update`.
- `Cargo.lock`: written by cargo; change it with `cargo update -p <crate>`, never by hand.

## Validation matrix

Run before considering an edit done. `make check` applies to every row.

| You edited | Run |
| --- | --- |
| `**/*.rs` | `make verify`, then `make check-stage` |
| `Cargo.toml`, `meta/Cargo.toml`, `Cargo.lock`, `deny.toml` | `make verify`, `make rust-deny` (network; the commit hook runs it too), `cargo test --locked -p meta --test toolchain --test workspace_lints` |
| `rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml` | `make verify`, `cargo test --locked -p meta --test toolchain` |
| `deployments/docker/Dockerfile`, `.hadolint.yaml`, `.dockerignore` | `cargo test --locked -p meta --test dockerfiles --test toolchain` (hadolint runs in `make check` and in the dockerfiles test), `make docker-build` (needs a `dhi.io` login) |
| `scripts/check-suppressions.sh`, `.suppressions-baseline` | `cargo test --locked -p meta --test suppressions` |
| `scripts/check-license-headers.sh` | `cargo test --locked -p meta --test license_headers` |
| `scripts/hooks/gofmt-on-edit.sh` | shellcheck (in `make check`); run it once by editing a file in Claude Code |
| `.github/scripts/binary-attestation-status.sh` | `bash .github/scripts/binary-attestation-status.test.sh` (also the `release-scripts` CI job) |
| `.github/workflows/**`, `.github/actions/**` | `pre-commit run actionlint --all-files`, `cargo test --locked -p meta --test toolchain --test dockerfiles` |
| `.pre-commit-config.yaml`, `Makefile`, `.mk/**` | `make check`, `cargo test --locked -p meta --test toolchain`, `make help` |
| `meta/**` | `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1 cargo test --locked -p meta` |
| `**/*.md` | `cargo test --locked -p meta --test docs`; prose wraps at 100 columns (markdownlint) |

## Rust coding rules

The lints live once, in `[workspace.lints]` of the root `Cargo.toml`, with their settings in
`clippy.toml`; every member declares `[lints] workspace = true`, and every lint and test command
runs workspace-wide. In short:

- **Lint baseline:** `unsafe_code` is forbidden; `clippy::pedantic` warns, and clippy runs with
  `-D warnings`.
- **Suppressions:** only `#[expect(lint, reason = "...")]`. `#[allow]` is denied
  (`allow_attributes`), and so is a suppression without a reason. The reason says why the fix does
  not apply here, not which lint fired.
- **No panics outside tests:** `unwrap_used` and `expect_used` are denied (tests may use both).
  Return an error, one `thiserror` enum per module; `anyhow` only in `main` and `run`.
- **No direct printing:** `print_stdout` and `print_stderr` are denied. Log through `tracing`; the
  version output is an explicit `writeln!` to stdout.
- **Exhaustive matches:** `wildcard_enum_match_arm` is denied: a match on an enum names every
  variant.
- **Descriptive names:** `min_ident_chars` refuses single-letter names except `x` and `y`. Prefer
  `index`, `state`, `timezone`, `client_id`.
- **Item order:** public items before private ones within an `impl`.
- **Tests:** unit tests sit in a `#[cfg(test)] mod tests` beside the code (or a `tests.rs` in the
  module directory). Timers are driven by the paused tokio clock (`#[tokio::test(start_paused =
  true)]`), not by sleeping. A test ported from Go carries a doc comment naming the Go test it
  replaces (``/// Go: `TestJobFiresOnceThenReschedules` ``).

## Conventions worth knowing

- **Clock discipline:** `clippy.toml`'s `disallowed-methods` bans `jiff::Timestamp::now`,
  `jiff::Zoned::now`, `std::time::SystemTime::now` and `std::time::Instant::now`. Scheduling and
  window code reads the time from the injected `src/clock` clock, so the tests can drive it; only
  `src/clock` and the debug weather simulation read the wall clock, each through an
  `#[expect(clippy::disallowed_methods, reason = "...")]`.
- **Daylight saving:** every "local date and wall time to instant" conversion goes through
  `clock::resolve_local` (a gap moves forward by its length, a fold takes the earlier instant). No
  other module calls jiff's disambiguation directly.
- **Version strings:** `src/version.rs` reads `AWTRIX_CONTROLLER_VERSION`,
  `AWTRIX_CONTROLLER_COMMIT` and `AWTRIX_CONTROLLER_DATE` at compile time (`option_env!`), falling
  back to `dev`, `none` and `unknown`. `make rust-run` sets them from git (`RUST_VERSION_ENV` in
  `.mk/rust.mk`), and the Dockerfile and the release workflow from the release version.
- Every Rust and shell file, the Dockerfile, the Makefile and the repo-local `.mk` fragments carry
  the MIT license header, with the year of the file's first commit (copy it from any Rust or shell
  file, or use the IntelliJ MIT copyright profile). The `check-license-headers` pre-commit hook
  (`scripts/check-license-headers.sh`) checks the whole tree; the make-common files are exempt.
- A new `#[expect]`, `# shellcheck disable=` or `# hadolint ignore=` explains why the fix does not
  apply here, not which rule fired.
- `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1` (set by the `rust` CI job and the release's `test` job)
  turns a missing hadolint into a meta-test failure instead of a skip.

## Suppressions are ratcheted

Every in-file suppression is counted per rule against `.suppressions-baseline` by the
`check-suppressions` pre-commit hook (`scripts/check-suppressions.sh`), in five families:
`expect/<lint>` (`#[expect(...)]`, one count per lint it names), `allow/<lint>` (`#[allow(...)]`,
which the lints refuse in Rust code, so it stays at zero), `shellcheck-disable/<code>`,
`hadolint-ignore/<rule>` and `hadolint-global-ignore/<rule>`.

Adding one is allowed, but never silently: the hook refuses a commit that adds a suppression line
without changing the baseline in the same commit, including a swap or a moved file that leaves the
count unchanged. Prefer fixing the finding. If a suppression really is right, write it with a reason
(`#[expect(clippy::<lint>, reason = "...")]`), stage it, run
`scripts/check-suppressions.sh --update`, and commit the baseline alongside it. A count that has
fallen below its baseline fails the same way, with the same fix: the file describes the repository,
not a past one.

## Project skills

Skills live in `.agents/skills/` (symlinked as `.claude/skills`). Load them before the work, not
after review:

- `go-style-guide` — before any `.go` edit.
- `adversarial-review` — for any review request ("review my diff", "is this ready to merge").
- `go-to-rust-port` — before planning or implementing any part of a Go-to-Rust port. Update it in
  the same commit as any new non-obvious learning.

## Commit messages

All commits MUST be Conventional Commits 1.0.0 **with a scope**:
`<type>(<scope>)[!]: <description>`, optional blank-line body and footers. Enforced by the
`conventional-pre-commit` `commit-msg` hook (`--force-scope`) and by the `conventional-commits` CI
job. Types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, `style`,
`revert`. Breaking changes use `!` before `:` or a `BREAKING CHANGE:` footer. Release notes are not
built from these messages: `gh release create --generate-notes` lists the merged pull requests by
title. Examples: `fix(scheduler): skip a missed run after a clock jump`,
`feat(weather): add a wind overlay`.

Release versions are derived by `svu` from the commits since the last tag, so a wrong type ships a
wrong version:

| Release | Commit | Example |
| --- | --- | --- |
| major | any type with `!` before the colon, or a `BREAKING CHANGE:` footer | `feat(config)!: rename the energy_saving keys` |
| minor | `feat` | `feat(weather): add a wind overlay` |
| patch | `fix` | `fix(scheduler): skip a missed run after a clock jump` |
| none | everything else: `perf`, `refactor`, `build`, `ci`, `chore`, `docs`, `style`, `test`, `revert` | `perf(broker): ...` |

The highest bump among the commits wins; with only "none" commits since the last tag, a release with
no version fails with "nothing to bump".

**Pick the type by whether the change should ship, not by what kind of change it is.** Anything that
changes the shipped binary or image and that users should receive is `fix` (or `feat`), even when it
is a performance improvement, a refactor or a revert. Use `perf`, `refactor`, `style` and `revert`
only when the commit is deliberately not meant to trigger a release on its own. A `revert` of a
shipped `feat` or `fix` is itself a `fix`. `svu` matches `feat`/`fix` anywhere in the subject (e.g.
`prefix:` counts as `fix:`), so avoid a word ending in `feat` or `fix` directly before a colon in
other subjects. PRs land as merge commits, so every commit counts, not just the PR title. See
[`docs/release.md`](docs/release.md).
