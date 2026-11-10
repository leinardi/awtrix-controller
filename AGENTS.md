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
make rust-build    # cargo build --workspace --all-targets --all-features --locked, unstamped
make rust-test     # cargo test --workspace --locked (the meta-tests in meta/ included)
make rust-fmt      # cargo fmt --all; rust-fmt-check only checks
make rust-clippy   # cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
make verify        # verify-rust: rust-build, rust-fmt-check, rust-clippy, rust-test and rust-doc
make rust-doc      # cargo doc --workspace --all-features --no-deps --locked, warnings as errors
make rust-run ARGS="--config config.yaml"   # run with AWTRIX_CONTROLLER_VERSION/_COMMIT/_DATE set from git
make rust-dist     # stamped release binary in dist/awtrix-controller
make rust-deny     # cargo deny check: advisories, licences, bans, sources (network required)
make audit-deps    # cargo deny check advisories (network required); cargo-deny also runs in CI and as a pre-commit hook on Cargo.toml/Cargo.lock/deny.toml changes
make check         # pre-commit on all files: rustfmt, clippy, tests, linters
make check-stage   # pre-commit on the staging area only
make dist-static   # the static release binaries, dist/awtrix-controller-linux-{amd64,arm64} (zig, cargo-zigbuild)
make image-build-prebuilt   # the image's prebuilt target from dist/, for IMAGE_PLATFORM (default: the host's)
make image-test    # scripts/image-test.sh on that image, IMAGE_TEST_TIER full (default), reduced or smoke
make image-build-test       # image-build-prebuilt, then image-test (ordered, safe under make -j)
make docker-build  # the image's default target, which compiles the binary itself
make docker-run
```

Before calling a change done, run the rows of the [validation matrix](#validation-matrix) that
match it.

Single test (a unit test is named by its module path, an integration test by its file):

```bash
cargo test --locked -p awtrix-controller --lib scheduler::tests::job_fires_once_then_reschedules
cargo test --locked -p awtrix-controller --test broker
```

The Makefile pulls shared snippets from `leinardi/make-common@v1` into `.mk/` on first run,
`rust.mk` among them, configured by the Makefile's `RUST_STAMP_PREFIX`. To
refresh: `make mk-common-update`. Project targets live in the local `.mk/*.mk` files listed in
`MK_LOCAL_FILES` (`verify.mk`, `audit-deps.mk`, `docker-run.mk`, `dist.mk`, `image.mk`), never as
recipes in the Makefile.

## Layout

A Cargo workspace: the root package `awtrix-controller` (a library plus the binary) and `meta`.
`rust-toolchain.toml` pins the toolchain, `rustfmt.toml` and `clippy.toml` sit at the root,
`deny.toml` configures cargo-deny, and the lint set is `[workspace.lints]` in the root
`Cargo.toml`.

- `src/main.rs` — the entry point: builds the runtime (service-kit's `build_runtime`) and ends it
  within the shutdown budget (`run_until_exit`). `src/lib.rs` holds the module tree.
- `src/cli/` — the clap flags, Go-compatible value parsing, and the debug-only test-notification
  flags.
- `src/run/` — wiring: config, logger, broker, controllers, the device event queue, the signal
  wait and the 2 s `SHUTDOWN_BUDGET`; `run::Exit` is service-kit's `Exit`, the exit code and the
  deadline `main` ends the runtime by. `src/version.rs` — the version strings (`buildinfo!`).
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
- `src/logger/` — the tracing subscriber (text or JSON), built by `tracing-setup` with the `log`
  facade bridged in; the level and format are resolved here.
- `src/test_support.rs` — test-only helpers: `install` scopes the JSON subscriber over a
  `tracing-setup` `Capture`.
- `tests/` — integration tests: `broker.rs` (MQTT clients over real sockets) and `run.rs` (the
  whole run, shutdown and exit codes, and the built binary).
- `meta/` — the repository meta-tests, `publish = false`: `meta/tests/toolchain.rs` (one Rust
  version everywhere, the zig, cargo-zigbuild and ShellCheck pins, Trivy never pinned, no vacuous
  flags), `dockerfiles.rs` (the image's frontend, its five stages and their tag-pinned bases),
  `workspace_lints.rs` (every member opts into the workspace lints) and `rust_common_pins.rs` (the
  rust-common pins: one `vX.Y.Z` tag, each `version` its `<major>.<minor>`). Most run rust-common's
  `repo-meta` checks with this repository's parameters, beside the rules that are its own.
  `meta/src/lib.rs` holds what they share: `repo()`, the repository under test.
- `deployments/` — the Dockerfile (`dhi.io` bases pinned by tag, deliberately not by digest:
  dhi.io rebuilds its tags too often; the `# syntax` frontend by digest; two targets on one `base`
  stage: `runtime`, the default, whose build stage cross-compiles a static musl binary with
  cargo-zigbuild, and `prebuilt`, which COPYs the release binary from the `dist` build context), a
  compose example and the sample config.
- `.github/workflows/` — `ci.yaml` (`pre-commit-all`, `rust`, `image`, `conventional-commits` and
  the `ci-result` aggregator; the reviewdog jobs on pull requests), `release.yaml` (`guard`,
  `ci-green`, `audit`, `release`; [`docs/release.md`](docs/release.md)), `scheduled-scan.yaml` and
  `pre-commit-warmup.yaml`. Shared steps come from
  [gh-composite-actions](https://github.com/leinardi/gh-composite-actions), pinned by SHA.
  `.trivyignore.yaml` holds the scans' accepted findings.
- `droast.toml` — the Dockerfile Roast policy: the seven required OCI labels, strict labels, a
  required ShellCheck bridge.
- `docs/release.md` — how a release is cut and recovered.
- `scripts/` — `check-license-headers.sh`, `image-test.sh` (the image tests in three tiers: CI's
  full amd64 and reduced arm64 suites, and the release's smoke test), `hooks/rustfmt-on-edit.sh`
  (the Claude Code edit hook in `.claude/settings.json`) and make-common's `bootstrap-mk-common.sh`.

### Generated or vendored: never hand-edit

- `.mk/{docker,help,password,pre-commit,rust}.mk` and `scripts/bootstrap-mk-common.sh`: fetched from
  make-common; refresh them with `make mk-common-update`, and port a change upstream.
- `Cargo.lock`: written by cargo; change it with `cargo update -p <crate>`, never by hand.

### Shared crates (rust-common)

`clock` (the injected clock, re-exported by `src/clock`), `service-kit` (`buildinfo!`, and with
its `runtime` feature the runtime builder and its bounded shutdown) and `tracing-setup` (the
logger's subscriber, with the `log-bridge` feature; the root `[dev-dependencies]` add its `testing`
feature for `Capture` and `set_default`) are the root package's; `repo-meta` and `testenv-core`
(default features) are the meta crate's. They come from
[leinardi/rust-common](https://github.com/leinardi/rust-common), a git dependency pinned by release
tag (`tag = "v1.1.0"`) in the root `Cargo.toml`'s `[workspace.dependencies]`, with
`version = "1.1"`, the tag's `<major>.<minor>`. Each member opts in with `workspace = true`, plus
its own features.

- **A behaviour difference is a parameter of the shared crate**, passed from here, never a fork of
  its code into this repository. A fix to shared code lands in rust-common first, then reaches here
  by moving the `tag`.
- **Moving the `tag`** needs the network: change it on all five lines (a partial move locks two tags
  side by side, and the `rust_common_pins` meta-test refuses it), and `version` with it when the
  tag's `<major>.<minor>` moves (a `v1.1.0` checkout does not resolve against `version = "1.0"`),
  then run `cargo fetch` before the `--locked` make targets, which otherwise refuse the stale lock.
  The manifest change is what makes cargo re-resolve the git source; `cargo fetch` changes nothing
  else, whereas `cargo update -p <crate>` also moves unrelated edges (windows-sys, socket2,
  getrandom). `git diff Cargo.lock` must touch only rust-common lines. Commit the `Cargo.toml` and
  `Cargo.lock` changes together. `deny.toml` allow-lists the source (`allow-git`); it changes with
  the URL. `make rust-deny` and the pre-commit hook check the whole workspace (`--workspace`), so
  the graphs of the root package and of the meta crate (`repo-meta`, `testenv-core` and their
  dependencies) are both gated.
- **Local work on a shared crate** uses an uncommitted
  `[patch."https://github.com/leinardi/rust-common"]` table in the root `Cargo.toml`, one
  `<name> = { path = "../rust-common/crates/<name>" }` line for each of the five crates (a patched
  crate's own rust-common dependencies, such as `go-compat`, are path dependencies there, so they
  follow). The patch re-locks the crates, so refresh `Cargo.lock` once without `--locked`
  (`cargo fetch`). When done, remove the table and `git checkout Cargo.lock`: neither
  the table nor the patched lock is ever committed, since CI and the image build cannot see the
  path.

This repository's intentional differences from the other Rust services:

- **The signal wait stays local** (`run::wait_for_stop`, not service-kit's `shutdown_signal`): it
  also ends on the `CancellationToken` `run_with_options` is given, which the tests stop the run
  with, and when SIGTERM cannot be installed it warns and keeps waiting on SIGINT, where
  service-kit's future would never resolve.
- **The local-time conversions stay local** (`clock::resolve_local`, `clock::next_local_midnight`):
  they are this controller's daylight-saving policy on jiff's zones, not part of the injected
  clock; only `Clock`, `TokioClock` and the tests' `FakeClock` come from
  `clock`.

## Validation matrix

Run before considering an edit done. `make check` applies to every row.

| You edited | Run |
| --- | --- |
| `**/*.rs` | `make verify`, then `make check-stage` |
| the rust-common `tag` in the root `Cargo.toml` | `cargo fetch` (only rust-common lines change in `Cargo.lock`), then `make verify`, `make rust-deny` and `cargo test --locked -p meta`, with the network ([Shared crates](#shared-crates-rust-common)); the `[patch]` table and a patched `Cargo.lock` are never committed |
| `Cargo.toml`, `meta/Cargo.toml`, `Cargo.lock`, `deny.toml` | `make verify`, `make rust-deny` (network; the commit hook runs it too), `cargo test --locked -p meta --test toolchain --test workspace_lints` |
| `rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml` | `make verify`, `cargo test --locked -p meta --test toolchain` |
| `deployments/docker/Dockerfile`, `droast.toml`, `.dockerignore` | `cargo test --locked -p meta --test dockerfiles --test toolchain` (droast runs in `make check`); gh-composite-actions' `check-pinned-bases/check-pinned-bases.sh --tag-only dhi.io deployments/docker/Dockerfile`; `make dist-static`, then `make image-build-test`, and with QEMU `make image-build-test IMAGE_PLATFORM=linux/arm64 IMAGE_TEST_TIER=reduced`; `make docker-build` (all need a `dhi.io` login) |
| `scripts/image-test.sh`, `.mk/dist.mk`, `.mk/image.mk` | `make dist-static`, then `make image-build-test` (and the arm64 run above); keep `docs/release.md#what-ci-proves` true |
| `.trivyignore.yaml` | every entry scoped to paths or purls, with a statement and an `expired_at`; scan the prebuilt image with the latest Trivy as `trivy-gate` runs it (`--ignore-unfixed --severity HIGH,CRITICAL --ignorefile .trivyignore.yaml`) |
| `scripts/check-license-headers.sh` | `pre-commit run check-license-headers --all-files` |
| `scripts/hooks/rustfmt-on-edit.sh` | shellcheck (in `make check`); run it once by editing a `.rs` file in Claude Code |
| `.github/workflows/**`, `.github/dependabot.yml` | `pre-commit run actionlint --all-files`, `cargo test --locked -p meta --test toolchain --test dockerfiles`; reason through each trigger the workflow has (pull request, fork pull request, push, dispatch, schedule); for `release.yaml`, keep `docs/release.md` in step and dispatch a dry run once CI passed on the commit; for `ci.yaml`'s `image` job, keep `docs/release.md#what-ci-proves` true |
| `.pre-commit-config.yaml`, `Makefile`, `.mk/**` | `make check`, `cargo test --locked -p meta --test toolchain`, `make help` |
| `meta/**` | `cargo test --locked -p meta` |
| `**/*.md` | `pre-commit run lychee --all-files` (offline links and fragments); prose wraps at 100 columns (markdownlint) |

## Rust coding rules

Use the maintainer's user-level Rust skills for general coding guidance. The lints live once, in
`[workspace.lints]` of the root `Cargo.toml`, with their settings in `clippy.toml`; every member
declares `[lints] workspace = true`, and every lint and test command runs workspace-wide. In short:

- **Lint baseline:** `unsafe_code` is forbidden; `clippy::pedantic` warns, and clippy runs with
  `-D warnings`.
- **Suppressions:** only `#[expect(lint, reason = "...")]`. `#[allow]` is denied
  (`allow_attributes`), and so is a suppression without a reason. The reason says why the fix does
  not apply here, not which lint fired.
- **No panics outside tests:** `unwrap_used` and `expect_used` are denied (tests may use both).
  Return an error, one `thiserror` enum per module.
- **No direct printing:** `print_stdout` and `print_stderr` are denied. Log through `tracing`; the
  version output is an explicit `writeln!` to stdout.
- **Exhaustive matches:** `wildcard_enum_match_arm` is denied: a match on an enum names every
  variant.
- **Descriptive names:** `min_ident_chars` refuses single-letter names except `x` and `y`. Prefer
  `index`, `state`, `timezone`, `client_id`.
- **Item order:** public items before private ones within an `impl`.
- **Tests:** unit tests sit in a `#[cfg(test)] mod tests` beside the code (or a `tests.rs` in the
  module directory). Timers are driven by the paused tokio clock (`#[tokio::test(start_paused =
  true)]`), not by sleeping. A test ported from Go carries a doc comment naming the Go function it
  replaces (``/// Go: `TestJobFiresOnceThenReschedules` ``).

## Conventions worth knowing

- **Clock discipline:** `clippy.toml`'s `disallowed-methods` bans `jiff::Timestamp::now`,
  `jiff::Zoned::now`, `std::time::SystemTime::now` and `std::time::Instant::now`, and its
  `disallowed-types` bans `clock::SystemClock`. Scheduling and window code reads the time from the
  injected `src/clock` clock (rust-common's `clock`, re-exported without `SystemClock`), so the
  tests can drive it, through `run::Options::clock`; only the production wiring in `src/run`
  (`#[expect(clippy::disallowed_types, reason = "...")]` on its `use`) and the debug weather
  simulation (`#[expect(clippy::disallowed_methods, reason = "...")]`) read the wall clock.
- **Daylight saving:** every "local date and wall time to instant" conversion goes through
  `clock::resolve_local` (a gap moves forward by its length, a fold takes the earlier instant). No
  other module calls jiff's disambiguation directly.
- **Version strings:** `src/version.rs` reads `AWTRIX_CONTROLLER_VERSION`,
  `AWTRIX_CONTROLLER_COMMIT` and `AWTRIX_CONTROLLER_DATE` at compile time (service-kit's
  `buildinfo!`), falling back to `dev`, `none` and `unknown` when unset (an empty stamp stays
  empty). `make rust-dist` and `make rust-run` set them from git, the date from HEAD's commit
  (`RUST_STAMP_PREFIX` and `BUILD_DATE` in the Makefile, for make-common's `.mk/rust.mk`;
  `make rust-build` builds unstamped, as the tests do, through `RUST_BUILD_ENV :=`);
  `make dist-static` from `DIST_VERSION`, `DIST_COMMIT` and `DIST_DATE`, which the release sets to
  the tag, the commit's first seven characters and its commit date; and the Dockerfile's build stage
  from its `VERSION`, `COMMIT` and `DATE` arguments, which also fill the image labels.
- Every Rust and shell file, the Dockerfile, the Makefile and the repo-local `.mk` fragments carry
  the MIT license header, with the year of the file's first commit (copy it from any Rust or shell
  file, or use the IntelliJ MIT copyright profile). The `check-license-headers` pre-commit hook
  (`scripts/check-license-headers.sh`) checks the whole tree; the make-common files are exempt.
- A new `#[expect]`, `# shellcheck disable=` or `# droast ignore=` explains why the fix does not
  apply here, not which rule fired (droast also refuses one without a `reason="..."`).

## GitHub conventions

Shared by every `leinardi/*` repository.

- **Jobs:** every job sets `timeout-minutes` (a composite action cannot time out its own steps).
  Every checkout that pushes nothing sets `persist-credentials: false`; the release's checkout
  sets it too, and its tag push passes the token to that one `git push`. Every job that holds a
  write permission or `id-token` runs in the `release` environment.
- **Concurrency:** CI groups a pull request's runs by `${{ github.workflow }} @ <head label>` and
  cancels the older one; every other run (a push to the default branch, a dispatch) gets its own
  group, `${{ github.run_id }}`, because GitHub keeps one pending run per group and the release
  gate needs CI on the exact commit (see `ci.yaml`). A warm-up uses `<name>-${{ github.ref }}`,
  never a static group. The release uses the one `release` group and is never cancelled.
- **Caches:** the default branch is the only writer, because every pull request can restore its
  caches and the default branch cannot restore a pull request's. The pre-commit cache is saved
  only by `pre-commit-warmup.yaml`; CI restores it with `actions/cache/restore` under the same key,
  `pre-commit|<OS>|<resolved Python version>|<hash of .pre-commit-config.yaml>`, so CI sets up the
  warm-up's `python_version` and installs its pre-commit version. The Rust cache is
  `Swatinem/rust-cache` with `shared-key: ci`, saved on `main` only (the `image` job under its own
  `key: static`); the release writes its own, `prefix-key: release`. No image build uses a layer
  cache: the prebuilt target is the base plus one COPY.
- **pre-commit** is pinned to the same version in CI, the warm-up and the
  `gha-pre-commit-reviewdog-actions` jobs (their `pre-commit-version` input).
- **Release:** `release.yaml` is a `workflow_dispatch` with an optional `version` and a `dry_run`,
  built from gh-composite-actions (`release-version`, `require-ci-green`, `release-mode`,
  `toolchain-record`, `toolchain-recover`, `check-pinned-bases`, `trivy-gate`,
  `binary-attestation-status`, `setup-zigbuild`, `setup-cargo-deny`). svu derives the version from
  the commits only when none is given, and is installed only then, so a version entered by hand
  releases even when GitHub's release downloads fail. Trivy is never pinned: `trivy-gate` installs
  its latest release. A push to `main` that merges a Dependabot pull request releases a patch on its
  own when only Dependabot's work is unreleased and one commit is a `fix(deps)`
  (gh-reusable-workflows' `dependabot-release` with `ecosystems: docker cargo`); every other push
  stops at that job.
- **Dependabot:** monthly, `cooldown: default-days: 7`, labels `dependencies` and `ci`, one group
  per ecosystem. The shipped ecosystems use `fix(deps)`: `cargo` (with `prefix-development:
  chore(deps)` and separate production and development groups) and `docker` (the runtime base's
  tag); GitHub Actions and pre-commit use `chore(deps)`.
- **Issue and pull request templates:** keep the Step 1/2/3 structure of template-default, with
  this repository's project name, version command, environment block, snippet language and extra
  checkboxes. The pull request checklist always keeps the Conventional Commits and
  validation-matrix items.

## Project skills

Skills live in `.agents/skills/` (symlinked as `.claude/skills`). Load them before the work, not
after review:

- `adversarial-review` — for any review request ("review my diff", "is this ready to merge").

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
