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

// Package toolchain_test pins the repository's tooling: one Go line across go.mod, the image and
// every setup-go step, one hadolint version between the hook and CI, the pinned govulncheck, and
// the absence of flags that make a gate pass on nothing.
package toolchain_test

import (
	"fmt"
	"io/fs"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/leinardi/awtrix-controller/test/internal/toolgate"
)

// readRepoFile reads a file by its path relative to the repository root. Reading it also makes it
// part of the test cache key.
func readRepoFile(t *testing.T, rel string) string {
	t.Helper()

	data, err := os.ReadFile(filepath.Join(toolgate.RepoRoot(t), filepath.FromSlash(rel)))
	if err != nil {
		t.Fatalf("read %s: %v", rel, err)
	}

	return string(data)
}

// githubYAML lists every YAML file under .github, relative to the repository root.
func githubYAML(t *testing.T) []string {
	t.Helper()

	root := toolgate.RepoRoot(t)

	var files []string

	err := filepath.WalkDir(
		filepath.Join(root, ".github"),
		func(path string, entry fs.DirEntry, err error) error {
			if err != nil {
				return err
			}

			if !entry.IsDir() &&
				(strings.HasSuffix(path, ".yaml") || strings.HasSuffix(path, ".yml")) {
				rel, relErr := filepath.Rel(root, path)
				if relErr != nil {
					return fmt.Errorf("relative path of %s: %w", path, relErr)
				}

				files = append(files, filepath.ToSlash(rel))
			}

			return nil
		},
	)
	if err != nil {
		t.Fatal(err)
	}

	if len(files) == 0 {
		t.Fatal("no YAML files under .github; the scan would check nothing")
	}

	return files
}

// glob lists the files directly in dir (relative to the repository root) with the suffix, as
// slash-separated paths relative to the root.
func glob(t *testing.T, dir, suffix string) []string {
	t.Helper()

	entries, err := os.ReadDir(filepath.Join(toolgate.RepoRoot(t), dir))
	if err != nil {
		t.Fatalf("list %s: %v", dir, err)
	}

	var files []string

	for _, entry := range entries {
		if entry.Type().IsRegular() && strings.HasSuffix(entry.Name(), suffix) {
			files = append(files, dir+"/"+entry.Name())
		}
	}

	return files
}
