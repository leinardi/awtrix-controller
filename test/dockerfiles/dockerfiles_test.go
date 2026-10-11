/*
 * MIT License
 *
 * Copyright (c) 2026 Roberto Leinardi
 *
 * Permission is hereby granted, free of charge, to any person obtaining a copy
 * of this software and associated documentation files (the "Software"), to deal
 * in the Software without restriction, including without limitation the rights
 * to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
 * copies of the Software, and to permit persons to whom the Software is
 * furnished to do so, subject to the following conditions:
 *
 * The above copyright notice and this permission notice shall be included in all
 * copies or substantial portions of the Software.
 *
 * THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
 * IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
 * FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
 * AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
 * LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
 * OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
 * SOFTWARE.
 */

package dockerfiles_test

import (
	"context"
	"os"
	"os/exec"
	"path/filepath"
	"regexp"
	"slices"
	"strings"
	"testing"
	"time"

	"github.com/leinardi/awtrix-controller/test/dockerfiles"
	"github.com/leinardi/awtrix-controller/test/internal/toolgate"
)

const (
	shippedDir        = "deployments/docker"
	shippedDockerfile = "deployments/docker/Dockerfile"
	releaseWorkflow   = ".github/workflows/release.yaml"
	dockerFragment    = ".mk/docker.mk"
	hadolintConfig    = ".hadolint.yaml"
	wantPlatforms     = "linux/amd64,linux/arm64"
	hadolintTimeout   = time.Minute
)

// wantLabels are the seven OCI labels of the runtime stage; an empty value means any non-empty one.
var wantLabels = map[string]string{
	"org.opencontainers.image.title":       "awtrix-controller",
	"org.opencontainers.image.description": "",
	"org.opencontainers.image.source":      "https://github.com/leinardi/awtrix-controller",
	"org.opencontainers.image.version":     "${VERSION}",
	"org.opencontainers.image.revision":    "${COMMIT}",
	"org.opencontainers.image.created":     "${DATE}",
	"org.opencontainers.image.licenses":    "MIT",
}

// TestShippedDockerfileIsTheOnlyOne pins that deployments/docker holds exactly the image that
// ships. A second file there would look shipped to every reader and be built by nothing.
func TestShippedDockerfileIsTheOnlyOne(t *testing.T) {
	t.Parallel()

	entries, err := os.ReadDir(repoPath(t, shippedDir))
	if err != nil {
		t.Fatalf("read %s: %v", shippedDir, err)
	}

	var found []string

	for _, entry := range entries {
		if entry.Name() == "Dockerfile" || strings.HasSuffix(entry.Name(), ".Dockerfile") {
			found = append(found, entry.Name())
		}
	}

	if !slices.Equal(found, []string{"Dockerfile"}) {
		t.Errorf("%s holds %v, want exactly [Dockerfile]", shippedDir, found)
	}
}

// TestSyntaxDirectiveIsPinned pins the BuildKit frontend that reads the Dockerfile. A bare
// docker/dockerfile:1.7 is a tag that can be moved under a build, and Dependabot never updates the
// line, so it is a literal reference pinned by digest (docs/release.md).
func TestSyntaxDirectiveIsPinned(t *testing.T) {
	t.Parallel()

	syntax, ok := dockerfiles.Syntax(readRepoFile(t, shippedDockerfile))
	if !ok {
		t.Fatalf("%s has no syntax parser directive on its first line", shippedDockerfile)
	}

	if dockerfiles.Repository(syntax) != "docker/dockerfile" {
		t.Errorf("%s reads with the frontend %q, want docker/dockerfile", shippedDockerfile, syntax)
	}

	if !dockerfiles.DigestPinned(syntax) {
		t.Errorf("the frontend %q is not pinned by digest", syntax)
	}
}

// TestShippedStages pins the two named stages, in order, each from a literal base pinned by tag.
// The bases are deliberately not pinned by digest: dhi.io republishes its tags to ship security
// fixes, and every release build is scanned (AGENTS.md, docs/release.md). A variable base, a
// missing tag or latest would leave the base unreviewable.
func TestShippedStages(t *testing.T) {
	t.Parallel()

	stages := shippedStages(t)

	names := make([]string, 0, len(stages))
	for _, stage := range stages {
		names = append(names, stage.Name)

		if !dockerfiles.TagPinned(stage.Image) {
			t.Errorf("stage %q is FROM %q, want a literal reference with a tag other than latest",
				stage.Name, stage.Image)
		}
	}

	if !slices.Equal(names, []string{"build", "runtime"}) {
		t.Errorf("the stages are %q, want exactly build and runtime, in that order", names)
	}
}

// TestBuildStage pins how the binary is built: on the build platform, cross-compiled without cgo,
// reproducibly, and stamped with the version strings main reads.
func TestBuildStage(t *testing.T) {
	t.Parallel()

	build := stage(t, "build")

	if build.Platform != "$BUILDPLATFORM" {
		t.Errorf("the build stage runs on --platform=%q, want $BUILDPLATFORM", build.Platform)
	}

	if got := dockerfiles.Repository(build.Image); got != "dhi.io/golang" {
		t.Errorf("the build stage is FROM %s, want dhi.io/golang", got)
	}

	instructions := joined(build)
	for _, want := range []string{
		"ARG VERSION", "ARG COMMIT", "ARG DATE", "ARG TARGETOS", "ARG TARGETARCH",
		"CGO_ENABLED=0", "go mod download", "-trimpath", "-buildvcs=false",
		"-X 'main.version=${VERSION}'", "-X 'main.commit=${COMMIT}'", "-X 'main.date=${DATE}'",
		"-o /out/awtrix-controller ./cmd/awtrix-controller",
	} {
		if !strings.Contains(instructions, want) {
			t.Errorf("the build stage lacks %q:\n%s", want, instructions)
		}
	}
}

// TestRuntimeStage pins the posture of the image that ships: the shell-less static base, nothing
// run or written in it, the binary and the MQTT port, and exactly the seven OCI labels. There is
// no USER instruction: the image relies on dhi.io/static's non-root default, and an explicit root
// user would be a regression.
func TestRuntimeStage(t *testing.T) {
	t.Parallel()

	runtime := stage(t, "runtime")

	if got := dockerfiles.Repository(runtime.Image); got != "dhi.io/static" {
		t.Errorf("the runtime stage is FROM %s, want dhi.io/static", got)
	}

	if runtime.Platform != "" {
		t.Errorf("the runtime stage pins --platform=%s; it has to follow the target platform",
			runtime.Platform)
	}

	want := map[string]string{
		"EXPOSE":     "1883",
		"ENTRYPOINT": `["/usr/local/bin/awtrix-controller"]`,
		"COPY":       "--from=build /out/awtrix-controller /usr/local/bin/awtrix-controller",
	}

	seen := map[string][]string{}

	for _, instruction := range runtime.Instructions {
		switch instruction.Keyword {
		case "RUN", "SHELL", "VOLUME", "ADD", "ENV":
			t.Errorf("the runtime stage carries %s %s; it has no shell and writes nothing",
				instruction.Keyword, instruction.Args)
		case "USER":
			if user, _, _ := strings.Cut(instruction.Args, ":"); user == "root" || user == "0" {
				t.Errorf("the runtime stage runs as %s", instruction.Args)
			}
		}

		seen[instruction.Keyword] = append(seen[instruction.Keyword], instruction.Args)
	}

	for keyword, args := range want {
		if !slices.Equal(seen[keyword], []string{args}) {
			t.Errorf("the runtime stage has %s %q, want exactly %s %s",
				keyword, seen[keyword], keyword, args)
		}
	}

	labels := dockerfiles.Labels(runtime)

	for key, value := range wantLabels {
		if labels[key] == "" || (value != "" && labels[key] != value) {
			t.Errorf("label %s is %q, want %q", key, labels[key], value)
		}
	}

	if len(labels) != len(wantLabels) {
		t.Errorf("the runtime stage carries labels %v, want exactly the seven OCI labels", labels)
	}
}

// TestLabelArgumentsAreSupplied pins that every build that publishes or runs an image passes the
// three build arguments the version, revision and created labels read: without them the labels
// say dev, none and unknown.
func TestLabelArgumentsAreSupplied(t *testing.T) {
	t.Parallel()

	for _, file := range []string{releaseWorkflow, dockerFragment} {
		content := readRepoFile(t, file)

		for _, arg := range []string{"VERSION", "COMMIT", "DATE"} {
			pattern := regexp.MustCompile(`(?m)(^\s*|--build-arg )` + arg + `=\S`)
			if !pattern.MatchString(content) {
				t.Errorf("%s does not pass the %s build argument", file, arg)
			}
		}
	}
}

// TestPlatforms pins the two platforms the release builds: dhi.io/static has no arm/v7.
func TestPlatforms(t *testing.T) {
	t.Parallel()

	content := readRepoFile(t, releaseWorkflow)

	match := regexp.MustCompile(`(?m)^\s*PLATFORMS:\s*(\S+)\s*$`).FindStringSubmatch(content)
	if len(match) < 2 || match[1] != wantPlatforms {
		t.Errorf("%s declares PLATFORMS %q, want %q", releaseWorkflow, match, wantPlatforms)
	}
}

// TestHadolintEnforcesTheLabels drives hadolint with the repository's configuration: the shipped
// Dockerfile passes, and the same file without one of the seven labels fails with DL3049. Without
// the error override a missing label is only info, which the failure threshold lets through.
func TestHadolintEnforcesTheLabels(t *testing.T) {
	t.Parallel()

	hadolint := toolgate.Lookup(t, toolgate.Hadolint)
	config := repoPath(t, hadolintConfig)
	// hadolint reads the configuration only through exec, which the test cache cannot see;
	// reading it here makes an edit to it invalidate a cached pass.
	readRepoFile(t, hadolintConfig)

	output, err := runHadolint(t, hadolint, config, repoPath(t, shippedDockerfile))
	if err != nil {
		t.Errorf("hadolint rejects %s: %v\n%s", shippedDockerfile, err, output)
	}

	shipped := readRepoFile(t, shippedDockerfile)

	withoutVersion := regexp.MustCompile(`(?m)^\s*org\.opencontainers\.image\.version=.*\n`).
		ReplaceAllString(shipped, "")
	if withoutVersion == shipped {
		t.Fatal("could not remove the version label from the fixture")
	}

	scratch := filepath.Join(t.TempDir(), "Dockerfile")

	err = os.WriteFile(scratch, []byte(withoutVersion), 0o600)
	if err != nil {
		t.Fatal(err)
	}

	output, err = runHadolint(t, hadolint, config, scratch)
	if err == nil || !strings.Contains(output, "DL3049") {
		t.Errorf("hadolint accepted a Dockerfile without the version label: %v\n%s", err, output)
	}
}

// TestParseRejectsWhatTheAssertionsMustNotMiss is the negative control for the reader: each of
// these inputs has to come out as something the assertions above refuse.
func TestParseRejectsWhatTheAssertionsMustNotMiss(t *testing.T) {
	t.Parallel()

	const digest = "@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"

	for name, image := range map[string]string{
		"no tag":                  "dhi.io/static",
		"latest":                  "dhi.io/static:latest",
		"a variable tag":          "dhi.io/golang:${GO_LINE}-alpine3.23-dev",
		"a variable image":        "$BASE",
		"a stage name, not a ref": "build",
	} {
		if dockerfiles.TagPinned(image) {
			t.Errorf("%s (%q) passes as tag-pinned", name, image)
		}
	}

	if !dockerfiles.TagPinned("dhi.io/static:20250419") {
		t.Error("a literal tag does not pass as tag-pinned")
	}

	for name, image := range map[string]string{
		"a tag only":           "docker/dockerfile:1.7",
		"a short digest":       "docker/dockerfile:1.7@sha256:0123",
		"a digest with no tag": "docker/dockerfile" + digest,
	} {
		if dockerfiles.DigestPinned(image) {
			t.Errorf("%s (%q) passes as digest-pinned", name, image)
		}
	}

	if !dockerfiles.DigestPinned("docker/dockerfile:1.7" + digest) {
		t.Error("a literal tag plus digest does not pass as digest-pinned")
	}

	stages, err := dockerfiles.Parse(
		"FROM a:1 AS one\nRUN echo \\\n  two\n# note\nFROM b:2\nUSER 1:1\n",
	)
	if err != nil {
		t.Fatalf("parse: %v", err)
	}

	if len(stages) != 2 || stages[0].Name != "one" || stages[1].Name != "" ||
		joined(stages[0]) != "RUN echo two" || joined(stages[1]) != "USER 1:1" {
		t.Errorf("parsed %+v", stages)
	}

	for name, content := range map[string]string{
		"no directive":         "FROM a:1\n",
		"after a comment":      "# note\n# syntax=docker/dockerfile:1.7" + digest + "\n",
		"after a blank line":   "\n# syntax=docker/dockerfile:1.7" + digest + "\n",
		"after an instruction": "FROM a:1\n# syntax=docker/dockerfile:1.7" + digest + "\n",
	} {
		if syntax, ok := dockerfiles.Syntax(content); ok {
			t.Errorf("%s: read the syntax directive %q, which BuildKit ignores", name, syntax)
		}
	}

	_, err = dockerfiles.Parse("FROM a:1\nRUN echo \\\n")
	if err == nil {
		t.Error("a file ending inside a continuation parsed")
	}
}

func runHadolint(t *testing.T, hadolint, config, dockerfile string) (string, error) {
	t.Helper()

	ctx, cancel := context.WithTimeout(t.Context(), hadolintTimeout)
	defer cancel()

	cmd := exec.CommandContext(ctx, hadolint, "--no-color", "--config", config, dockerfile)
	output, err := cmd.CombinedOutput()

	return string(output), err
}

func shippedStages(t *testing.T) []dockerfiles.Stage {
	t.Helper()

	stages, err := dockerfiles.Parse(readRepoFile(t, shippedDockerfile))
	if err != nil {
		t.Fatalf("%s: %v", shippedDockerfile, err)
	}

	return stages
}

func stage(t *testing.T, name string) dockerfiles.Stage {
	t.Helper()

	var found []dockerfiles.Stage

	for _, candidate := range shippedStages(t) {
		if candidate.Name == name {
			found = append(found, candidate)
		}
	}

	if len(found) != 1 {
		t.Fatalf("%s has %d stages named %s, want one", shippedDockerfile, len(found), name)
	}

	return found[0]
}

func joined(stage dockerfiles.Stage) string {
	lines := make([]string, 0, len(stage.Instructions))
	for _, instruction := range stage.Instructions {
		lines = append(lines, instruction.Keyword+" "+instruction.Args)
	}

	return strings.Join(lines, "\n")
}

func repoPath(t *testing.T, rel string) string {
	t.Helper()

	return filepath.Join(toolgate.RepoRoot(t), filepath.FromSlash(rel))
}

// readRepoFile reads a file, which also records it in the test cache key.
func readRepoFile(t *testing.T, rel string) string {
	t.Helper()

	data, err := os.ReadFile(repoPath(t, rel))
	if err != nil {
		t.Fatalf("read %s: %v", rel, err)
	}

	return string(data)
}
