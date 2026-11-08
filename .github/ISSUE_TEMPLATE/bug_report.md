---
name: Bug report
about: Report a bug or unexpected behavior
title: "[Bug]: "
labels: ["bug"]
assignees: []
---
## Step 1: Are you in the right place?

* [ ] I have checked that there are no duplicate active or recent issues (bugs, questions, or
  feature requests) describing this problem.
* [ ] I am using the latest release (the binary, or `ghcr.io/leinardi/awtrix-controller:latest`),
  or have tested a build of `main`.
* [ ] This is not a security vulnerability (those go through the private process in SECURITY.md).
* [ ] This is about awtrix-controller, not about the Awtrix3 firmware itself.

## Step 2: Describe your environment

<!-- The version: `awtrix-controller --version`, or for the image
`docker run --rm ghcr.io/leinardi/awtrix-controller:<tag> --version` -->

* awtrix-controller version (`awtrix-controller --version`): `?`
* How it runs (release binary, image tag and digest, own build) and where (Docker Compose, Docker
  Swarm, systemd, other): `?`
* Host operating system and architecture: `?`
* Awtrix3 firmware version and device: `?`

## Step 3: Describe the problem

### Steps to reproduce

1. ---
2. ---
3. ---

### Observed results

<!-- What happened? This could be a description, error message, or log output. -->

*

### Expected results

<!-- What did you expect to happen? -->

*

### Relevant code, configuration, or data

<!-- Paste the smallest configuration that reproduces the issue. Never paste a secret: replace
the MQTT password with a placeholder. -->

```yaml
# Example snippet (replace with your own)
```

### Error / log output

<!-- If you are getting an error, paste the output here. -->

```text
<paste logs or error messages here>
```

<!-- Adding screenshots, screen recordings, or additional context is always helpful. -->
