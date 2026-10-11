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

package toolchain_test

import (
	"fmt"
	"maps"
	"regexp"
	"slices"
	"strings"
	"testing"

	"gopkg.in/yaml.v3"
)

const (
	setupGo        = "actions/setup-go"
	goVersionFile  = "go.mod"
	auditFragment  = ".mk/audit-deps.mk"
	govulncheckPin = "v1.8.0"
)

// pinSource is one place a version is written: a file and the pattern whose first group is the
// version there.
type pinSource struct {
	file    string
	pattern *regexp.Regexp
}

// TestGoLineIsOneValue: the go directive and the build stage's golang tag name the same Go line
// (major.minor). The patch may differ: Dependabot moves the image's patch within the line, and the
// go directive is the minimum toolchain, not the one that builds.
func TestGoLineIsOneValue(t *testing.T) {
	t.Parallel()

	expectOneValue(t, "Go line", []pinSource{
		{file: "go.mod", pattern: regexp.MustCompile(`(?m)^go (\d+\.\d+)(?:\.\d+)?$`)},
		{
			file:    "deployments/docker/Dockerfile",
			pattern: regexp.MustCompile(`dhi\.io/golang:(\d+\.\d+)[.-]`),
		},
	})
}

// TestActionStepsArePinned: every setup-go step under .github reads the Go line from go.mod, and
// no step writes a go-version of its own, which the Go line test above would not see.
func TestActionStepsArePinned(t *testing.T) {
	t.Parallel()

	steps := 0

	for _, file := range githubYAML(t) {
		parsed, err := parseActionSteps(file, []byte(readRepoFile(t, file)))
		if err != nil {
			t.Fatal(err)
		}

		for _, problem := range goPinProblems(parsed) {
			t.Error(problem)
		}

		for _, step := range parsed {
			if step.action == setupGo {
				steps++
			}
		}
	}

	if steps == 0 {
		t.Error("no setup-go step found under .github; the rule would check nothing")
	}
}

// TestActionPinRules is the negative control of the step rules: each unpinned form is a problem.
func TestActionPinRules(t *testing.T) {
	t.Parallel()

	steps, err := parseActionSteps("sample.yaml", []byte(`jobs:
  test:
    steps:
      - uses: actions/setup-go@0123456789abcdef0123456789abcdef01234567 # v7.0.0
        with:
          go-version-file: go.mod
      - uses: actions/setup-go@0123456789abcdef0123456789abcdef01234567
        with:
          go-version: "1.26"
      - uses: actions/setup-go@0123456789abcdef0123456789abcdef01234567
      - uses: actions/setup-go@0123456789abcdef0123456789abcdef01234567
        with:
          go-version-file: .go-version
      - uses: some/other-action@0123456789abcdef0123456789abcdef01234567
        with:
          go-version: "1.27"
`))
	if err != nil {
		t.Fatal(err)
	}

	// The second step breaks both rules, the next two one each, and the last one the go-version
	// rule.
	if problems := goPinProblems(steps); len(problems) != 5 {
		t.Errorf("want 5 problems, got %d: %q", len(problems), problems)
	}
}

// TestHadolintIsOneVersion: the hadolint hook runs the system binary, so the version CI installs
// is the one that lints; it must follow the hook's rev, which Dependabot moves.
func TestHadolintIsOneVersion(t *testing.T) {
	t.Parallel()

	expectOneValue(t, "hadolint", []pinSource{
		{
			file: ".pre-commit-config.yaml",
			pattern: regexp.MustCompile(
				`repo: https://github\.com/hadolint/hadolint\s+rev:\s*(v[\d.]+)`,
			),
		},
		{
			file:    ".github/actions/setup-hadolint/action.yaml",
			pattern: regexp.MustCompile(`HADOLINT_VERSION:\s*["']?(v[\d.]+)`),
		},
	})
}

// TestAuditGovulncheckIsPinned: the scanner is fetched at a fixed version and runs over the test
// packages too. An unpinned @latest would make two runs of the same commit mean different things.
func TestAuditGovulncheckIsPinned(t *testing.T) {
	t.Parallel()

	fragment := readRepoFile(t, auditFragment)

	for _, want := range []string{
		"GOVULNCHECK_VERSION ?= " + govulncheckPin + "\n",
		"golang.org/x/vuln/cmd/govulncheck@$(GOVULNCHECK_VERSION)\n",
		"$(GO) run $(GOVULNCHECK) -test ./...\n",
	} {
		if !strings.Contains(fragment, want) {
			t.Errorf("%s does not contain %q", auditFragment, want)
		}
	}

	if strings.Contains(fragment, "@latest") {
		t.Errorf("%s fetches something @latest", auditFragment)
	}
}

// actionStep is one step, in a workflow or a composite action, that uses an action.
type actionStep struct {
	file string
	// action is the uses: value without its @ref.
	action string
	with   map[string]string
}

// parseActionSteps finds every mapping with a uses: key anywhere in the document, so job steps and
// composite action steps alike.
func parseActionSteps(file string, content []byte) ([]actionStep, error) {
	var document any

	err := yaml.Unmarshal(content, &document)
	if err != nil {
		return nil, fmt.Errorf("parse %s: %w", file, err)
	}

	var steps []actionStep

	var walk func(node any)

	walk = func(node any) {
		switch value := node.(type) {
		case map[string]any:
			if uses, ok := value["uses"].(string); ok {
				action, _, _ := strings.Cut(uses, "@")
				step := actionStep{file: file, action: action, with: map[string]string{}}

				inputs, _ := value["with"].(map[string]any)
				for key, input := range inputs {
					step.with[key] = fmt.Sprint(input)
				}

				steps = append(steps, step)
			}

			for _, key := range slices.Sorted(maps.Keys(value)) {
				walk(value[key])
			}
		case []any:
			for _, child := range value {
				walk(child)
			}
		}
	}

	walk(document)

	return steps, nil
}

// goPinProblems applies the Go rules to steps: no step writes a go-version, and setup-go reads
// go.mod.
func goPinProblems(steps []actionStep) []string {
	var problems []string

	for _, step := range steps {
		if step.with["go-version"] != "" {
			problems = append(problems, fmt.Sprintf(
				"%s: %s gets go-version %q; the Go line comes from %s",
				step.file, step.action, step.with["go-version"], goVersionFile))
		}

		if step.action == setupGo && step.with["go-version-file"] != goVersionFile {
			problems = append(problems, fmt.Sprintf(
				"%s: setup-go must read go-version-file: %s", step.file, goVersionFile))
		}
	}

	return problems
}

// expectOneValue fails unless every source has the pin and all found values agree.
func expectOneValue(t *testing.T, what string, sources []pinSource) {
	t.Helper()

	found := map[string][]string{}

	for _, source := range sources {
		matches := source.pattern.FindAllStringSubmatch(readRepoFile(t, source.file), -1)
		if len(matches) == 0 {
			t.Errorf("%s: %s does not write it (%s)", what, source.file, source.pattern)
		}

		for _, match := range matches {
			found[match[1]] = append(found[match[1]], source.file)
		}
	}

	if len(found) > 1 {
		values := make([]string, 0, len(found))
		for _, value := range slices.Sorted(maps.Keys(found)) {
			values = append(values, value+" in "+strings.Join(slices.Compact(found[value]), ", "))
		}

		t.Errorf("%s is not one value: %s", what, strings.Join(values, "; "))
	}
}
