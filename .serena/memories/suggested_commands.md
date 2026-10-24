# Suggested commands

- `make help` — lists every target (project targets live in `.mk/*.mk`, never as Makefile recipes).
- `make go-build` (into `./dist/`), `make go-test` (`go test -race ./...`), `make go-vet`, `make
  go-lint`, `make verify-go` (all four).
- Single test: `go test ./internal/scheduler -run TestName -v`.
- Meta-tests requiring tools: `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1 go test -count=1 ./test/...`
  (missing hadolint becomes a failure instead of a skip).
- `make check` (pre-commit, all files) / `make check-stage` (staged only).
- `make audit-deps` (govulncheck, network), `make go-tidy`.
- `make docker-build` / `make docker-run` (dhi.io login required).
- Suppression baseline refresh: `scripts/check-suppressions.sh --update` (commit the baseline with
  the suppression).
- Workflow lint: `pre-commit run actionlint --all-files`.
