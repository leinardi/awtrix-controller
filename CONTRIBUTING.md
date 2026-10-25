# Contributing

## Prerequisites

- Rust 1.98: [rustup](https://rustup.rs/) installs the `1.98.0` toolchain that
  `rust-toolchain.toml` pins, with clippy and rustfmt, on the first `cargo` command
- [cargo-deny](https://github.com/EmbarkStudios/cargo-deny), for `make audit-deps` and
  `make rust-deny` (`cargo install --locked cargo-deny`)
- [cargo-zigbuild](https://github.com/rust-cross/cargo-zigbuild) and [zig](https://ziglang.org/),
  only to build the static musl binaries the releases ship outside Docker (with the target added by
  `rustup target add x86_64-unknown-linux-musl`)
- Docker, for building the container image
- [Dockerfile Roast](https://github.com/immanuwell/dockerfile-roast) (`droast`): pre-commit builds
  its own with `cargo` at the pinned rev, but the Dockerfile meta-tests run the `droast` on `PATH`
  (the release `.github/actions/setup-droast` pins; they skip without it, unless
  `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1`)
- [ShellCheck](https://github.com/koalaman/shellcheck) on `PATH`, at the release
  `.github/actions/setup-shellcheck` pins: droast's ShellCheck bridge is required, so the droast
  hook and the Dockerfile meta-tests need it
- [`pre-commit`](https://pre-commit.com/): install the hooks once with `make pre-commit-install`. It
  installs both the `pre-commit` and the `commit-msg` hooks, so commit messages are checked when you
  commit, not when the release runs.

## Building and testing

```bash
make rust-build      # cargo build of the whole workspace, every target
make rust-test       # cargo test --workspace, the meta-tests in meta/ included
make rust-fmt        # format the code (rust-fmt-check only checks)
make rust-clippy     # clippy over the workspace, warnings as errors
make verify          # rust-build, rust-fmt-check, rust-clippy and rust-test in one go
make rust-run ARGS="--config config.yaml"  # run with the version strings set from git
make audit-deps      # cargo-deny against the RustSec advisories (network required)
make rust-deny       # every cargo-deny check: advisories, licences, bans, sources (network required)
make docker-build    # the container image, for the host's architecture
```

## Linting

```bash
make check         # the full pre-commit suite on every file (rustfmt, clippy, droast, markdownlint, shellcheck, ...)
make check-stage   # the same, on the staged files only
```

CI runs the same hooks, the tests and cargo-deny on every pull request and every push to `main`
(`.github/workflows/ci.yaml`).

Every source file carries the MIT license header (the `check-license-headers` hook), and every
`#[expect(...)]`, `#[allow(...)]`, `# shellcheck disable=` or `# droast ignore=` is counted against
`.suppressions-baseline` (the `check-suppressions` hook): see
[AGENTS.md](AGENTS.md#suppressions-are-ratcheted).

## Branches and pull requests

- Branch names: `feat/<short-description>`, `fix/<short-description>`, `chore/<short-description>`.
- Keep pull requests focused: one logical change each.
- All CI checks must pass before merge.

## Commit messages

All commits must follow [Conventional Commits 1.0.0](https://www.conventionalcommits.org/en/v1.0.0/)
with a scope: `<type>(<scope>)[!]: <description>`. The `conventional-pre-commit` hook enforces this
on `commit-msg`, and the `conventional-commits` CI job checks it again on every pull request.
Release notes are not built from these messages: `gh release create --generate-notes` lists the
merged pull requests by title.

Common types: `feat`, `fix`, `docs`, `test`, `refactor`, `perf`, `build`, `ci`, `chore`, `style`,
`revert`. Use a lower-case, imperative description:

```text
fix(scheduler): fire yearly notifications on leap days
feat(weather): add a wind overlay
feat(config)!: rename the energy_saving keys
```

Mark breaking changes with `!` before the colon or a `BREAKING CHANGE: <description>` footer.

## Versioning

The release version is derived from these types: since the last release, any `feat` makes the next
release a minor, any `fix` a patch, and `!` or a `BREAKING CHANGE:` footer a major; `build`,
`chore`, `ci`, `docs`, `perf`, `refactor`, `revert`, `style` and `test` bump nothing. See
[`docs/release.md`](docs/release.md).

Pick the type by whether the change should ship, not by what kind of change it is. A performance
improvement, a refactor or a revert that changes the shipped binary or image and that users should
receive is a `fix` (or a `feat`). Use `perf`, `refactor`, `style` and `revert` only when the commit
is deliberately not meant to trigger a release on its own.

Pull requests are merged with merge commits; squash and rebase merging are disabled. Every commit in
a pull request therefore lands on `main` as it is and counts toward the version, so each commit
needs a correct type, not just the pull request as a whole. The `conventional-commits` CI job checks
every one of them. Enabling squash merging would make the pull-request title the commit subject
instead, and would need a CI check on pull-request titles first.

## Reporting security issues

Privately, through GitHub's security advisories: see [SECURITY.md](SECURITY.md).
