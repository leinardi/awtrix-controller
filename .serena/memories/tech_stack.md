# Tech stack

- Rust (one version line across `rust-toolchain.toml`, the `Cargo.toml` `rust-version`, the
  Dockerfile's `dhi.io/rust` tag; CI installs it from `rust-toolchain.toml` with
  `rustup toolchain install`; `meta/tests/toolchain.rs` enforces it).
  Static musl binaries via cargo-zigbuild + zig.
- Crates: tokio + tokio-util, rmqtt + rmqtt-retainer (embedded broker), rumqttc (tests only),
  serde/serde_json/serde_yaml_ng, jiff (bundled tzdb) + sunrise, ureq with rustls/ring/webpki-roots,
  tracing + tracing-subscriber, clap, thiserror. From leinardi/rust-common (git, pinned by tag;
  see AGENTS.md "Shared crates"): clock, service-kit, tracing-setup (its `log-bridge` feature
  bridges the `log` facade), and repo-meta + testenv-core for the meta-tests.
- Lint: `[workspace.lints]` (clippy pedantic + restriction lints) and `clippy.toml`; rustfmt.
  Dependencies: cargo-deny (`deny.toml`).
- Build/infra: Makefile + `.mk/*.mk`; make-common snippets
  (`.mk/{docker,help,password,pre-commit}.mk`) are vendored from `leinardi/make-common` — never
  hand-edit, refresh with `make mk-common-update`.
- Image: `dhi.io` hardened bases (`dhi.io/rust` debian build stage, `dhi.io/static` runtime),
  multi-arch amd64/arm64, `# syntax` frontend pinned by digest. Pulling needs a `dhi.io` login.
- Release: svu derives the version from Conventional Commits; cosign keyless signing + provenance
  attestations (see `docs/release.md`).
