# Task completion

Run the rows of the AGENTS.md validation matrix that match the edited paths; `make check` applies to
every row. Most common:

- `**/*.go`: `make verify-go`, then `make check-stage`.
- `go.mod`/`go.sum`: `make go-tidy`, `make audit-deps`, `go test ./test/toolchain/`.
- Dockerfile/.hadolint/.dockerignore: `go test ./test/dockerfiles/ ./test/toolchain/`, `make
  docker-build`.
- `.github/**`: `pre-commit run actionlint --all-files`, `go test ./test/toolchain/
  ./test/dockerfiles/`.
- `test/**`: `AWTRIX_CONTROLLER_REQUIRE_TOOLS=1 go test -count=1 ./test/...`.
- `**/*.md`: `go test -count=1 ./test/docs/`; prose wraps at 100 columns.
- New file: MIT license header with the year of first commit (`scripts/check-license-headers.sh`).
