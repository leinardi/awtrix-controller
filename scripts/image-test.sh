#!/usr/bin/env bash
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

# image-test.sh — tests a built awtrix-controller image of one platform, in three tiers, each
# including the one before it:
#
#   smoke    the release's short test: the image is the platform's, its binary is byte for byte
#            the given release binary (sha256), --version prints the expected stamps, and the
#            container started with the sample configuration accepts an authenticated MQTT
#            connection (CONNACK 0).
#   reduced  CI's arm64 test, under QEMU: smoke, plus the binary's ELF machine is the platform's
#            and the binary is static (no interpreter, no shared library), the binary's mode and
#            owner in the image are 0755 root:root, and the image config (non-root user,
#            entrypoint, exposed port, labels) is what the Dockerfile writes.
#   full     CI's amd64 test: reduced, plus the container hardened as docker-compose.yaml runs it
#            (read-only root, no-new-privileges, every capability dropped) refuses a wrong
#            password (CONNACK 4 or 5), stops cleanly on SIGTERM (exit code 0), and a missing
#            configuration file fails the start with a non-zero exit code.
#
# Usage:
#   scripts/image-test.sh --tier smoke|reduced|full --image IMAGE --platform os/arch \
#     --binary FILE [--expect-version "VERSION COMMIT DATE"]
#
# IMAGE must already be in the local Docker image store, for PLATFORM. Every container it starts
# is removed on exit. Needs docker, sha256sum, tar and (reduced, full) readelf.

set -euo pipefail
# readelf and tar print in the C locale, the one the checks below read.
export LC_ALL=C

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SAMPLE_CONFIG="${REPO_ROOT}/deployments/config.sample.yaml"
BINARY_PATH="usr/local/bin/awtrix-controller"
CONFIG_PATH="/etc/awtrix-controller/config.yaml"
# The sample configuration's broker credentials.
MQTT_USERNAME="awtrix"
MQTT_PASSWORD="changeme"

usage() {
	sed -n '/^# Usage:/,/^# IMAGE/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
	exit 2
}

die() {
	echo "image-test: FAIL: $*" >&2
	exit 1
}

pass() {
	echo "image-test: ok: $*"
}

tier="" image="" platform="" binary="" expect_version=""
while [[ $# -gt 0 ]]; do
	case "$1" in
	--tier) tier="${2:-}" ;;
	--image) image="${2:-}" ;;
	--platform) platform="${2:-}" ;;
	--binary) binary="${2:-}" ;;
	--expect-version) expect_version="${2:-}" ;;
	*) usage ;;
	esac
	shift 2 || usage
done
case "${tier}" in
smoke) level=1 ;;
reduced) level=2 ;;
full) level=3 ;;
*) usage ;;
esac
[[ -n ${image} && -n ${platform} && -n ${binary} ]] || usage
[[ -f ${binary} ]] || die "the release binary ${binary} does not exist"
[[ -f ${SAMPLE_CONFIG} ]] || die "the sample configuration ${SAMPLE_CONFIG} does not exist"

work="$(mktemp -d)"
containers=()
cleanup() {
	if [[ ${#containers[@]} -gt 0 ]]; then
		docker rm -f "${containers[@]}" >/dev/null 2>&1 || true
	fi
	rm -rf "${work}"
}
trap cleanup EXIT

# Sets NAME to a unique container name, which the exit trap removes. Not called through $( ): a
# subshell would add the name to its own copy of the list.
new_container() {
	NAME="awtrix-image-test-$1-$$-${RANDOM}"
	containers+=("${NAME}")
}

# --- The image is the platform's -------------------------------------------------------------

got_platform="$(docker image inspect -f '{{.Os}}/{{.Architecture}}' "${image}")"
[[ ${got_platform} == "${platform}" ]] || die "${image} is ${got_platform}, want ${platform}"
pass "image platform ${got_platform}"

# --- The binary in the image is the release binary --------------------------------------------

# The image's filesystem, exported from a container that never starts, so the binary's bytes,
# mode and owner are read without running anything.
new_container export
export_name="${NAME}"
docker create --platform "${platform}" --name "${export_name}" "${image}" >/dev/null
docker export -o "${work}/rootfs.tar" "${export_name}"
mkdir "${work}/rootfs"
tar -xf "${work}/rootfs.tar" -C "${work}/rootfs" "${BINARY_PATH}"
extracted="${work}/rootfs/${BINARY_PATH}"

want_sum="$(sha256sum "${binary}")"
want_sum="${want_sum%% *}"
got_sum="$(sha256sum "${extracted}")"
got_sum="${got_sum%% *}"
[[ ${got_sum} == "${want_sum}" ]] ||
	die "/${BINARY_PATH} in the image is sha256 ${got_sum}, the release binary ${binary} is ${want_sum}"
pass "/${BINARY_PATH} is the release binary (sha256 ${got_sum})"

# --- --version prints the stamps ---------------------------------------------------------------

version_out="$(docker run --rm --platform "${platform}" "${image}" --version)"
if [[ -n ${expect_version} ]]; then
	[[ ${version_out} == "${expect_version}" ]] ||
		die "--version prints '${version_out}', want '${expect_version}'"
else
	[[ ${version_out} =~ ^[^[:space:]]+\ [^[:space:]]+\ [^[:space:]]+$ ]] ||
		die "--version prints '${version_out}', not '<version> <commit> <date>'"
fi
pass "--version: ${version_out}"

# --- The broker accepts a connection ----------------------------------------------------------

# One MQTT 3.1.1 CONNECT (clean session, user name and password, keep-alive 60 s) to 127.0.0.1:$1
# as user $2 with password $3, over bash's /dev/tcp; prints the CONNACK return code, or nothing
# when the connection fails or no CONNACK arrives.
mqtt_connack() {
	local port="$1" user="$2" password="$3" client="image-test-$$"
	local -a fields=("${client}" "${user}" "${password}")
	local payload="" field remaining
	for field in "${fields[@]}"; do
		payload+="$(printf '\\x%02x\\x%02x' $((${#field} >> 8)) $((${#field} & 255)))${field}"
	done
	remaining=$((10 + 6 + ${#client} + ${#user} + ${#password}))
	((remaining < 128)) || return 0
	# A subshell, so the descriptor is closed with it.
	(
		exec 3<>"/dev/tcp/127.0.0.1/${port}" || exit 0
		# %b expands the \xHH escapes; the names and the password hold none of their own.
		printf '%b' "\\x10\\x$(printf '%02x' "${remaining}")\\x00\\x04MQTT\\x04\\xc2\\x00\\x3c${payload}" >&3
		# Fixed header (0x20, length 2), session-present flags, return code.
		timeout 10 head -c 4 <&3 >"${work}/connack" || exit 0
		od -An -tx1 -v "${work}/connack" >"${work}/connack.hex"
		read -r type length _ code <"${work}/connack.hex" || exit 0
		[[ ${type} == 20 && ${length} == 02 ]] || exit 0
		printf '%d' "0x${code}"
	)
}

# Starts the image as $1 with the sample configuration, the MQTT port published on a free
# loopback port, and any further docker run options; prints the host port.
start_broker() {
	local name="$1"
	shift
	docker run -d --platform "${platform}" --name "${name}" \
		-v "${SAMPLE_CONFIG}:${CONFIG_PATH}:ro" -p 127.0.0.1::1883 "$@" \
		"${image}" --config "${CONFIG_PATH}" --log-level info >/dev/null
	local mapping
	mapping="$(docker port "${name}" 1883/tcp)"
	mapping="${mapping%%$'\n'*}"
	printf '%s' "${mapping##*:}"
}

# Waits up to 60 s (QEMU is slow) for an authenticated CONNECT to $2 of container $1 to be
# answered, and prints the CONNACK return code; dies when the container exits or never answers.
await_connack() {
	local name="$1" port="$2" user="$3" password="$4" code="" running
	for _ in $(seq 1 60); do
		running="$(docker inspect -f '{{.State.Running}}' "${name}")"
		if [[ ${running} != true ]]; then
			docker logs "${name}" >&2 || true
			die "the container ${name} exited before its broker answered"
		fi
		code="$(mqtt_connack "${port}" "${user}" "${password}")"
		if [[ -n ${code} ]]; then
			printf '%s' "${code}"
			return 0
		fi
		sleep 1
	done
	docker logs "${name}" >&2 || true
	die "the broker of ${name} sent no CONNACK within 60 s"
}

hardening=()
if ((level >= 3)); then
	# docker-compose.yaml's hardening, plus every capability dropped: the controller needs none.
	hardening=(--read-only --tmpfs /tmp --security-opt no-new-privileges:true --cap-drop ALL)
fi
new_container broker
broker="${NAME}"
port="$(start_broker "${broker}" "${hardening[@]}")"
code="$(await_connack "${broker}" "${port}" "${MQTT_USERNAME}" "${MQTT_PASSWORD}")"
[[ ${code} == 0 ]] || die "the sample credentials got CONNACK return code ${code}, want 0"
pass "the broker accepts the sample credentials (CONNACK 0)"

if ((level >= 2)); then
	# --- The binary is the platform's, and static ------------------------------------------

	case "${platform}" in
	linux/amd64) want_machine="Advanced Micro Devices X86-64" ;;
	linux/arm64) want_machine="AArch64" ;;
	*) die "no ELF machine known for ${platform}" ;;
	esac
	header="$(readelf -h "${extracted}")"
	[[ ${header} == *"Machine:"*"${want_machine}"* ]] ||
		die "/${BINARY_PATH} is not a ${want_machine} binary"
	program_headers="$(readelf -lW "${extracted}")"
	[[ ${program_headers} != *INTERP* ]] || die "/${BINARY_PATH} has a program interpreter: it is not static"
	dynamic="$(readelf -dW "${extracted}")"
	[[ ${dynamic} != *"(NEEDED)"* ]] || die "/${BINARY_PATH} needs a shared library: it is not static"
	pass "/${BINARY_PATH} is a static ${want_machine} binary"

	# --- The binary's mode and owner ---------------------------------------------------------

	listing="$(tar --numeric-owner -tvf "${work}/rootfs.tar" "${BINARY_PATH}")"
	read -r mode owner _ <<<"${listing}"
	[[ ${mode} == -rwxr-xr-x && ${owner} == 0/0 ]] ||
		die "/${BINARY_PATH} is ${mode} ${owner}, want -rwxr-xr-x 0/0"
	pass "/${BINARY_PATH} is ${mode} ${owner}"

	# --- The image config ----------------------------------------------------------------------

	user="$(docker image inspect -f '{{.Config.User}}' "${image}")"
	[[ -n ${user} && ${user} != root && ${user} != 0 && ${user} != 0:* && ${user} != root:* ]] ||
		die "the image runs as '${user:-root}', want the base's non-root user"
	entrypoint="$(docker image inspect -f '{{json .Config.Entrypoint}}' "${image}")"
	[[ ${entrypoint} == '["/usr/local/bin/awtrix-controller"]' ]] ||
		die "the entrypoint is ${entrypoint}, want [\"/usr/local/bin/awtrix-controller\"]"
	ports="$(docker image inspect -f '{{json .Config.ExposedPorts}}' "${image}")"
	[[ ${ports} == '{"1883/tcp":{}}' ]] || die "the exposed ports are ${ports}, want 1883/tcp only"
	if [[ -n ${expect_version} ]]; then
		read -r want_label_version want_label_revision want_label_created <<<"${expect_version}"
		for pair in "version=${want_label_version}" "revision=${want_label_revision}" "created=${want_label_created}"; do
			key="${pair%%=*}" want="${pair#*=}"
			got="$(docker image inspect -f "{{ index .Config.Labels \"org.opencontainers.image.${key}\" }}" "${image}")"
			[[ ${got} == "${want}" ]] || die "the label org.opencontainers.image.${key} is '${got}', want '${want}'"
		done
	fi
	pass "image config: user ${user}, entrypoint ${entrypoint}, ports ${ports}, labels"
fi

if ((level >= 3)); then
	# --- Credentials are checked -----------------------------------------------------------------

	code="$(await_connack "${broker}" "${port}" "${MQTT_USERNAME}" "not-the-password")"
	[[ ${code} == 4 || ${code} == 5 ]] ||
		die "a wrong password got CONNACK return code ${code}, want 4 or 5 (refused)"
	pass "the broker refuses a wrong password (CONNACK ${code})"

	# --- A stop is clean -------------------------------------------------------------------------

	docker stop -t 10 "${broker}" >/dev/null
	exit_code="$(docker inspect -f '{{.State.ExitCode}}' "${broker}")"
	[[ ${exit_code} == 0 ]] || die "docker stop: exit code ${exit_code}, want 0"
	pass "docker stop: exit code 0"

	# --- A missing configuration fails the start -----------------------------------------------

	new_container missing
	missing="${NAME}"
	docker run --platform "${platform}" --name "${missing}" "${image}" \
		--config /etc/awtrix-controller/absent.yaml >/dev/null 2>&1 || true
	exit_code="$(docker inspect -f '{{.State.ExitCode}}' "${missing}")"
	[[ ${exit_code} != 0 ]] || die "a missing configuration file exited 0"
	pass "a missing configuration file fails the start (exit code ${exit_code})"
fi

echo "image-test: ${tier}: every check passed for ${image} (${platform})"
