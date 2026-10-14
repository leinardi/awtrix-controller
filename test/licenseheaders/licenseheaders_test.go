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

// Package licenseheaders_test drives scripts/check-license-headers.sh: on this repository, and on
// scratch repositories that hold each kind of file it must reject or let through.
package licenseheaders_test

import (
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"testing"

	"github.com/leinardi/awtrix-controller/test/internal/gitenv"
	"github.com/leinardi/awtrix-controller/test/internal/toolgate"
)

const scriptPath = "scripts/check-license-headers.sh"

// notice is the MIT notice without its comment prefix, as the repository's files carry it.
var notice = []string{
	"MIT License",
	"",
	"Copyright (c) 2026 Roberto Leinardi",
	"",
	"Permission is hereby granted, free of charge, to any person obtaining a copy",
	"of this software and associated documentation files (the \"Software\"), to deal",
	"in the Software without restriction, including without limitation the rights",
	"to use, copy, modify, merge, publish, distribute, sublicense, and/or sell",
	"copies of the Software, and to permit persons to whom the Software is",
	"furnished to do so, subject to the following conditions:",
	"",
	"The above copyright notice and this permission notice shall be included in all",
	"copies or substantial portions of the Software.",
	"",
	"THE SOFTWARE IS PROVIDED \"AS IS\", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR",
	"IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,",
	"FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE",
	"AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER",
	"LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,",
	"OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE",
	"SOFTWARE.",
}

// lastRequiredLine is the line of goHeader that holds the last notice line the checker requires
// (the warranty disclaimer's first line), counted from 1.
const lastRequiredLine = 16

// headerWindow is the number of leading lines the checker reads.
const headerWindow = 30

// untrackedPrefix marks scratch files that are written but never added: the check reads tracked
// files only.
const untrackedPrefix = "untracked/"

var (
	goHeader    = comment("/*", " * ", " *", " */") + "\npackage x\n"
	hashHeader  = comment("#", "# ", "#", "#")
	shellHeader = "#!/usr/bin/env bash\n" + hashHeader + "\necho ok\n"
	makefile    = hashHeader + "\nMK_COMMON_FILES := docker.mk help.mk\n"
)

// comment renders the notice as a comment block: an opening line, each notice line with prefix (or
// blank for an empty one), and a closing line.
func comment(opening, prefix, blank, closing string) string {
	lines := []string{opening}

	for _, line := range notice {
		if line == "" {
			lines = append(lines, blank)

			continue
		}

		lines = append(lines, prefix+line)
	}

	return strings.Join(append(lines, closing), "\n") + "\n"
}

// TestRepositoryHeaders runs the check on this repository.
func TestRepositoryHeaders(t *testing.T) {
	t.Parallel()

	root := toolgate.RepoRoot(t)
	readScript(t, root)
	readTrackedFiles(t, root)

	// The repository's own git environment is the right one here: under the commit hook it names
	// the index being committed.
	cmd := exec.CommandContext(t.Context(), scriptPath)
	cmd.Dir = root

	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Errorf("the repository fails its own license header check: %v\n%s", err, out)
	}
}

// TestHeaderCases runs the check on scratch repositories, one file kind at a time.
func TestHeaderCases(t *testing.T) {
	t.Parallel()

	script := readScript(t, toolgate.RepoRoot(t))
	fillerInWindow := strings.Repeat("// filler\n", headerWindow-lastRequiredLine)
	syntaxLine := "# syntax=docker/dockerfile:1.7\n\n"

	for name, test := range map[string]struct {
		files map[string]string
		pass  bool
	}{
		"go file with the header":  {map[string]string{"internal/x/x.go": goHeader}, true},
		"go file without a header": {map[string]string{"internal/x/x.go": "package x\n"}, false},
		"a year range": {
			map[string]string{"internal/x/x.go": strings.Replace(goHeader, "2026", "2025-2026", 1)},
			true,
		},
		"another copyright holder": {
			map[string]string{
				"internal/x/x.go": strings.Replace(goHeader, "Roberto Leinardi", "Someone Else", 1),
			},
			false,
		},
		"the notice cut short": {
			map[string]string{"internal/x/x.go": strings.Replace(goHeader, " * THE SOFTWARE IS", " *", 1)},
			false,
		},
		"shell script with the header":   {map[string]string{"scripts/a.sh": shellHeader}, true},
		"shell script without a header":  {map[string]string{"scripts/a.sh": "#!/usr/bin/env bash\necho\n"}, false},
		"dockerfile after its directive": {map[string]string{"deployments/docker/Dockerfile": syntaxLine + hashHeader + "FROM scratch\n"}, true},
		"dockerfile without a header":    {map[string]string{"deployments/docker/Dockerfile": "FROM scratch\n"}, false},
		"local make fragment, no header": {map[string]string{".mk/verify.mk": "x:\n"}, false},
		"makefile without a header":      {map[string]string{"Makefile": "MK_COMMON_FILES := docker.mk\n"}, false},
		"header ending on the last line of the window": {
			map[string]string{"internal/x/x.go": fillerInWindow + goHeader},
			true,
		},
		"header ending one line past the window": {
			map[string]string{"internal/x/x.go": "// filler\n" + fillerInWindow + goHeader},
			false,
		},
		"untracked file without a header": {map[string]string{untrackedPrefix + "scratch.go": "package x\n"}, true},
		"upstream make fragment":          {map[string]string{".mk/docker.mk": "x:\n", ".mk/help.mk": "y:\n"}, true},
		"bootstrap script":                {map[string]string{"scripts/bootstrap-mk-common.sh": "#!/bin/sh\n"}, true},
		"markdown, yaml and json":         {map[string]string{"README.md": "# x\n", "a.yaml": "a: 1\n", "b.json": "{}\n"}, true},
	} {
		t.Run(name, func(t *testing.T) {
			t.Parallel()

			dir := scratchRepo(t, script, test.files)

			out, err := runIn(t, dir)
			if test.pass && err != nil {
				t.Errorf("rejected, want accepted: %v\n%s", err, out)
			}

			if !test.pass && err == nil {
				t.Errorf("accepted, want rejected:\n%s", out)
			}
		})
	}
}

// readTrackedFiles reads the index and every tracked file, so a change to any of them is part of
// the test cache key and a cached pass is not replayed after a headerless file is added.
func readTrackedFiles(t *testing.T, root string) {
	t.Helper()

	for _, args := range [][]string{{"rev-parse", "--git-path", "index"}, {"ls-files"}} {
		cmd := exec.CommandContext(t.Context(), "git", args...)
		cmd.Dir = root

		out, err := cmd.Output()
		if err != nil {
			t.Fatalf("git %v: %v", args, err)
		}

		for name := range strings.SplitSeq(strings.TrimSpace(string(out)), "\n") {
			if !filepath.IsAbs(name) {
				name = filepath.Join(root, name)
			}

			// A deleted tracked file is the script's concern, not this one's.
			_, _ = os.ReadFile(name)
		}
	}
}

// readScript reads the checker, which also makes it part of the test cache key: a program run only
// through exec is not, and an edited script would otherwise replay a cached pass.
func readScript(t *testing.T, root string) []byte {
	t.Helper()

	data, err := os.ReadFile(filepath.Join(root, scriptPath))
	if err != nil {
		t.Fatalf("read the checker: %v", err)
	}

	return data
}

// scratchRepo creates a git repository holding the checker, a Makefile and files.
func scratchRepo(t *testing.T, script []byte, files map[string]string) string {
	t.Helper()

	dir := t.TempDir()

	root, err := os.OpenRoot(dir)
	if err != nil {
		t.Fatalf("open %s: %v", dir, err)
	}

	defer func() { _ = root.Close() }()

	files[scriptPath] = string(script)
	if _, ok := files["Makefile"]; !ok {
		files["Makefile"] = makefile
	}

	for name, content := range files {
		err = root.MkdirAll(filepath.Dir(name), 0o750)
		if err != nil {
			t.Fatalf("create the directory of %s: %v", name, err)
		}

		err = root.WriteFile(name, []byte(content), 0o600)
		if err != nil {
			t.Fatalf("write %s: %v", name, err)
		}
	}

	err = root.Chmod(scriptPath, 0o700)
	if err != nil {
		t.Fatalf("make the checker executable: %v", err)
	}

	git(t, dir, "init", "--quiet")

	for name := range files {
		if !strings.HasPrefix(name, untrackedPrefix) {
			git(t, dir, "add", "--", name)
		}
	}

	return dir
}

// runIn runs the checker kept at scriptPath inside the repository at dir.
func runIn(t *testing.T, dir string) (string, error) {
	t.Helper()

	// A relative command path is resolved against Dir.
	cmd := exec.CommandContext(t.Context(), scriptPath)
	cmd.Dir = dir
	cmd.Env = gitenv.Isolated()

	out, err := cmd.CombinedOutput()

	return string(out), err
}

func git(t *testing.T, dir string, args ...string) {
	t.Helper()

	cmd := exec.CommandContext(t.Context(), "git", args...)
	cmd.Dir = dir
	cmd.Env = gitenv.Isolated()

	out, err := cmd.CombinedOutput()
	if err != nil {
		t.Fatalf("git %v: %v\n%s", args, err, out)
	}
}
