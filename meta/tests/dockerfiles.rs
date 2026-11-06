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
//! labels and the platforms, and droast's enforcement of the label schema. The rules that need no
//! tool are `repo_meta::image`'s, with this repository's parameters, and their negative controls
//! run them on altered copies of the real files; the label arguments and the droast runs (with
//! the tool gate `meta::TOOLS`) are this repository's own.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use repo_meta::dockerfile::{self, Stage};
use repo_meta::image::{
    BasePolicy, BuildBase, BuildStage, Download, RuntimeStage, STAMP_FIELDS, StampSources, Stamps,
    build_stage_problems, dockerfiles, label_policy_problems, label_problems,
    only_dockerfile_problems, platform_problems, runtime_problems, stage_problems, syntax_problems,
};
use repo_meta::{Problem, pattern, report, strings};

const SHIPPED_DOCKERFILE: &str = "deployments/docker/Dockerfile";
const RELEASE_WORKFLOW: &str = ".github/workflows/release.yaml";
const WORKFLOWS_DIR: &str = ".github/workflows";
const DOCKER_FRAGMENT: &str = ".mk/docker.mk";
const DROAST_CONFIG: &str = "droast.toml";
const SETUP_ZIGBUILD: &str = ".github/actions/setup-zigbuild/action.yaml";
const WANT_PLATFORMS: &str = "linux/amd64,linux/arm64";
/// dhi.io/static has no arm/v7, and no 386.
const FORBIDDEN_PLATFORMS: &[&str] = &["linux/arm/v", "linux/386"];
const STAGES: &[&str] = &["build", "runtime"];

/// The version stamps src/version.rs reads (through service-kit's `buildinfo!`), the build stage's
/// RUN that runs cargo zigbuild sets and make-common's rust.mk sets for `make rust-build`.
const STAMPS: Stamps<'static> = Stamps {
    prefix: "AWTRIX_CONTROLLER",
    fields: &STAMP_FIELDS,
    version_file: "src/version.rs",
    build_command: "cargo zigbuild",
};

/// The seven OCI labels of the runtime stage, exactly; `None` means any non-empty value.
const WANT_LABELS: &[(&str, Option<&str>)] = &[
    ("org.opencontainers.image.title", Some("awtrix-controller")),
    ("org.opencontainers.image.description", None),
    (
        "org.opencontainers.image.source",
        Some("https://github.com/leinardi/awtrix-controller"),
    ),
    ("org.opencontainers.image.version", Some("${VERSION}")),
    ("org.opencontainers.image.revision", Some("${COMMIT}")),
    ("org.opencontainers.image.created", Some("${DATE}")),
    ("org.opencontainers.image.licenses", Some("MIT")),
];

/// What the build stage's joined instructions must contain: the stamp arguments, the toolchain
/// pins, the platform arguments, the static musl triples, the checked downloads, the locked
/// cargo-zigbuild install and build, the version stamps and the copy of the binary.
const BUILD_REQUIRED: &[&str] = &[
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
];

fn shipped() -> String {
    meta::repo().read(SHIPPED_DOCKERFILE)
}

/// The stage named `name` of `content`, which must be the only one.
fn stage(content: &str, name: &str) -> Stage {
    let stages =
        dockerfile::parse(content).unwrap_or_else(|error| panic!("{SHIPPED_DOCKERFILE}: {error}"));
    dockerfile::stage_named(SHIPPED_DOCKERFILE, &stages, name)
        .cloned()
        .unwrap_or_else(|problem| panic!("{problem}"))
}

/// The value the setup-zigbuild action pins for `key`: CI's copy of the zig release the build
/// stage must download, so the build stage's rule does not read its own.
fn action_pin(key: &str) -> String {
    let action = meta::repo().read(SETUP_ZIGBUILD);
    pattern(&format!(r#"(?m)^\s*{key}:\s*"?([\d.]+)"?\s*$"#))
        .captures(&action)
        .map_or_else(
            || panic!("{SETUP_ZIGBUILD} pins no {key}"),
            |captures| captures[1].to_owned(),
        )
}

/// The exact release of the build stage's dhi.io/rust tag, which its musl standard libraries
/// must be downloaded for (`toolchain.rs` holds the tag to `ARG RUST_VERSION` and to the line of
/// rust-toolchain.toml); `<no exact release>` when the tag names none, which the rule refuses.
fn image_release(dockerfile: &str) -> String {
    let build = stage(dockerfile, "build");
    pattern(r"^(\d+\.\d+\.\d+)-")
        .captures(dockerfile::tag(&build.image))
        .map_or_else(
            || "<no exact release>".to_owned(),
            |captures| captures[1].to_owned(),
        )
}

/// The arms a download must have, each `(platform, asset)` with the sha256 the Dockerfile's arm
/// for exactly that platform and asset writes. The pairs are this repository's; a digest that is
/// not a sha256, or an arm with another asset, leaves the arm unread, so the rule refuses it.
fn arms(
    dockerfile: &str,
    asset_var: &str,
    sha_var: &str,
    pairs: &[(&str, &str)],
) -> Vec<(String, String, String)> {
    pairs
        .iter()
        .map(|(platform, asset)| {
            let digest = pattern(&format!(
                r"{}\)\s*{asset_var}={}; {sha_var}=([0-9a-f]{{64}}) ;;",
                regex::escape(platform),
                regex::escape(asset)
            ))
            .captures(dockerfile)
            .map_or_else(
                || "<no sha256 arm>".to_owned(),
                |captures| captures[1].to_owned(),
            );
            ((*platform).to_owned(), (*asset).to_owned(), digest)
        })
        .collect()
}

/// The build stage's two downloads: zig for the build platform at the release CI pins too, and
/// the target platform's musl standard library for the image's own Rust release; each in its own
/// RUN, chosen by its platform argument's `case` with exactly these arms, and checked with
/// `sha256sum -c -`. cargo-zigbuild is not downloaded: cargo installs it, `--locked`.
fn downloads(dockerfile: &str) -> Vec<Download<'static>> {
    vec![
        Download {
            tool: "zig",
            url: "https://ziglang.org/download/${ZIG_VERSION}/zig-${zig_arch}-linux-${ZIG_VERSION}.tar.xz",
            platform_arg: "BUILDARCH",
            version_arg: "ZIG_VERSION",
            version: action_pin("ZIG_VERSION"),
            asset_var: "zig_arch",
            sha_var: "zig_sha256",
            arms: arms(
                dockerfile,
                "zig_arch",
                "zig_sha256",
                &[("amd64", "x86_64"), ("arm64", "aarch64")],
            ),
        },
        Download {
            tool: "the musl rust-std",
            url: "https://static.rust-lang.org/dist/rust-std-${RUST_VERSION}-${triple}.tar.xz",
            platform_arg: "TARGETARCH",
            version_arg: "RUST_VERSION",
            version: image_release(dockerfile),
            asset_var: "triple",
            sha_var: "std_sha256",
            arms: arms(
                dockerfile,
                "triple",
                "std_sha256",
                &[
                    ("amd64", "x86_64-unknown-linux-musl"),
                    ("arm64", "aarch64-unknown-linux-musl"),
                ],
            ),
        },
    ]
}

/// The build stage's rules against `dockerfile`: on the build platform, from dhi.io/rust,
/// cross-compiled by cargo-zigbuild to the target platform's static musl triple from the locked
/// dependency graph, stamped, with every download a rule's and checked in the RUN that fetches it.
fn build_problems(dockerfile: &str) -> Vec<Problem> {
    let downloads = downloads(dockerfile);
    let rules = BuildStage {
        platform: "$BUILDPLATFORM",
        base: BuildBase::Repository("dhi.io/rust"),
        required: BUILD_REQUIRED,
        downloads: &downloads,
    };
    build_stage_problems(&stage(dockerfile, "build"), &rules)
}

/// The stamps of `dockerfile`, against src/version.rs, the Makefile and make-common's rust.mk.
fn stamp_problems(dockerfile: &str) -> Vec<Problem> {
    stamp_problems_with(dockerfile, &meta::repo().read(STAMPS.version_file))
}

/// [`stamp_problems`], against the version module `version_rs`.
fn stamp_problems_with(dockerfile: &str, version_rs: &str) -> Vec<Problem> {
    let repo = meta::repo();
    STAMPS.problems(&StampSources {
        version_rs,
        build: &stage(dockerfile, "build"),
        makefile: &repo.read("Makefile"),
        rust_mk: &repo.read(".mk/rust.mk"),
        release: None,
    })
}

/// The runtime stage's rules: the shell-less static base, nothing run or written in it, the
/// binary, the MQTT port and the entrypoint, exactly the seven OCI labels, and no root user.
/// There is no USER instruction: the image relies on dhi.io/static's non-root default, and an
/// explicit root user would be a regression (a rule repo-meta's runtime rules do not hold, so it
/// stays here).
fn runtime_stage_problems(dockerfile: &str) -> Vec<Problem> {
    let rules = RuntimeStage {
        repository: "dhi.io/static",
        forbidden: &["RUN", "SHELL", "VOLUME", "ADD", "ENV"],
        exactly: &[
            ("EXPOSE", "1883"),
            ("ENTRYPOINT", r#"["/usr/local/bin/awtrix-controller"]"#),
            (
                "COPY",
                "--from=build /out/awtrix-controller /usr/local/bin/awtrix-controller",
            ),
        ],
    };
    let runtime = stage(dockerfile, "runtime");
    let mut problems = runtime_problems(&runtime, &rules);
    for instruction in &runtime.instructions {
        if instruction.keyword == "USER" {
            let user = instruction
                .args
                .split_once(':')
                .map_or(instruction.args.as_str(), |(user, _)| user);
            if user == "root" || user == "0" {
                problems.push(Problem::new(format!(
                    "the runtime stage runs as {}",
                    instruction.args
                )));
            }
        }
    }
    problems.extend(label_problems(&runtime, WANT_LABELS));
    problems
}

/// Every workflow, with its text.
fn workflows() -> Vec<(String, String)> {
    let repo = meta::repo();
    repo.read_all(&repo.files_under(WORKFLOWS_DIR, &[".yaml", ".yml"]))
}

/// Go: `TestShippedDockerfileIsTheOnlyOne`. deployments/docker holds exactly the image that
/// ships, and the repository no other Dockerfile (tracked or new, `Dockerfile.<name>` and
/// `<name>.Dockerfile` included): a second one would look shipped to every reader and be built by
/// nothing.
#[test]
fn shipped_dockerfile_is_the_only_one() {
    let problems = only_dockerfile_problems(&dockerfiles(&meta::repo()), SHIPPED_DOCKERFILE);
    assert!(problems.is_empty(), "{}", report(&problems));
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

/// Go: `TestBuildStage`, for the Rust build: on the build platform, cross-compiled by
/// cargo-zigbuild to the target platform's static musl triple, from the locked dependency graph,
/// with every toolchain download checked against its own digest in the RUN that fetches it, and
/// stamped with the version strings the binary reads.
#[test]
fn build_stage() {
    let problems = build_problems(&shipped());
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// The version, commit and date the binary reports come from the build: the build stage (in the
/// RUN that runs cargo zigbuild) and the make targets set the three variables src/version.rs
/// reads at compile time (service-kit's `buildinfo!`), from the same build arguments and git
/// values. A path that stamped nothing would ship a binary reporting "dev". The release
/// workflow's binaries are stamped in `.github/workflows/release.yaml`, which this does not check.
#[test]
fn version_is_stamped() {
    let problems = stamp_problems(&shipped());
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// Go: `TestRuntimeStage`. The posture of the image that ships: the shell-less static base,
/// nothing run or written in it, the binary and the MQTT port, and exactly the seven OCI labels.
#[test]
fn runtime_stage() {
    let problems = runtime_stage_problems(&shipped());
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// The labels droast.toml requires are exactly the runtime stage's, strict-labels refuses any
/// other, and a missing label (DF074) keeps the severity droast ships, an error. The tool-free
/// half of droast's label schema; the droast runs below are the other.
#[test]
fn label_policy_matches_the_runtime_labels() {
    let repo = meta::repo();
    let problems = label_policy_problems(
        SHIPPED_DOCKERFILE,
        &shipped(),
        DROAST_CONFIG,
        &repo.read(DROAST_CONFIG),
    );
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// Go: `TestLabelArgumentsAreSupplied`. Every build that publishes or runs an image passes the
/// three build arguments the version, revision and created labels read: without them the labels
/// say dev, none and unknown.
#[test]
fn label_arguments_are_supplied() {
    let repo = meta::repo();
    for file in [RELEASE_WORKFLOW, DOCKER_FRAGMENT] {
        let content = repo.read(file);
        for argument in ["VERSION", "COMMIT", "DATE"] {
            assert!(
                pattern(&format!(r"(?m)(^\s*|--build-arg ){argument}=\S")).is_match(&content),
                "{file} does not pass the {argument} build argument"
            );
        }
    }
}

/// Go: `TestPlatforms`. The two platforms the release builds, wherever a workflow declares
/// platforms, the release workflow among them: dhi.io/static has no arm/v7.
#[test]
fn platforms() {
    let problems = platform_problems(
        &workflows(),
        RELEASE_WORKFLOW,
        WANT_PLATFORMS,
        FORBIDDEN_PLATFORMS,
    );
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// The label test, by name: the gate tests below run it again in a child process.
const LABEL_TEST: &str = "droast_enforces_the_labels";

/// The path of droast, after checking that the `shellcheck` its required bridge runs is on `PATH`
/// too. `None` when either is missing and the test should return early.
fn lookup_droast() -> Option<PathBuf> {
    let droast = meta::TOOLS.lookup(meta::DROAST)?;
    meta::TOOLS.lookup(meta::SHELLCHECK)?;
    Some(droast)
}

fn run_droast(droast: &Path, dockerfile: &Path) -> Output {
    Command::new(droast)
        .arg("--config")
        .arg(meta::repo().path(DROAST_CONFIG))
        .arg(dockerfile)
        .output()
        .unwrap_or_else(|error| panic!("run droast: {error}"))
}

/// Go's label test, now driving droast. droast, with the repository's configuration, passes the
/// shipped Dockerfile and fails the same file without one of the seven labels with DF074, a
/// missing required label, which is an error.
#[test]
fn droast_enforces_the_labels() {
    let Some(droast) = lookup_droast() else {
        return;
    };
    let repo = meta::repo();

    let output = run_droast(&droast, &repo.path(SHIPPED_DOCKERFILE));
    assert!(
        output.status.success(),
        "droast rejects {SHIPPED_DOCKERFILE}:\n{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let shipped = shipped();
    let without_version = pattern(r"(?m)^\s*org\.opencontainers\.image\.version=.*\n")
        .replace_all(&shipped, "")
        .into_owned();
    assert_ne!(
        without_version, shipped,
        "could not remove the version label from the fixture"
    );
    let scratch = tempfile::tempdir().expect("scratch directory");
    let scratch_dockerfile = scratch.path().join("Dockerfile");
    fs::write(&scratch_dockerfile, without_version).expect("write the fixture");

    let output = run_droast(&droast, &scratch_dockerfile);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        !output.status.success() && text.contains("DF074"),
        "droast accepted a Dockerfile without the version label:\n{text}"
    );
}

/// Runs the label test alone in a child process whose `PATH` holds only a stub `droast`, so
/// droast is found and the `shellcheck` its bridge needs is not; `require` sets the toolgate
/// variable to 1, otherwise it is unset.
fn run_label_test_without_shellcheck(require: bool) -> Output {
    let path = tempfile::tempdir()
        .unwrap_or_else(|error| panic!("create the scratch PATH directory: {error}"));
    let stub = path.path().join(meta::DROAST);
    fs::write(&stub, "#!/bin/sh\nexit 0\n")
        .unwrap_or_else(|error| panic!("write the stub droast: {error}"));
    fs::set_permissions(&stub, fs::Permissions::from_mode(0o755))
        .unwrap_or_else(|error| panic!("make the stub droast executable: {error}"));

    let test_binary =
        std::env::current_exe().unwrap_or_else(|error| panic!("find the test binary: {error}"));
    let mut command = Command::new(test_binary);
    command
        .args([LABEL_TEST, "--exact", "--nocapture", "--test-threads=1"])
        .env("PATH", path.path());
    if require {
        command.env(meta::TOOLS.require_env, "1");
    } else {
        command.env_remove(meta::TOOLS.require_env);
    }
    command
        .output()
        .unwrap_or_else(|error| panic!("run {LABEL_TEST} in a child process: {error}"))
}

/// The label test with droast but no shellcheck, outside CI: it returns early, passing, and says
/// that it did not run and why.
#[test]
fn label_test_skips_without_shellcheck_by_default() {
    let output = run_label_test_without_shellcheck(false);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success() && stdout.contains("1 passed"),
        "{LABEL_TEST} did not pass without shellcheck:\n{stdout}{stderr}"
    );
    assert!(
        stderr.contains("shellcheck not found on PATH"),
        "{LABEL_TEST} skipped without naming the missing shellcheck:\n{stderr}"
    );
}

/// The label test with droast but no shellcheck, as CI runs it: a failure, not a skip.
#[test]
fn label_test_fails_without_shellcheck_when_tools_are_required() {
    let output = run_label_test_without_shellcheck(true);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        !output.status.success() && stdout.contains("1 failed"),
        "{LABEL_TEST} did not fail without shellcheck under {}=1:\n{stdout}{stderr}",
        meta::TOOLS.require_env
    );
    assert!(
        format!("{stdout}{stderr}").contains("shellcheck is required"),
        "{LABEL_TEST} failed for another reason than the missing shellcheck:\n{stdout}{stderr}"
    );
}

/// `content` with its first `from` replaced by `to`; `from` must be there, so an edit cannot
/// silently change nothing.
fn edited(content: &str, from: &str, to: &str) -> String {
    assert!(content.contains(from), "{from:?} is gone");
    content.replacen(from, to, 1)
}

/// A negative control of the build stage rules: each edit of the real Dockerfile below breaks
/// exactly one rule, and the rules report exactly that one problem.
#[test]
fn build_rules_refuse_each_edit() {
    let content = shipped();
    let edit = |from: &str, to: &str| edited(&content, from, to);
    let zig_check = "\t&& echo \"${zig_sha256}  /tmp/zig.tar.xz\" | sha256sum -c - \\\n";
    for (name, broken, want) in [
        (
            "an extra download beside a listed one",
            edit(
                zig_check,
                &format!("{zig_check}\t&& curl -fsSL -o /tmp/x https://example.invalid/x \\\n"),
            ),
            2,
        ),
        ("an unchecked download", edit(zig_check, ""), 2),
        (
            "an arm for another platform",
            edit(
                "\t\t*) echo \"no zig for",
                "\t\tarm) zig_arch=arm; zig_sha256=0000000000000000000000000000000000000000000000000000000000000000 ;; \\\n\t\t*) echo \"no zig for",
            ),
            1,
        ),
        (
            "a zig release other than CI's",
            edit("ARG ZIG_VERSION=", "ARG ZIG_VERSION=9"),
            1,
        ),
        (
            "rust-std for another release than the image's",
            edit("ARG RUST_VERSION=", "ARG RUST_VERSION=9"),
            1,
        ),
        (
            "a fetch on its own, unchecked and unlisted",
            edit(
                "WORKDIR /src\n",
                "RUN curl -fsSL -o /tmp/y https://example.invalid/y\nWORKDIR /src\n",
            ),
            2,
        ),
        (
            "an ADD of a URL",
            edit(
                "WORKDIR /src\n",
                "ADD https://example.invalid/z /tmp/z\nWORKDIR /src\n",
            ),
            1,
        ),
        (
            "another build image repository",
            edit("dhi.io/rust:", "docker.io/library/rust:"),
            1,
        ),
        (
            "the build stage on the target platform",
            edit("--platform=$BUILDPLATFORM ", ""),
            1,
        ),
        (
            "an unlocked build",
            edit("--release --locked --target", "--release --target"),
            1,
        ),
        (
            "an unstamped build",
            edit("AWTRIX_CONTROLLER_DATE=\"${DATE}\" \\\n", "\\\n"),
            1,
        ),
    ] {
        let problems = build_problems(&broken);
        assert_eq!(
            problems.len(),
            want,
            "{name}: want {want} problems, got:\n{}",
            report(&problems)
        );
    }
}

/// The negative control of the stamp rules: one stamp out of the building RUN, and altered copies
/// of the real src/version.rs (its `buildinfo!` under another prefix, the call commented out, and
/// a `buildinfo!` that is not service-kit's), each of which reads none of the three stamps.
#[test]
fn stamp_rules_refuse_each_edit() {
    let shipped = shipped();
    let unstamped = edited(&shipped, "AWTRIX_CONTROLLER_DATE=\"${DATE}\" \\\n", "\\\n");
    assert_eq!(
        strings(&stamp_problems(&unstamped)),
        ["the RUN that runs cargo zigbuild does not stamp AWTRIX_CONTROLLER_DATE from $DATE"]
    );

    let version_rs = meta::repo().read(STAMPS.version_file);
    let altered = |from: &str, to: &str| {
        let altered = version_rs.replacen(from, to, 1);
        assert_ne!(
            altered, version_rs,
            "the alteration {from:?} changed nothing"
        );
        altered
    };
    let unread = |field: &str| format!("src/version.rs does not read AWTRIX_CONTROLLER_{field}");
    let want = [unread("VERSION"), unread("COMMIT"), unread("DATE")];
    for (name, broken) in [
        (
            "another prefix",
            altered("\x22AWTRIX_CONTROLLER\x22,", "\x22AWTRIX\x22,"),
        ),
        (
            "the call commented out",
            altered("const BUILD", "// const BUILD"),
        ),
        (
            "a buildinfo! that is not service-kit's",
            altered(
                "use service_kit::{BuildInfo, Defaults, EmptyStamp, buildinfo};",
                "use service_kit::{BuildInfo, Defaults, EmptyStamp};",
            ),
        ),
    ] {
        assert_eq!(
            strings(&stamp_problems_with(&shipped, &broken)),
            want,
            "{name} was accepted"
        );
    }
}

/// A negative control of the runtime rules: each edit breaks one rule, and exactly one problem
/// comes back.
#[test]
fn runtime_rules_refuse_each_edit() {
    let content = shipped();
    let edit = |from: &str, to: &str| edited(&content, from, to);
    for (name, broken) in [
        (
            "a shell in the runtime stage",
            edit(
                "# Copy the binary\n",
                "SHELL [\"/bin/sh\", \"-c\"]\n# Copy the binary\n",
            ),
        ),
        (
            "a RUN in the runtime stage",
            edit("# Copy the binary\n", "RUN true\n# Copy the binary\n"),
        ),
        (
            "an ENV in the runtime stage",
            edit("# Copy the binary\n", "ENV X=1\n# Copy the binary\n"),
        ),
        (
            "root",
            edit("# Copy the binary\n", "USER root\n# Copy the binary\n"),
        ),
        (
            "uid 0",
            edit("# Copy the binary\n", "USER 0:0\n# Copy the binary\n"),
        ),
        ("another port", edit("EXPOSE 1883", "EXPOSE 1884")),
        (
            "a second port",
            edit("EXPOSE 1883\n", "EXPOSE 1883\nEXPOSE 8883\n"),
        ),
        (
            "another entrypoint",
            edit(
                r#"ENTRYPOINT ["/usr/local/bin/awtrix-controller"]"#,
                r#"ENTRYPOINT ["/bin/sh"]"#,
            ),
        ),
        (
            "the runtime stage on a pinned platform",
            edit(
                "FROM dhi.io/static:",
                "FROM --platform=linux/amd64 dhi.io/static:",
            ),
        ),
        (
            "another runtime base",
            edit("FROM dhi.io/static:", "FROM dhi.io/debian-base:"),
        ),
        (
            "an extra label",
            edit(
                "org.opencontainers.image.licenses=\"MIT\"",
                "org.opencontainers.image.licenses=\"MIT\" extra.label=\"x\"",
            ),
        ),
        (
            "another licence",
            edit(
                "org.opencontainers.image.licenses=\"MIT\"",
                "org.opencontainers.image.licenses=\"Apache-2.0\"",
            ),
        ),
        (
            "an empty description",
            edit(
                "org.opencontainers.image.description=\"Awtrix3 MQTT Controller\"",
                "org.opencontainers.image.description=\"\"",
            ),
        ),
        (
            "a missing label",
            pattern(r"(?m)^\s*org\.opencontainers\.image\.created=.*\n")
                .replace(&content, "")
                .into_owned(),
        ),
    ] {
        assert_ne!(broken, content, "{name} changed nothing");
        let problems = runtime_stage_problems(&broken);
        assert_eq!(
            problems.len(),
            1,
            "{name}: want one problem, got:\n{}",
            report(&problems)
        );
    }
}

/// A negative control of the file, frontend and stage rules: each edit of the real Dockerfile
/// breaks one rule, and exactly one problem comes back.
#[test]
fn file_frontend_and_stage_rules_refuse_each_edit() {
    let content = shipped();
    let edit = |from: &str, to: &str| edited(&content, from, to);

    assert_eq!(
        strings(&only_dockerfile_problems(
            &[
                SHIPPED_DOCKERFILE.to_owned(),
                "deployments/docker/Dockerfile.debug".to_owned(),
            ],
            SHIPPED_DOCKERFILE,
        )),
        [
            "the repository holds the Dockerfiles [\"deployments/docker/Dockerfile\", \
             \"deployments/docker/Dockerfile.debug\"]; want only deployments/docker/Dockerfile"
        ]
    );

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

/// A negative control of the label-policy and platform rules: each edit of the real droast.toml
/// and release workflow breaks one rule, and exactly that comes back.
#[test]
fn label_policy_and_platform_rules_refuse_each_edit() {
    let content = shipped();
    let droast = meta::repo().read(DROAST_CONFIG);
    for (name, config) in [
        (
            "labels not strict",
            droast.replacen("strict-labels = true", "strict-labels = false", 1),
        ),
        (
            "DF074 lowered",
            droast.replacen(
                "[severity-overrides]\n",
                "[severity-overrides]\nDF074 = \"warning\"\n",
                1,
            ),
        ),
        (
            "a label droast does not require",
            droast.replacen("\"org.opencontainers.image.licenses\" = \"spdx\"\n", "", 1),
        ),
    ] {
        assert_ne!(config, droast, "{name} changed nothing");
        let problems = label_policy_problems(SHIPPED_DOCKERFILE, &content, DROAST_CONFIG, &config);
        assert_eq!(problems.len(), 1, "{name}: {}", report(&problems));
    }

    let workflows = workflows();
    let release = workflows
        .iter()
        .position(|(file, _)| file == RELEASE_WORKFLOW)
        .unwrap_or_else(|| panic!("no {RELEASE_WORKFLOW}"));
    let mut broken = workflows.clone();
    broken[release].1 = edited(
        &broken[release].1,
        &format!("PLATFORMS: {WANT_PLATFORMS}"),
        "PLATFORMS: linux/amd64",
    );
    assert_eq!(
        platform_problems(
            &broken,
            RELEASE_WORKFLOW,
            WANT_PLATFORMS,
            FORBIDDEN_PLATFORMS
        )
        .len(),
        1,
        "one platform"
    );
    broken[release].1 = edited(
        &workflows[release].1,
        &format!("PLATFORMS: {WANT_PLATFORMS}"),
        "PLATFORMS: linux/amd64,linux/arm64,linux/arm/v7",
    );
    assert_eq!(
        platform_problems(
            &broken,
            RELEASE_WORKFLOW,
            WANT_PLATFORMS,
            FORBIDDEN_PLATFORMS
        )
        .len(),
        2,
        "a third platform, which is also a forbidden one"
    );
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
