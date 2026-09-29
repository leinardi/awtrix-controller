# Security policy

## Supported versions

Only the latest release gets security fixes. A fix ships as a new version: released tags and image tags are immutable, so an
existing version is never rebuilt or replaced. Upgrade to the latest release, or follow `ghcr.io/leinardi/awtrix-controller:latest` or the
`:<major>` tag.

## Reporting a vulnerability

Report it privately through GitHub's
[private vulnerability reporting](https://github.com/leinardi/awtrix-controller/security/advisories/new), not in a public issue, a pull
request or a discussion. Include the version or image digest, how the broker is exposed (TCP and WebSocket ports, network), and the steps or input that trigger the problem.

This is a project maintained in spare time, so reports are handled on a best-effort basis. You will get an answer in the advisory,
and the fix, once released, is credited there unless you prefer otherwise.

## Scope

In scope: the awtrix-controller binary, the container image published to GHCR, the release artifacts, and the workflows that build and
publish them.

Out of scope: vulnerabilities in the Awtrix3 firmware, the comqtt broker library and Open-Meteo and in the base images, which are reported upstream. A base image or dependency
vulnerability that affects a published release is still worth reporting here if the weekly scheduled scan has not caught it.

## Security model

- **One shared MQTT credential.** The embedded broker accepts a client only with the configured `mqtt.username` and
  `mqtt.password`, and every authenticated client may publish and subscribe to every topic: any holder of the credential can
  drive every connected device. Change the sample password before deploying, and treat the config file as a secret.
- **No TLS on the broker.** The TCP and optional WebSocket listeners speak plain MQTT, as Awtrix3 devices do, so the credential
  and every message cross the network in clear text. Run the controller and the devices on a trusted network segment, and do not
  expose the broker ports to the internet.
- **Outbound traffic.** The controller calls the Open-Meteo forecast API over HTTPS for weather overlays, sending the configured
  coordinates and timezone and nothing else.
- **Supply chain.** Every GitHub Action is pinned to a commit SHA and every image the workflows and the Dockerfile use to an index
  digest. Each release image is scanned for `HIGH` and `CRITICAL` vulnerabilities before its version is tagged, and the latest
  release is scanned again every week, together with the Go dependencies. The release process is described in
  [docs/release.md](docs/release.md).

## Verifying a release

Release images carry a build provenance attestation and a keyless cosign signature from the release workflow. Verify an image by
digest before you trust it:

```bash
IMAGE=ghcr.io/leinardi/awtrix-controller
DIGEST=sha256:...   # from `docker buildx imagetools inspect $IMAGE:<version>`

gh attestation verify "oci://$IMAGE@$DIGEST" --repo leinardi/awtrix-controller \
  --signer-workflow leinardi/awtrix-controller/.github/workflows/release.yaml --source-ref refs/heads/main

cosign verify "$IMAGE@$DIGEST" \
  --certificate-identity https://github.com/leinardi/awtrix-controller/.github/workflows/release.yaml@refs/heads/main \
  --certificate-oidc-issuer https://token.actions.githubusercontent.com
```

The signature is stored as a Sigstore bundle in an OCI 1.1 referring artifact, so `cosign verify` needs cosign v3, or v2.6 or later
with `--new-bundle-format`.

The release binaries carry a build provenance attestation from the same workflow. Verify a downloaded binary before you run it:

```bash
gh attestation verify awtrix-controller-linux-amd64 --repo leinardi/awtrix-controller \
  --signer-workflow leinardi/awtrix-controller/.github/workflows/release.yaml --source-ref refs/heads/main
```

Releases up to and including v1.1.2 predate this release workflow and carry neither.
