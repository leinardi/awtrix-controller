#
# MIT License
#
# Copyright (c) 2026 Roberto Leinardi
#
# Permission is hereby granted, free of charge, to any person obtaining a copy
# of this software and associated documentation files (the "Software"), to deal
# in the Software without restriction, including without limitation the rights
# to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
# copies of the Software, and to permit persons to whom the Software is
# furnished to do so, subject to the following conditions:
#
# The above copyright notice and this permission notice shall be included in all
# copies or substantial portions of the Software.
#
# THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
# IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
# FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
# AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
# LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
# OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
# SOFTWARE.
#

# Resolve repository root (Makefile can live anywhere)
REPO_ROOT := $(shell git rev-parse --show-toplevel 2>/dev/null || pwd)

MK_COMMON_REPO        ?= leinardi/make-common
MK_COMMON_VERSION     ?= v1

MK_COMMON_DIR         := $(REPO_ROOT)/.mk

# Shared snippets coming from make-common
MK_COMMON_FILES       := docker.mk help.mk password.mk pre-commit.mk rust.mk rust-release.mk image.mk

# Repo-local snippets that are NOT in make-common
MK_LOCAL_FILES        := docker-run.mk audit-deps.mk verify.mk

MK_COMMON_BOOTSTRAP_SCRIPT := $(REPO_ROOT)/scripts/bootstrap-mk-common.sh

# Bootstrap: the script will self-update and fetch the selected .mk snippets
MK_COMMON_BOOTSTRAP := $(shell "$(MK_COMMON_BOOTSTRAP_SCRIPT)" \
  "$(MK_COMMON_REPO)" \
  "$(MK_COMMON_VERSION)" \
  "$(MK_COMMON_DIR)" \
  "$(MK_COMMON_FILES)")

# -----------------------------------------------------------------------------
# Project-specific config
# -----------------------------------------------------------------------------
BIN_NAME     ?= awtrix-controller
DIST_DIR     ?= dist

# Rust settings for make-common's rust.mk. src/version.rs reads the AWTRIX_CONTROLLER_VERSION,
# _COMMIT and _DATE stamps. The default features are all of them: no member defines a [features]
# table. cargo-deny checks the whole workspace (rust.mk's default RUST_DENY_ARGS, --workspace), the
# meta crate's graph included.
RUST_STAMP_PREFIX := AWTRIX_CONTROLLER

# The date stamp is HEAD's commit date in UTC (rust.mk's GIT_COMMIT_DATE, the clock's only
# outside git), the date release.yaml stamps. A stamp that moves only with a commit keeps a
# repeated build a cache hit: a new date on every make would recompile the crate that reads it
# each time.
BUILD_DATE ?= $(or $(GIT_COMMIT_DATE),$(shell date -u +"%Y-%m-%dT%H:%M:%SZ"))

# rust-build unstamped, as rust-test builds: the two share the awtrix-controller crate's units
# instead of each recompiling it after the other. rust-dist, rust-run and the image build stay
# stamped.
RUST_BUILD_ENV :=

# Release settings for make-common's rust-release.mk, owned by this repository: `:=`, so an
# environment variable cannot change them (a command-line assignment still can). The
# Dockerfile's build stage and the setup-zigbuild steps of CI and the release carry copies of the
# zig, cargo-zigbuild and rust-std pins, which meta/tests/toolchain.rs keeps equal to these.
# checksums.txt covers both shipped platforms even when a release recovery rebuilds only some
# (RUST_RELEASE_PLATFORMS).
RUST_RELEASE_ZIG_VERSION := 0.17.0
RUST_RELEASE_ZIG_SHA256_x86_64 := 1cbe9df9f27e6b78d14ccbca43b6703a404ef79ef1c463de901d7f088d4e2026
RUST_RELEASE_ZIG_SHA256_aarch64 := 9e8d11661d4ae3bd57702a3832781e23ad151dde5798e16a5ccd503f65234ff8
RUST_RELEASE_CARGO_ZIGBUILD_VERSION := 0.24.0
RUST_RELEASE_CARGO_ZIGBUILD_SHA256_x86_64-unknown-linux-musl := 52be2cb85ad3875682564a29bac1832b2ab3db68b1c504765c92d5249cc9b96f
RUST_RELEASE_CARGO_ZIGBUILD_SHA256_aarch64-unknown-linux-gnu := de42626985f428033078dfc12ac8a59ee713f8e5627235ec883b1c899bee895c
RUST_RELEASE_RUST_STD_VERSION := 1.98.1
RUST_RELEASE_RUST_STD_SHA256_x86_64-unknown-linux-musl := bd9f35880de21dab0e387e84009c00001ae7836a4ba565cb16823f13f75f325f
RUST_RELEASE_RUST_STD_SHA256_aarch64-unknown-linux-musl := 7e0ad2e67817f224b7a6331e48d6397303fc050edaccf81784ed786ffddc884a
RUST_RELEASE_TRIPLE_linux_amd64 := x86_64-unknown-linux-musl
RUST_RELEASE_TRIPLE_linux_arm64 := aarch64-unknown-linux-musl
RUST_RELEASE_SHIPPED_PLATFORMS := linux/amd64 linux/arm64
RUST_RELEASE_DIR := $(REPO_ROOT)/$(DIST_DIR)

# Docker build settings
DOCKERFILE     ?= deployments/docker/Dockerfile
DOCKER_CONTEXT ?= .

IMAGE_NAME   ?= $(BIN_NAME)
IMAGE_REPO   ?= $(IMAGE_NAME)
IMAGE_TAG    ?= $(GIT_VERSION)

# Drop-in runtime defaults (matches old `run`/`docker-run`)
ARGS ?= --config $(REPO_ROOT)/deployments/config.sample.yaml --log-level debug
DOCKER_RUN_ARGS ?= --config /etc/awtrix-controller/config.yaml

# -----------------------------------------------------------------------------
# Include shared make logic (fetched from make-common)
# -----------------------------------------------------------------------------
include $(addprefix $(MK_COMMON_DIR)/,$(MK_COMMON_FILES))

# -----------------------------------------------------------------------------
# Include repo-local logic (no bootstrap; lives only in this repo)
# -----------------------------------------------------------------------------
-include $(addprefix $(REPO_ROOT)/.mk/,$(MK_LOCAL_FILES))

.PHONY: mk-common-update
mk-common-update: ## Check for remote updates of shared .mk files
	@echo "[mk] Checking for updates from $(MK_COMMON_REPO)@$(MK_COMMON_VERSION)"
	MK_COMMON_UPDATE=1 "$(MK_COMMON_BOOTSTRAP_SCRIPT)" \
	  "$(MK_COMMON_REPO)" \
	  "$(MK_COMMON_VERSION)" \
	  "$(MK_COMMON_DIR)" \
	  "$(MK_COMMON_FILES)"

# -----------------------------------------------------------------------------
# Adding new targets
# -----------------------------------------------------------------------------
# Do NOT add recipes directly to this file. Instead:
#   - Project-specific targets -> new .mk/<fragment>.mk added to MK_LOCAL_FILES
#   - Generic targets (useful beyond this repo) -> new or updated .mk/<fragment>.mk
#     added to MK_COMMON_FILES, then open a PR to port the change upstream at
#     https://github.com/leinardi/make-common so mk-common-update keeps working.
