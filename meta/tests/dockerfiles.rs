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

//! The shipped image: its one Dockerfile, the pinned frontend, the build and runtime stages, the
//! labels and the platforms, and hadolint's enforcement of the label schema.

use std::fs;
use std::process::{Command, Output};

use meta::dockerfile::{self, Stage};
use meta::{repo, toolgate};
use regex::Regex;

const SHIPPED_DIR: &str = "deployments/docker";
const SHIPPED_DOCKERFILE: &str = "deployments/docker/Dockerfile";
const RELEASE_WORKFLOW: &str = ".github/workflows/release.yaml";
const DOCKER_FRAGMENT: &str = ".mk/docker.mk";
const HADOLINT_CONFIG: &str = ".hadolint.yaml";
const WANT_PLATFORMS: &str = "linux/amd64,linux/arm64";

/// The seven OCI labels of the runtime stage; an empty value means any non-empty one.
const WANT_LABELS: &[(&str, &str)] = &[
    ("org.opencontainers.image.title", "awtrix-controller"),
    ("org.opencontainers.image.description", ""),
    (
        "org.opencontainers.image.source",
        "https://github.com/leinardi/awtrix-controller",
    ),
    ("org.opencontainers.image.version", "${VERSION}"),
    ("org.opencontainers.image.revision", "${COMMIT}"),
    ("org.opencontainers.image.created", "${DATE}"),
    ("org.opencontainers.image.licenses", "MIT"),
];

fn shipped_stages() -> Vec<Stage> {
    dockerfile::parse(&repo::read(SHIPPED_DOCKERFILE))
        .unwrap_or_else(|error| panic!("{SHIPPED_DOCKERFILE}: {error}"))
}

fn stage(name: &str) -> Stage {
    let found: Vec<Stage> = shipped_stages()
        .into_iter()
        .filter(|candidate| candidate.name == name)
        .collect();
    match <[Stage; 1]>::try_from(found) {
        Ok([stage]) => stage,
        Err(found) => panic!(
            "{SHIPPED_DOCKERFILE} has {} stages named {name}, want one",
            found.len()
        ),
    }
}

/// Go: `TestShippedDockerfileIsTheOnlyOne`. deployments/docker holds exactly the image that ships.
/// A second file there would look shipped to every reader and be built by nothing.
#[test]
fn shipped_dockerfile_is_the_only_one() {
    let mut found: Vec<String> = fs::read_dir(repo::path(SHIPPED_DIR))
        .expect("list deployments/docker")
        .map(|entry| {
            entry
                .expect("directory entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name == "Dockerfile" || name.ends_with(".Dockerfile"))
        .collect();
    found.sort();
    assert_eq!(found, ["Dockerfile"], "{SHIPPED_DIR} holds {found:?}");
}

/// Go: `TestSyntaxDirectiveIsPinned`. The `BuildKit` frontend that reads the Dockerfile is a
/// literal reference pinned by digest: a bare docker/dockerfile:1.7 is a tag that can be moved
/// under a build, and Dependabot never updates the line (docs/release.md).
#[test]
fn syntax_directive_is_pinned() {
    let syntax = dockerfile::syntax(&repo::read(SHIPPED_DOCKERFILE))
        .expect("the Dockerfile has a syntax parser directive on its first line");
    assert_eq!(
        dockerfile::repository(&syntax),
        "docker/dockerfile",
        "{syntax}"
    );
    assert!(
        dockerfile::digest_pinned(&syntax),
        "the frontend {syntax:?} is not pinned by digest"
    );
}

/// Go: `TestShippedStages`. Exactly two named stages, in order, each from a literal base pinned by
/// tag. The bases are deliberately not pinned by digest: dhi.io republishes its tags to ship
/// security fixes, and every release build is scanned. A variable base, a missing tag or latest
/// would leave the base unreviewable.
#[test]
fn shipped_stages_are_build_and_runtime() {
    let stages = shipped_stages();
    for stage in &stages {
        assert!(
            dockerfile::tag_pinned(&stage.image),
            "stage {:?} is FROM {:?}, want a literal reference with a tag other than latest",
            stage.name,
            stage.image
        );
    }
    let names: Vec<&str> = stages.iter().map(|stage| stage.name.as_str()).collect();
    assert_eq!(names, ["build", "runtime"]);
}

/// Go: `TestBuildStage`, for the Rust build: on the build platform, cross-compiled by
/// cargo-zigbuild to the target platform's static musl triple, from the locked dependency graph,
/// with the toolchain downloads checked, and stamped with the version strings the binary reads.
#[test]
fn build_stage() {
    let build = stage("build");
    assert_eq!(build.platform, "$BUILDPLATFORM");
    assert_eq!(dockerfile::repository(&build.image), "dhi.io/rust");

    let instructions = build.joined();
    let missing: Vec<&str> = [
        "ARG VERSION",
        "ARG COMMIT",
        "ARG DATE",
        "ARG RUST_VERSION=",
        "ARG ZIG_VERSION=",
        "ARG CARGO_ZIGBUILD_VERSION=",
        "ARG BUILDARCH",
        "ARG TARGETARCH",
        "amd64) triple=x86_64-unknown-linux-musl;",
        "arm64) triple=aarch64-unknown-linux-musl;",
        "sha256sum -c -",
        r#"cargo install --locked "cargo-zigbuild@${CARGO_ZIGBUILD_VERSION}""#,
        r#"cargo zigbuild --release --locked --target "${triple}" --bin awtrix-controller"#,
        r#"AWTRIX_CONTROLLER_VERSION="${VERSION}""#,
        r#"AWTRIX_CONTROLLER_COMMIT="${COMMIT}""#,
        r#"AWTRIX_CONTROLLER_DATE="${DATE}""#,
        r#"cp "target/${triple}/release/awtrix-controller" /out/awtrix-controller"#,
    ]
    .into_iter()
    .filter(|want| !instructions.contains(want))
    .collect();
    assert!(
        missing.is_empty(),
        "the build stage lacks {missing:?}:\n{instructions}"
    );

    // Every download is checked against its own digest: each curl in the stage is followed by a
    // sha256sum check of the file it wrote.
    let downloads = instructions.matches("curl -fsSL").count();
    let checks = instructions.matches("| sha256sum -c -").count();
    assert!(downloads > 0, "the build stage downloads nothing");
    assert_eq!(
        downloads, checks,
        "a download in the build stage is not checked"
    );
}

/// Go: `TestRuntimeStage`. The posture of the image that ships: the shell-less static base,
/// nothing run or written in it, the binary and the MQTT port, and exactly the seven OCI labels.
/// There is no USER instruction: the image relies on dhi.io/static's non-root default, and an
/// explicit root user would be a regression.
#[test]
fn runtime_stage() {
    let runtime = stage("runtime");
    assert_eq!(dockerfile::repository(&runtime.image), "dhi.io/static");
    assert!(
        runtime.platform.is_empty(),
        "the runtime stage pins --platform={}; it has to follow the target platform",
        runtime.platform
    );

    let mut problems = Vec::new();
    for instruction in &runtime.instructions {
        match instruction.keyword.as_str() {
            "RUN" | "SHELL" | "VOLUME" | "ADD" | "ENV" => problems.push(format!(
                "the runtime stage carries {} {}; it has no shell and writes nothing",
                instruction.keyword, instruction.args
            )),
            "USER" => {
                let user = instruction
                    .args
                    .split_once(':')
                    .map_or(instruction.args.as_str(), |(user, _)| user);
                if user == "root" || user == "0" {
                    problems.push(format!("the runtime stage runs as {}", instruction.args));
                }
            }
            _ => {}
        }
    }
    for (keyword, args) in [
        ("EXPOSE", "1883"),
        ("ENTRYPOINT", r#"["/usr/local/bin/awtrix-controller"]"#),
        (
            "COPY",
            "--from=build /out/awtrix-controller /usr/local/bin/awtrix-controller",
        ),
    ] {
        let seen: Vec<&str> = runtime
            .instructions
            .iter()
            .filter(|instruction| instruction.keyword == keyword)
            .map(|instruction| instruction.args.as_str())
            .collect();
        if seen != [args] {
            problems.push(format!(
                "the runtime stage has {keyword} {seen:?}, want exactly {keyword} {args}"
            ));
        }
    }

    let labels = dockerfile::labels(&runtime);
    for (key, value) in WANT_LABELS {
        let actual = labels.get(*key).map_or("", String::as_str);
        if actual.is_empty() || (!value.is_empty() && actual != *value) {
            problems.push(format!("label {key} is {actual:?}, want {value:?}"));
        }
    }
    if labels.len() != WANT_LABELS.len() {
        problems.push(format!(
            "the runtime stage carries labels {labels:?}, want exactly the seven OCI labels"
        ));
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// Go: `TestLabelArgumentsAreSupplied`. Every build that publishes or runs an image passes the
/// three build arguments the version, revision and created labels read: without them the labels
/// say dev, none and unknown.
#[test]
fn label_arguments_are_supplied() {
    for file in [RELEASE_WORKFLOW, DOCKER_FRAGMENT] {
        let content = repo::read(file);
        for argument in ["VERSION", "COMMIT", "DATE"] {
            let pattern = Regex::new(&format!(r"(?m)(^\s*|--build-arg ){argument}=\S"))
                .expect("valid pattern");
            assert!(
                pattern.is_match(&content),
                "{file} does not pass the {argument} build argument"
            );
        }
    }
}

/// Go: `TestPlatforms`. The two platforms the release builds: dhi.io/static has no arm/v7.
#[test]
fn platforms() {
    let content = repo::read(RELEASE_WORKFLOW);
    let platforms = Regex::new(r"(?m)^\s*PLATFORMS:\s*(\S+)\s*$")
        .expect("valid pattern")
        .captures(&content)
        .map(|captures| captures[1].to_owned());
    assert_eq!(platforms.as_deref(), Some(WANT_PLATFORMS));
}

fn run_hadolint(hadolint: &std::path::Path, dockerfile: &std::path::Path) -> Output {
    Command::new(hadolint)
        .arg("--no-color")
        .arg("--config")
        .arg(repo::path(HADOLINT_CONFIG))
        .arg(dockerfile)
        .output()
        .unwrap_or_else(|error| panic!("run hadolint: {error}"))
}

/// Go: `TestHadolintEnforcesTheLabels`. hadolint, with the repository's configuration, passes the
/// shipped Dockerfile and fails the same file without one of the seven labels with DL3049. Without
/// the error override a missing label is only info, which the failure threshold lets through.
#[test]
fn hadolint_enforces_the_labels() {
    let Some(hadolint) = toolgate::lookup(toolgate::HADOLINT) else {
        return;
    };

    let output = run_hadolint(&hadolint, &repo::path(SHIPPED_DOCKERFILE));
    assert!(
        output.status.success(),
        "hadolint rejects {SHIPPED_DOCKERFILE}:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );

    let shipped = repo::read(SHIPPED_DOCKERFILE);
    let without_version = Regex::new(r"(?m)^\s*org\.opencontainers\.image\.version=.*\n")
        .expect("valid pattern")
        .replace_all(&shipped, "")
        .into_owned();
    assert_ne!(
        without_version, shipped,
        "could not remove the version label from the fixture"
    );
    let scratch = tempfile::tempdir().expect("scratch directory");
    let scratch_dockerfile = scratch.path().join("Dockerfile");
    fs::write(&scratch_dockerfile, without_version).expect("write the fixture");

    let output = run_hadolint(&hadolint, &scratch_dockerfile);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success() && text.contains("DL3049"),
        "hadolint accepted a Dockerfile without the version label:\n{text}"
    );
}

/// Go: `TestParseRejectsWhatTheAssertionsMustNotMiss`, the negative control for the reader: each
/// of these inputs has to come out as something the assertions above refuse.
#[test]
fn parse_rejects_what_the_assertions_must_not_miss() {
    const DIGEST: &str = "@sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    for (name, image) in [
        ("no tag", "dhi.io/static"),
        ("latest", "dhi.io/static:latest"),
        ("a variable tag", "dhi.io/rust:${RUST_LINE}-debian13-dev"),
        ("a variable image", "$BASE"),
        ("a stage name, not a ref", "build"),
    ] {
        assert!(
            !dockerfile::tag_pinned(image),
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

    assert!(
        dockerfile::parse("FROM a:1\nRUN echo \\\n").is_err(),
        "a file ending inside a continuation parsed"
    );
}
