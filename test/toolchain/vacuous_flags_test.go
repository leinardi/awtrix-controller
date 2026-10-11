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
	"regexp"
	"strings"
	"testing"
)

// vacuousFlags turn a gate with nothing to check, or a failing one, into a pass. None of them may
// appear where gates are defined.
var vacuousFlags = []struct {
	name    string
	pattern *regexp.Regexp
}{
	{"--passWithNoTests", regexp.MustCompile(`--pass-?[Ww]ith-?[Nn]o-?[Tt]ests`)},
	{"--no-error-on-unmatched-pattern", regexp.MustCompile(`--no-error-on-unmatched-pattern`)},
	{"|| true", regexp.MustCompile(`\|\|\s*(true\b|:(\s|;|$)|exit\s+0\b)`)},
	{"make's - recipe prefix", regexp.MustCompile(`^\t[@+]*-`)},
	{".IGNORE", regexp.MustCompile(`^\.IGNORE\b`)},
	{
		"make --ignore-errors",
		regexp.MustCompile(`\$\(MAKE\).*\s(-[a-zA-Z]*i[a-zA-Z]*|--ignore-errors)\b`),
	},
	{"continue-on-error", regexp.MustCompile(`continue-on-error`)},
	{
		"if: false",
		regexp.MustCompile(`\bif:\s*(["']?false["']?\s*$|["']?\$\{\{\s*false\s*\}\}["']?)`),
	},
}

// allowedHit is a vacuous-looking line that is not one: the file it may appear in, and the
// pattern the whole trimmed line must match.
type allowedHit struct {
	file    string
	pattern *regexp.Regexp
	reason  string
}

// allowedHits are the reviewed exceptions. Each must still match at least one line, so an entry
// cannot outlive the code it excuses and quietly cover a new line later.
var allowedHits = []allowedHit{
	{
		file: ".github/workflows/release.yaml",
		// grep exits 1 when it selects nothing, and "no release tag yet" is an answer here, not
		// a failure; the brace group confines the || true to that one grep under pipefail.
		pattern: regexp.MustCompile(`\{ grep -E "\$RELEASE_TAG_REGEX" \|\| true; \}`),
		reason:  "a tag filter that may select nothing",
	},
	{
		file: ".github/workflows/release.yaml",
		// The first release has no HEAD^ history with tags; the empty answer means "no previous
		// release", which the step handles.
		pattern: regexp.MustCompile(`\{ git tag --merged "HEAD\^" 2>/dev/null \|\| true; \}`),
		reason:  "the previous release lookup on a first release",
	},
}

// TestNoVacuousFlags scans the files that define gates: the Makefile and every make fragment, the
// pre-commit configuration and every YAML file under .github, the workflows included.
func TestNoVacuousFlags(t *testing.T) {
	t.Parallel()

	fragments := glob(t, ".mk", ".mk")
	if len(fragments) == 0 {
		t.Fatal("no make fragments found under .mk/; the scan would check nothing")
	}

	files := append([]string{"Makefile", ".pre-commit-config.yaml"}, fragments...)
	files = append(files, githubYAML(t)...)

	used := make([]bool, len(allowedHits))

	for _, file := range files {
		for _, hit := range vacuousHits(readRepoFile(t, file)) {
			index := allowedIndex(file, hit.line)
			if index >= 0 {
				used[index] = true

				continue
			}

			t.Errorf("%s:%d: %s: %s", file, hit.number, hit.flag, hit.line)
		}
	}

	for index, entry := range allowedHits {
		if !used[index] {
			t.Errorf(
				"the allowed hit %q in %s matches no line; remove it",
				entry.reason,
				entry.file,
			)
		}
	}
}

// TestVacuousFlagsAreRecognized is the negative control: each flag, written as a gate would carry
// it, is found, and an allowed pattern excuses only its own file.
func TestVacuousFlagsAreRecognized(t *testing.T) {
	t.Parallel()

	for _, sample := range []string{
		`"test": "vitest run --passWithNoTests"`,
		`"lint": "eslint src --no-error-on-unmatched-pattern"`,
		"\tgo vet ./... || true",
		"\tgo vet ./... ||:",
		"\tgo vet ./... || exit 0",
		"\t-go vet ./...",
		"\t@-go vet ./...",
		".IGNORE: go-lint",
		"\t$(MAKE) -i go-test",
		"\t$(MAKE) --ignore-errors go-test",
		"        continue-on-error: true",
		"    if: false",
		"    if: ${{ false }}",
		"    if: 'false'",
		"    if: \"${{ false }}\"",
	} {
		if len(vacuousHits(sample)) == 0 {
			t.Errorf("no vacuous flag found in %q", sample)
		}
	}

	allowed := `{ grep -E "$RELEASE_TAG_REGEX" || true; } | sort -V | tail -n1`
	if allowedIndex(".github/workflows/release.yaml", allowed) < 0 {
		t.Error("the release tag filter is not allowed in release.yaml")
	}

	if allowedIndex(".github/workflows/ci.yaml", allowed) >= 0 {
		t.Error("the release tag filter is allowed outside release.yaml")
	}

	if allowedIndex(".github/workflows/release.yaml", "make go-test || true") >= 0 {
		t.Error("an unrelated || true is allowed in release.yaml")
	}
}

type vacuousHit struct {
	number int
	flag   string
	line   string
}

func vacuousHits(content string) []vacuousHit {
	var hits []vacuousHit

	for index, line := range strings.Split(content, "\n") {
		for _, flag := range vacuousFlags {
			if flag.pattern.MatchString(line) {
				hits = append(hits, vacuousHit{index + 1, flag.name, strings.TrimSpace(line)})
			}
		}
	}

	return hits
}

// allowedIndex returns the index of the allowed hit that excuses line in file, or -1.
func allowedIndex(file, line string) int {
	for index, entry := range allowedHits {
		if entry.file == file && entry.pattern.MatchString(line) {
			// The excused construct must be the line's only flag: an || true elsewhere on the
			// same line stays a finding.
			rest := entry.pattern.ReplaceAllString(line, "")
			if len(vacuousHits(rest)) == 0 {
				return index
			}
		}
	}

	return -1
}
