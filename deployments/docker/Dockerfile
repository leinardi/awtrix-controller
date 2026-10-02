# syntax=docker/dockerfile:1.7

# Every base image is pinned by index digest: the release scans the image it
# builds and a recovery re-run rebuilds it, so a tag that moved in between would
# ship something nobody scanned. Dependabot bumps the tag and the digest together.

########################
# deps: cache modules
########################
FROM --platform=$BUILDPLATFORM dhi.io/golang:1.26.8-alpine3.23-dev@sha256:e248fe1cc163930de8df06e96241f29430998f088b981e479f00e1d6dc47f38c AS deps
WORKDIR /src
RUN --mount=type=cache,target=/var/cache/apk \
	apk add --no-cache git
COPY go.mod go.sum ./
# Use a named cache so we can reuse it in the build stage (no COPY needed)
RUN --mount=type=cache,target=/go/pkg/mod,id=gomodcache \
	go mod download

########################
# build: compile static binary
########################
FROM --platform=$BUILDPLATFORM dhi.io/golang:1.26.8-alpine3.23-dev@sha256:e248fe1cc163930de8df06e96241f29430998f088b981e479f00e1d6dc47f38c AS build
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
FROM dhi.io/static:20250419@sha256:98ef7a853608577e8d66dad1d25ada75d745d782f28d84e9ecfb85dfeb1f9c98

# hadolint ignore=DL3050
LABEL org.opencontainers.image.title="awtrix-controller" \
	org.opencontainers.image.description="Awtrix3 MQTT Controller" \
	org.opencontainers.image.source="https://github.com/leinardi/awtrix-controller" \
	org.opencontainers.image.licenses="MIT"

# Copy the binary
COPY --from=build /out/awtrix-controller /usr/local/bin/awtrix-controller

# dhi.io/static defaults to a non-root user; the exporter does not need root.

# Document the MQTT port
EXPOSE 1883

ENTRYPOINT ["/usr/local/bin/awtrix-controller"]
