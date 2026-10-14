# syntax=docker/dockerfile:1.7@sha256:a57df69d0ea827fb7266491f2813635de6f17269be881f696fbfdf2d83dda33e

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
# The syntax line above pins the BuildKit frontend by index digest, so a moved 1.7 tag cannot
# change how this file is built. It is a parser directive and has to be the file's first line, ahead
# of this header. Dependabot's docker ecosystem does not update it: bump it by hand
# (docs/release.md).
#
# Base images are pinned by tag, not by digest: dhi.io republishes its tags
# often to ship security fixes, and a rebuild should pick those up. Every
# release build is scanned, so a newer base never ships unscanned.

########################
# build: compile static binary
########################
FROM --platform=$BUILDPLATFORM dhi.io/golang:1.26.9-alpine3.23-dev AS build
ARG VERSION="dev"
ARG COMMIT="none"
ARG DATE="unknown"

# Buildx provides these automatically for multi-arch builds
ARG TARGETOS
ARG TARGETARCH

ENV CGO_ENABLED=0 \
    GOOS=${TARGETOS} \
    GOARCH=${TARGETARCH}

WORKDIR /src

# Modules first, so a source change does not refetch them. This used to be a separate deps stage,
# but nothing referenced it, so BuildKit never built it.
COPY go.mod go.sum ./
RUN --mount=type=cache,target=/go/pkg/mod,id=gomodcache \
	go mod download

# Bring in source
COPY . .

# Reuse the same module & build caches (BuildKit)
RUN --mount=type=cache,target=/go/pkg/mod,id=gomodcache \
	--mount=type=cache,target=/root/.cache/go-build \
	go build -trimpath -buildvcs=false \
		-ldflags="-s -w -X 'main.version=${VERSION}' -X 'main.commit=${COMMIT}' -X 'main.date=${DATE}'" \
		-o /out/awtrix-controller ./cmd/awtrix-controller

########################
# runtime: dhi.io hardened static base (distroless-style, no shell).
# The daemon binary is CGO_ENABLED=0 so no libc is required.
########################
FROM dhi.io/static:20250419 AS runtime
ARG VERSION="dev"
ARG COMMIT="none"
ARG DATE="unknown"

# The seven OCI labels .hadolint.yaml requires; version, revision and created come from the same
# build arguments the binary is stamped with.
LABEL org.opencontainers.image.title="awtrix-controller" \
	org.opencontainers.image.description="Awtrix3 MQTT Controller" \
	org.opencontainers.image.source="https://github.com/leinardi/awtrix-controller" \
	org.opencontainers.image.version="${VERSION}" \
	org.opencontainers.image.revision="${COMMIT}" \
	org.opencontainers.image.created="${DATE}" \
	org.opencontainers.image.licenses="MIT"

# Copy the binary
COPY --from=build /out/awtrix-controller /usr/local/bin/awtrix-controller

# dhi.io/static defaults to a non-root user; the exporter does not need root.

# Document the MQTT port
EXPOSE 1883

ENTRYPOINT ["/usr/local/bin/awtrix-controller"]
