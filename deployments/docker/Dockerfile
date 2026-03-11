# syntax=docker/dockerfile:1.7

########################
# deps: cache modules
########################
FROM golang:1.26.1-alpine AS deps
WORKDIR /src
RUN --mount=type=cache,target=/var/cache/apk \
	apk add --no-cache ca-certificates git
COPY go.mod go.sum ./
# Use a named cache so we can reuse it in the build stage (no COPY needed)
RUN --mount=type=cache,target=/go/pkg/mod,id=gomodcache \
	go mod download

########################
# build: compile static binary
########################
FROM golang:1.26.1-alpine AS build
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
RUN --mount=type=cache,target=/var/cache/apk \
	apk add --no-cache ca-certificates

# Bring in source
COPY . .

# Reuse the same module & build caches (BuildKit)
RUN --mount=type=cache,target=/go/pkg/mod,id=gomodcache \
	--mount=type=cache,target=/root/.cache/go-build \
	go build -trimpath -buildvcs=false \
		-ldflags="-s -w -X 'main.version=${VERSION}' -X 'main.commit=${COMMIT}' -X 'main.date=${DATE}'" \
		-o /out/awtrix-controller ./cmd/awtrix-controller

########################
# certs: minimal CA bundle
########################
FROM alpine:3.23.3 AS certs
RUN --mount=type=cache,target=/var/cache/apk \
	apk add --no-cache ca-certificates
# leave at /etc/ssl/certs/ca-certificates.crt

########################
# runtime: distroless static nonroot
########################
FROM gcr.io/distroless/static:nonroot

# hadolint ignore=DL3050
LABEL org.opencontainers.image.title="awtrix-controller" \
	org.opencontainers.image.description="Awtrix3 MQTT Controller" \
	org.opencontainers.image.source="https://github.com/leinardi/awtrix-controller" \
	org.opencontainers.image.licenses="MIT"

# Copy the binary and CA bundle
COPY --from=build /out/awtrix-controller /usr/local/bin/awtrix-controller
COPY --from=certs /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/

# Run as non-root (UID/GID 65532 in distroless)
USER nonroot:nonroot

# Document the MQTT port (no EXPOSE required for security, but okay to include)
EXPOSE 1883

# Distroless has no shell; use absolute path
ENTRYPOINT ["/usr/local/bin/awtrix-controller"]
