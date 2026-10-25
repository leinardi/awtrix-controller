# Suggested commands

- `make help` — lists every target (project targets live in `.mk/*.mk`, never as Makefile recipes).
- `make rust-build`, `make rust-test`, `make rust-clippy`, `make rust-fmt` / `rust-fmt-check`;
  `make verify` (build, fmt check, clippy, tests).
- Single test: `cargo test --locked -p awtrix-controller --lib <module>::tests::<name>`; an
  integration file: `cargo test --locked -p awtrix-controller --test broker`; a meta-test:
  `cargo test --locked -p meta --test docs`.
- Meta-tests requiring tools: `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1 cargo test --locked -p meta`
  (missing droast or shellcheck becomes a failure instead of a skip).
- `make check` (pre-commit, all files) / `make check-stage` (staged only).
- `make audit-deps` (cargo deny advisories, network), `make rust-deny` (all cargo-deny checks).
- `make rust-run ARGS="--config config.yaml"` (version env vars from git).
- `make docker-build` / `make docker-run` (dhi.io login required).
- Suppression baseline refresh: `scripts/check-suppressions.sh --update` (commit the baseline with
  the suppression).
- Workflow lint: `pre-commit run actionlint --all-files`.
