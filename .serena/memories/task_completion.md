# Task completion

Run the rows of the AGENTS.md validation matrix that match the edited paths; `make check` applies to
every row. Most common:

- `**/*.rs`: `make verify`, then `make check-stage`.
- `Cargo.toml`/`Cargo.lock`/`deny.toml`: `make verify`, `make rust-deny`,
  `cargo test --locked -p meta --test toolchain --test workspace_lints`.
- Dockerfile/.hadolint/.dockerignore: `cargo test --locked -p meta --test dockerfiles --test
  toolchain`, `make docker-build`.
- `.github/**`: `pre-commit run actionlint --all-files`, `cargo test --locked -p meta --test
  toolchain --test dockerfiles`.
- `meta/**`: `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1 cargo test --locked -p meta`.
- `**/*.md`: `cargo test --locked -p meta --test docs`; prose wraps at 100 columns.
- New file: MIT license header with the year of first commit (`scripts/check-license-headers.sh`).
