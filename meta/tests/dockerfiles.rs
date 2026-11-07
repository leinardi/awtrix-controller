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

//! The shipped image: the pinned frontend and the two stages. The frontend and stage rules are
//! `repo_meta::image`'s, with this repository's parameters, and their negative controls run them on
//! altered copies of the real file. The `droast` hook enforces the label schema.

use repo_meta::dockerfile;
use repo_meta::image::{BasePolicy, stage_problems, syntax_problems};
use repo_meta::report;

const SHIPPED_DOCKERFILE: &str = "deployments/docker/Dockerfile";
const STAGES: &[&str] = &["build", "runtime"];

fn shipped() -> String {
    meta::repo().read(SHIPPED_DOCKERFILE)
}

/// Go: `TestSyntaxDirectiveIsPinned`. The `BuildKit` frontend that reads the Dockerfile is a
/// literal docker/dockerfile reference pinned by tag and digest: a bare docker/dockerfile:1.7 is a
/// tag that can be moved under a build, and Dependabot never updates the line (docs/release.md).
#[test]
fn syntax_directive_is_pinned() {
    let problems = syntax_problems(SHIPPED_DOCKERFILE, &shipped(), true);
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// Go: `TestShippedStages`. Exactly two named stages, in order, each from a literal base pinned by
/// tag alone (`BasePolicy::TagOnly`; the build and runtime rules hold each to its dhi.io
/// repository). The bases are deliberately not pinned by digest: dhi.io republishes its tags to
/// ship security fixes, and every release build is scanned. A variable base, a missing tag or
/// latest would leave the base unreviewable.
#[test]
fn shipped_stages_are_build_and_runtime() {
    let problems = stage_problems(SHIPPED_DOCKERFILE, &shipped(), STAGES, BasePolicy::TagOnly);
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// `content` with its first `from` replaced by `to`; `from` must be there, so an edit cannot
/// silently change nothing.
fn edited(content: &str, from: &str, to: &str) -> String {
    assert!(content.contains(from), "{from:?} is gone");
    content.replacen(from, to, 1)
}

/// A negative control of the file, frontend and stage rules: each edit of the real Dockerfile
/// breaks one rule, and exactly one problem comes back.
#[test]
fn file_frontend_and_stage_rules_refuse_each_edit() {
    let content = shipped();
    let edit = |from: &str, to: &str| edited(&content, from, to);

    let syntax = dockerfile::syntax(&content).expect("the shipped Dockerfile has a frontend");
    let unpinned = syntax
        .split_once('@')
        .map_or(syntax.as_str(), |(tag, _)| tag);
    for (name, broken) in [
        ("a frontend by tag alone", edit(&syntax, unpinned)),
        (
            "another frontend",
            edit("docker/dockerfile:", "example/dockerfile:"),
        ),
        ("no frontend", edit(&format!("# syntax={syntax}\n"), "")),
    ] {
        let problems = syntax_problems(SHIPPED_DOCKERFILE, &broken, true);
        assert_eq!(problems.len(), 1, "{name}: {}", report(&problems));
    }

    for (name, broken) in [
        (
            "a runtime base by latest",
            edit("FROM dhi.io/static:20250419", "FROM dhi.io/static:latest"),
        ),
        (
            "a runtime base without a tag",
            edit("FROM dhi.io/static:20250419", "FROM dhi.io/static"),
        ),
        (
            "a base by tag and digest",
            edit(
                "FROM dhi.io/static:20250419",
                "FROM dhi.io/static:20250419@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            ),
        ),
        (
            "a variable base",
            edit("FROM dhi.io/static:20250419", "FROM $BASE"),
        ),
        ("a renamed stage", edit("AS runtime", "AS final")),
        (
            "a third stage",
            format!("{content}\nFROM dhi.io/static:20250419 AS extra\n"),
        ),
    ] {
        let problems = stage_problems(SHIPPED_DOCKERFILE, &broken, STAGES, BasePolicy::TagOnly);
        assert_eq!(problems.len(), 1, "{name}: {}", report(&problems));
    }
}

/// Go: `TestParseRejectsWhatTheAssertionsMustNotMiss`, the negative control for the reader: each
/// of these inputs has to come out as something the assertions above refuse.
#[test]
fn parse_rejects_what_the_assertions_must_not_miss() {
    const DIGEST: &str = "@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    for (name, image) in [
        ("no tag", "dhi.io/static".to_owned()),
        ("latest", "dhi.io/static:latest".to_owned()),
        (
            "a variable tag",
            "dhi.io/rust:${RUST_LINE}-debian13-dev".to_owned(),
        ),
        ("a variable image", "$BASE".to_owned()),
        ("a stage name, not a ref", "build".to_owned()),
        (
            "a tag and a digest",
            format!("dhi.io/static:20250419{DIGEST}"),
        ),
    ] {
        assert!(
            !dockerfile::tag_pinned(&image),
            "{name} ({image:?}) passes as tag-pinned"
        );
    }
    assert!(dockerfile::tag_pinned("dhi.io/static:20250419"));

    for (name, image) in [
        ("a tag only", "docker/dockerfile:1.7".to_owned()),
        (
            "a short digest",
            "docker/dockerfile:1.7@sha256:0123".to_owned(),
        ),
        ("a digest with no tag", format!("docker/dockerfile{DIGEST}")),
    ] {
        assert!(
            !dockerfile::digest_pinned(&image),
            "{name} ({image:?}) passes as digest-pinned"
        );
    }
    assert!(dockerfile::digest_pinned(&format!(
        "docker/dockerfile:1.7{DIGEST}"
    )));

    let stages =
        dockerfile::parse("FROM a:1 AS one\nRUN echo \\\n  two\n# note\nFROM b:2\nUSER 1:1\n")
            .expect("parse");
    assert_eq!(stages.len(), 2, "{stages:?}");
    assert_eq!(stages[0].name, "one");
    assert_eq!(stages[1].name, "");
    assert_eq!(stages[0].joined(), "RUN echo two");
    assert_eq!(stages[1].joined(), "USER 1:1");

    for (name, content) in [
        ("no directive", "FROM a:1\n".to_owned()),
        (
            "after a comment",
            format!("# note\n# syntax=docker/dockerfile:1.7{DIGEST}\n"),
        ),
        (
            "after a blank line",
            format!("\n# syntax=docker/dockerfile:1.7{DIGEST}\n"),
        ),
        (
            "after an instruction",
            format!("FROM a:1\n# syntax=docker/dockerfile:1.7{DIGEST}\n"),
        ),
    ] {
        assert_eq!(
            dockerfile::syntax(&content),
            None,
            "{name}: read a syntax directive BuildKit ignores"
        );
    }

    for (name, content) in [
        (
            "a file ending inside a continuation",
            "FROM a:1\nRUN echo \\\n",
        ),
        ("another escape character", "# escape=`\nFROM a:1\n"),
        ("a heredoc", "FROM a:1\nRUN <<EOF\necho\nEOF\n"),
    ] {
        assert!(dockerfile::parse(content).is_err(), "{name} parsed");
    }
}
