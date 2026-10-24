# AGENTS.md

## What this is

A self-contained MQTT broker and automation controller for
[Awtrix3](https://blueforcer.github.io/awtrix3/) LED matrix clocks. Devices connect to its embedded
broker (comqtt); the controller then pushes day/night themes and display settings, runs an
energy-saving window, fires scheduled notifications and overlays the Open-Meteo weather forecast.
The README and `deployments/config.sample.yaml` are the user-facing contract: configuration keys,
CLI flags and behaviour.

## Common commands

```bash
make go-build      # CGO_ENABLED=0 build into ./dist/
make go-test       # go test -race ./... (the meta-tests under test/ included)
make go-vet
make go-lint       # golangci-lint v2, all linters, at the pre-commit pin, over the whole module
make verify-go     # go-build, go-test, go-vet and go-lint
make go-tidy       # go mod tidy + go mod verify
make audit-deps    # govulncheck -test over every package (network required); also runs in CI and as a pre-commit hook on go.mod/go.sum changes
make check         # pre-commit on all files: lint, formatters, tests
make check-stage   # pre-commit on the staging area only
make docker-build
make docker-run
```

Before calling a change done, run the rows of the [validation matrix](#validation-matrix) that
match it.

Single test:

```bash
go test ./internal/scheduler -run TestJobFiresOnceThenReschedules -v
```

The Makefile pulls shared snippets from `leinardi/make-common@v1` into `.mk/` on first run. To
refresh: `make mk-common-update`. Project targets live in the local `.mk/*.mk` files listed in
`MK_LOCAL_FILES`, never as recipes in the Makefile.

## Layout

- `cmd/awtrix-controller/` — flags, wiring (`run.go`) and the debug-only test-notification mode
  (`testnotification.go`).
- `internal/broker/` — the embedded comqtt broker: TCP and optional WebSocket listeners, and the
  hook that authenticates clients and reacts to connects.
- `internal/clientstate/` — the thread-safe registry of connected clients.
- `internal/config/` — loading and validating the YAML configuration, with defaults.
- `internal/daynight/`, `internal/energysaving/` — sunrise/sunset from the configured coordinates,
  and the nightly window.
- `internal/settings/`, `internal/notification/`, `internal/weather/` — what is published to the
  devices: settings and themes, scheduled notifications, forecast overlays and severe-weather
  alerts.
- `internal/scheduler/`, `internal/clock/` — the recurring-job scheduler and the injectable clock
  the tests drive.
- `internal/model/` — the JSON payloads Awtrix3 accepts.
- `internal/logger/` — the slog singleton.
- `deployments/` — the Dockerfile (`dhi.io` bases pinned by tag, the `# syntax` frontend by
  digest), a compose example and the sample config.
- `docs/release.md` — how a release is cut and recovered.
- `test/` — meta-tests, all ordinary Go packages run by `go test ./...`: `test/toolchain` (one Go
  line, the hadolint and govulncheck pins, no vacuous flags), `test/dockerfiles` (the image's
  stages, labels and frontend, and hadolint's label schema), `test/docs` (Markdown links),
  `test/licenseheaders` and `test/suppressions` (the two scripts below), and `test/internal/`
  (`gitenv` isolates scratch-repository git commands, `toolgate` finds hadolint).
- `scripts/` — `check-license-headers.sh`, `check-suppressions.sh`, `hooks/gofmt-on-edit.sh` (the
  Claude Code edit hook in `.claude/settings.json`) and make-common's `bootstrap-mk-common.sh`.

### Generated or vendored: never hand-edit

- `.mk/{docker,help,go,password,pre-commit}.mk` and `scripts/bootstrap-mk-common.sh`: fetched from
  make-common; refresh them with `make mk-common-update`, and port a change upstream.
- `.suppressions-baseline`: written by `scripts/check-suppressions.sh --update`.

## Validation matrix

Run before considering an edit done. `make check` applies to every row.

| You edited | Run |
| --- | --- |
| `**/*.go` | `make verify-go`, then `make check-stage` |
| `go.mod`, `go.sum` | `make go-tidy`, `make audit-deps` (network; the commit hook runs it too), `go test ./test/toolchain/` |
| `deployments/docker/Dockerfile`, `.hadolint.yaml`, `.dockerignore` | `go test ./test/dockerfiles/ ./test/toolchain/` (hadolint runs in `make check` and in `test/dockerfiles`), `make docker-build` (needs a `dhi.io` login) |
| `.golangci.yaml` | `make go-lint` |
| `scripts/check-suppressions.sh`, `.suppressions-baseline` | `go test ./test/suppressions/` |
| `scripts/check-license-headers.sh` | `go test ./test/licenseheaders/` |
| `scripts/hooks/gofmt-on-edit.sh` | shellcheck (in `make check`); run it once by editing a `.go` file in Claude Code |
| `.github/scripts/binary-attestation-status.sh` | `bash .github/scripts/binary-attestation-status.test.sh` (also the `release-scripts` CI job) |
| `.github/workflows/**`, `.github/actions/**` | `pre-commit run actionlint --all-files`, `go test ./test/toolchain/ ./test/dockerfiles/` |
| `.pre-commit-config.yaml`, `Makefile`, `.mk/**` | `make check`, `go test ./test/toolchain/`, `make help` |
| `test/**` | `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1 go test -count=1 ./test/...` |
| `**/*.md` | `go test -count=1 ./test/docs/`; prose wraps at 100 columns (markdownlint) |

## Go coding rules

These are written out in full in `.agents/skills/go-style-guide/SKILL.md`; the short version:

- **Variable names:** always descriptive. Prefer `index`, `state`, `waitGroup`, `timezone`,
  `clientID`.
- **Never shadow Go builtins** as variables (`copy`, `len`, `cap`, `new`, `close`, `delete`,
  `append`, `error`).
- **Tests:** every top-level `Test*` starts with `t.Parallel()`.
- **Method order:** exported methods before unexported ones for the same type.
- **Enum switches:** an explicit `case` for every enum value, not only a `default`.
- **Error handling:** no inline `if err := ...; err != nil`. Use separate statements and unique
  names (`hookErr`, `tcpErr`).
- **Receivers:** stateless methods use an unnamed receiver: `func (*Type) Method()`.

## Conventions worth knowing

- Scheduling and window code reads the time from `internal/clock`, never from `time.Now`, so the
  tests can drive it (only the debug weather simulation in `internal/weather/simulate.go` reads the
  wall clock).
- Version strings (`version`, `commit`, `date`) live in `cmd/awtrix-controller/version.go` and are
  filled by `-ldflags -X main.version=...` from `GO_LDFLAGS` in `.mk/go.mk`.
- Every Go and shell file, the Dockerfile, the Makefile and the repo-local `.mk` fragments carry
  the MIT license header, with the year of the file's first commit (copy it from any Go or shell
  file, or use the IntelliJ MIT copyright profile). The `check-license-headers` pre-commit hook
  (`scripts/check-license-headers.sh`) checks the whole tree; the make-common files are exempt.
- A new `//nolint`, `# shellcheck disable=` or `# hadolint ignore=` explains why the fix does not
  apply here, not which rule fired.
- `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1` (set by the `go-test` CI job) turns a missing hadolint into a
  meta-test failure instead of a skip.

## Suppressions are ratcheted

Every in-file suppression is counted per rule against `.suppressions-baseline` by the
`check-suppressions` pre-commit hook (`scripts/check-suppressions.sh`): `//nolint:<linter>`,
`# shellcheck disable=`, `# hadolint ignore=` and `# hadolint global ignore=`.

Adding one is allowed, but never silently: the hook refuses a commit that adds a suppression line
without changing the baseline in the same commit, including a swap or a moved file that leaves the
count unchanged. Prefer fixing the finding. If a suppression really is right, write it with a reason
(`//nolint:<linter> // reason`), stage it, run `scripts/check-suppressions.sh --update`, and commit
the baseline alongside it. A count that has fallen below its baseline fails the same way, with the
same fix: the file describes the repository, not a past one.

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
