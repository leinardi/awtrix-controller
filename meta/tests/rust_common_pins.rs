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

//! The rust-common pins: every dependency on rust-common in the root `Cargo.toml` has a `tag` of
//! the form `vX.Y.Z`, a `version` equal to that tag's `X.Y`, and all of them share one tag. The
//! check is `repo_meta::git_pins`; the negative control runs it on an altered copy of the real
//! manifest.

use repo_meta::git_pins::{RUST_COMMON_URL, git_pins, root_git_pins};
use repo_meta::report;

/// The pins hold: no partial move of the tag, and no `version` left behind by it.
#[test]
fn rust_common_pins_agree() {
    let problems = root_git_pins(&meta::repo(), RUST_COMMON_URL);
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// The negative control, on the real manifest: one line's tag moved alone, and another line's
/// `version` left on another line, are each refused.
#[test]
fn pin_rules_refuse_each_edit() {
    let manifest = meta::repo().read("Cargo.toml");
    let lines: Vec<&str> = manifest
        .lines()
        .filter(|line| line.contains(&format!("git = \"{RUST_COMMON_URL}\"")))
        .collect();
    assert!(
        lines.len() >= 2,
        "fewer than two rust-common lines in Cargo.toml; the control needs two"
    );
    let tag = r#"tag = "v"#;
    let version = r#"version = ""#;
    let other_tag = lines[0].replacen(tag, r#"tag = "v9."#, 1);
    let other_version = lines[1].replacen(version, r#"version = "9."#, 1);
    assert!(
        other_tag != lines[0] && other_version != lines[1],
        "the lines carry no tag or version to alter"
    );

    let cases = [
        (
            "a tag moved on one line",
            manifest.replacen(lines[0], &other_tag, 1),
        ),
        (
            "a version of another line than its tag",
            manifest.replacen(lines[1], &other_version, 1),
        ),
    ];
    for (name, altered) in cases {
        let problems = git_pins(&[("Cargo.toml", &altered)], RUST_COMMON_URL);
        assert!(!problems.is_empty(), "{name} was accepted");
    }
}
