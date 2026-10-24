# Tech stack

- Go (single version line pinned across go.mod, Dockerfile build image, CI; `test/toolchain`
  enforces one value). CGO disabled, static binary.
- Deps: `wind-c/comqtt/v2` (embedded broker; pulls prometheus + quic-go transitively),
  `mstephenholl/go-solar` (sunrise/sunset, computes on the UTC date), `gopkg.in/yaml.v3`,
  `eclipse/paho.mqtt.golang` (tests only).
- Lint: golangci-lint v2 with `default: all`, pinned in pre-commit. Vulnerabilities: govulncheck
  (pinned in `.mk/audit-deps.mk`).
- Build/infra: Makefile + `.mk/*.mk`; make-common snippets
  (`.mk/{docker,help,go,password,pre-commit}.mk`) are vendored from `leinardi/make-common` — never
  hand-edit, refresh with `make mk-common-update`.
- Image: `dhi.io` hardened bases (golang build stage, `dhi.io/static` runtime), multi-arch
  amd64/arm64, `# syntax` frontend pinned by digest. Pulling needs a `dhi.io` login.
- Release: svu derives the version from Conventional Commits; cosign keyless signing + provenance
  attestations (see `docs/release.md`).
