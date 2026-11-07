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

//! The repository's tooling pins: every version written in more than one file is one value (the
//! Rust line and the image's release, zig, cargo-zigbuild, droast, `ShellCheck` and Trivy), and no
//! flag makes a gate pass on nothing. The checks are `repo_meta::toolchain::one_value` over this
//! repository's pin sources and `repo_meta::vacuous`'s; the negative controls run them on altered
//! copies of the real files and on samples.

use repo_meta::toolchain::{PinSource, one_value, rust_release};
use repo_meta::vacuous::{AllowedFragment, VacuousFlags};
use repo_meta::{Problem, report};

const DOCKERFILE: &str = "deployments/docker/Dockerfile";
const RUST_TOOLCHAIN: &str = "rust-toolchain.toml";
const SETUP_ZIGBUILD: &str = ".github/actions/setup-zigbuild/action.yaml";
const SETUP_DROAST: &str = ".github/actions/setup-droast/action.yaml";
const SETUP_SHELLCHECK: &str = ".github/actions/setup-shellcheck/action.yaml";
const PRE_COMMIT: &str = ".pre-commit-config.yaml";
const CI_WORKFLOW: &str = ".github/workflows/ci.yaml";
const RELEASE_WORKFLOW: &str = ".github/workflows/release.yaml";
const SCHEDULED_SCAN: &str = ".github/workflows/scheduled-scan.yaml";

/// The `X.Y` line of the release in the `dhi.io/rust` tag of a `FROM`.
const IMAGE_TAG_LINE: &str = r"(?mi)^[ \t]*FROM[ \t]+(?:--\S+[ \t]+)*dhi\.io/rust:(\d+\.\d+)\.\d+-";
const ZIG_DOCKERFILE: &str = r"(?m)^ARG ZIG_VERSION=(\S+)$";
const ZIG_ACTION: &str = r#"ZIG_VERSION:\s*"?([\d.]+)"#;
const ZIGBUILD_DOCKERFILE: &str = r"(?m)^ARG CARGO_ZIGBUILD_VERSION=(\S+)$";
const ZIGBUILD_ACTION: &str = r#"CARGO_ZIGBUILD_VERSION:\s*"?([\d.]+)"#;
const DROAST_HOOK: &str =
    r"repo: https://github\.com/immanuwell/dockerfile-roast\s+rev:\s*([\d.]+)";
const DROAST_ACTION: &str = r#"DROAST_VERSION:\s*["']?([\d.]+)"#;
/// shellcheck-py adds a fourth component of its own (v0.11.0.1), so only the first three count.
const SHELLCHECK_HOOK: &str =
    r"repo: https://github\.com/shellcheck-py/shellcheck-py\s+rev:\s*v?(\d+\.\d+\.\d+)(?:\.\d+)?";
const SHELLCHECK_ACTION: &str = r#"SHELLCHECK_VERSION:\s*"?v?(\d+\.\d+\.\d+)"#;
const TRIVY: &str = r"(?m)^\s*TRIVY_VERSION:\s*(v[\d.]+)";

/// Every YAML file under .github, with its text; none is a failure, since the scans would check
/// nothing.
fn github_yaml() -> Vec<(String, String)> {
    let repo = meta::repo();
    let files = repo.files_under(".github", &[".yaml", ".yml"]);
    assert!(
        !files.is_empty(),
        "no YAML files under .github; the scan would check nothing"
    );
    repo.read_all(&files)
}

/// `content` with its first `from` replaced by `to`; `from` must be there, so an edit cannot
/// silently change nothing.
fn edited(content: &str, from: &str, to: &str) -> String {
    assert!(content.contains(from), "{from:?} is gone");
    content.replacen(from, to, 1)
}

/// What the pin checks read of a file: `edit` applied to its path and its content.
type Edit<'edit> = &'edit dyn Fn(&str, String) -> String;

/// A source of `pattern` in `file`, whose content `edit` has applied.
fn source(edit: Edit, file: &str, pattern: &str) -> PinSource {
    PinSource::new(file, &edit(file, meta::repo().read(file)), pattern)
}

/// The same file for a preset source.
fn read(edit: Edit, file: &str) -> String {
    edit(file, meta::repo().read(file))
}

/// Every pin the repository writes in more than one file, each one value: the Rust line
/// (rust-toolchain.toml, Cargo's `rust-version`, the image's tag), the image's exact Rust release
/// (its tag and `ARG RUST_VERSION`, the release the musl standard libraries are downloaded for),
/// zig, cargo-zigbuild, droast, `ShellCheck` and Trivy. `edit` is applied to the content of each
/// file read.
fn pin_problems(edit: Edit) -> Vec<Problem> {
    let dockerfile = read(edit, DOCKERFILE);
    let mut problems = one_value(
        "the Rust line",
        &[
            PinSource::rust_channel_line(RUST_TOOLCHAIN, &read(edit, RUST_TOOLCHAIN)),
            PinSource::rust_version_line("Cargo.toml", &read(edit, "Cargo.toml")),
            PinSource::new(DOCKERFILE, &dockerfile, IMAGE_TAG_LINE),
        ],
        &[],
    );
    problems.extend(one_value(
        "the image's Rust release",
        &[
            PinSource::rust_image_tag(DOCKERFILE, &dockerfile),
            PinSource::rust_version_arg(DOCKERFILE, &dockerfile),
        ],
        &[],
    ));
    problems.extend(one_value(
        "zig",
        &[
            source(edit, DOCKERFILE, ZIG_DOCKERFILE),
            source(edit, SETUP_ZIGBUILD, ZIG_ACTION),
        ],
        &[],
    ));
    problems.extend(one_value(
        "cargo-zigbuild",
        &[
            source(edit, DOCKERFILE, ZIGBUILD_DOCKERFILE),
            source(edit, SETUP_ZIGBUILD, ZIGBUILD_ACTION),
        ],
        &[],
    ));
    problems.extend(one_value(
        "droast",
        &[
            source(edit, PRE_COMMIT, DROAST_HOOK),
            source(edit, SETUP_DROAST, DROAST_ACTION),
        ],
        &[],
    ));
    problems.extend(one_value(
        "ShellCheck",
        &[
            source(edit, PRE_COMMIT, SHELLCHECK_HOOK),
            source(edit, SETUP_SHELLCHECK, SHELLCHECK_ACTION),
        ],
        &[],
    ));
    problems.extend(one_value(
        "Trivy",
        &[
            source(edit, RELEASE_WORKFLOW, TRIVY),
            source(edit, SCHEDULED_SCAN, TRIVY),
        ],
        &[],
    ));
    problems
}

/// The pins hold: rust-toolchain.toml pins an exact release and the MSRV in Cargo.toml and the
/// image's tag are its line; the image's tag and `ARG RUST_VERSION` are one release; the release
/// binaries and the image are built with the same zig and cargo-zigbuild; the droast CI installs is
/// the release the pre-commit hook builds from source; droast's `ShellCheck` bridge runs the release
/// the shellcheck-py hook wraps; and the release and the scheduled scan run one Trivy.
#[test]
fn pins_are_one_value() {
    let problems = pin_problems(&|_, content| content);
    assert!(problems.is_empty(), "{}", report(&problems));
    rust_release(RUST_TOOLCHAIN, &meta::repo().read(RUST_TOOLCHAIN))
        .unwrap_or_else(|problem| panic!("{problem}"));
}

/// One negative control: `from` replaced by `to` in `file` breaks `want` problems.
type Refusal = (&'static str, &'static str, String, String, usize);

/// Each edit of the real files is refused with the problems it breaks.
fn assert_refused(refusals: Vec<Refusal>) {
    for (name, file, from, to, want) in refusals {
        let problems = pin_problems(&|path, content| {
            if path == file {
                edited(&content, &from, &to)
            } else {
                content
            }
        });
        assert_eq!(problems.len(), want, "{name}: {}", report(&problems));
    }
}

/// The negative control of the Rust pins, on the real files: another line or release, or a source
/// that no longer writes the pin.
#[test]
fn rust_pin_rules_refuse_each_edit() {
    let release = rust_release(RUST_TOOLCHAIN, &meta::repo().read(RUST_TOOLCHAIN))
        .unwrap_or_else(|problem| panic!("{problem}"));
    let line = release
        .rsplit_once('.')
        .map_or(release.as_str(), |(line, _)| line);
    let other_line = format!("{line}0");
    let channel = format!("channel = \"{release}\"");
    let msrv = format!("rust-version = \"{line}\"");
    let image_tag = format!("dhi.io/rust:{release}-");
    let argument = format!("ARG RUST_VERSION={release}");

    assert_refused(vec![
        (
            "a toolchain of another line",
            RUST_TOOLCHAIN,
            channel.clone(),
            format!("channel = \"{other_line}.1\""),
            1,
        ),
        (
            "a channel that is not a release",
            RUST_TOOLCHAIN,
            channel,
            "channel = \"stable\"".to_owned(),
            1,
        ),
        (
            "an MSRV of another line",
            "Cargo.toml",
            msrv,
            format!("rust-version = \"{other_line}\""),
            1,
        ),
        (
            "an image of another line",
            DOCKERFILE,
            image_tag,
            format!("dhi.io/rust:{other_line}.1-"),
            2,
        ),
        (
            "standard libraries of another patch",
            DOCKERFILE,
            argument,
            format!("ARG RUST_VERSION={line}.99"),
            1,
        ),
    ]);
}

/// The negative control of the tool pins, on the real files: a second version, or a source that no
/// longer writes the pin.
#[test]
fn tool_pin_rules_refuse_each_edit() {
    assert_refused(vec![
        (
            "zig",
            DOCKERFILE,
            "ARG ZIG_VERSION=".to_owned(),
            "ARG ZIG_VERSION=9".to_owned(),
            1,
        ),
        (
            "zig in CI",
            SETUP_ZIGBUILD,
            "ZIG_VERSION:".to_owned(),
            "ZIG_VERSION_GONE:".to_owned(),
            1,
        ),
        (
            "cargo-zigbuild",
            SETUP_ZIGBUILD,
            "CARGO_ZIGBUILD_VERSION: \"".to_owned(),
            "CARGO_ZIGBUILD_VERSION: \"9".to_owned(),
            1,
        ),
        (
            "droast",
            SETUP_DROAST,
            "DROAST_VERSION:".to_owned(),
            "DROAST_VERSION: 9".to_owned(),
            1,
        ),
        (
            "ShellCheck",
            SETUP_SHELLCHECK,
            "SHELLCHECK_VERSION: v".to_owned(),
            "SHELLCHECK_VERSION: v9.".to_owned(),
            1,
        ),
        (
            "Trivy in the release",
            RELEASE_WORKFLOW,
            "TRIVY_VERSION: v".to_owned(),
            "TRIVY_VERSION: v9.".to_owned(),
            1,
        ),
        (
            "Trivy in the scheduled scan",
            SCHEDULED_SCAN,
            "TRIVY_VERSION:".to_owned(),
            "TRIVY_VERSION_GONE:".to_owned(),
            1,
        ),
    ]);
}

/// The flags no gate may carry: the core set, the Rust set with the rule that clippy denies
/// warnings.
fn vacuous_flags() -> VacuousFlags {
    VacuousFlags::core().with_rust()
}

/// The reviewed exceptions, each an exact fragment of one file. Each must still match at least
/// one line, so an entry cannot outlive the code it excuses and quietly cover a new line later.
const ALLOWED: &[AllowedFragment<'static>] = &[
    // grep exits 1 when it selects nothing, and "no release tag yet" is an answer here, not a
    // failure; the brace group confines the || true to that one grep under pipefail.
    AllowedFragment {
        file: RELEASE_WORKFLOW,
        fragment: r#"{ grep -E "$RELEASE_TAG_REGEX" || true; }"#,
        reason: "a tag filter that may select nothing",
    },
    // The first release has no HEAD^ history with tags; the empty answer means "no previous
    // release", which the step handles.
    AllowedFragment {
        file: RELEASE_WORKFLOW,
        fragment: r#"{ git tag --merged "HEAD^" 2>/dev/null || true; }"#,
        reason: "the previous release lookup on a first release",
    },
];

/// Go: `TestNoVacuousFlags`. Scans the files that define gates: the Makefile and every make
/// fragment, the pre-commit configuration and every YAML file under .github, the workflows
/// included.
#[test]
fn no_vacuous_flags() {
    let repo = meta::repo();
    let fragments = repo.files_under(".mk", &[".mk"]);
    assert!(
        !fragments.is_empty(),
        "no make fragments found under .mk/; the scan would check nothing"
    );
    let mut files = repo.read_all(&["Makefile".to_owned(), PRE_COMMIT.to_owned()]);
    files.extend(repo.read_all(&fragments));
    files.extend(github_yaml());
    let problems = vacuous_flags().problems(&files, ALLOWED);
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// Go: `TestVacuousFlagsAreRecognized`, the negative control: each flag, written as a gate would
/// carry it, is found, and an allowed fragment excuses only itself, in its own file.
#[test]
fn vacuous_flags_are_recognized() {
    let flags = vacuous_flags();
    let hits = |file: &str, line: &str| {
        let mut used = vec![false; ALLOWED.len()];
        flags.hits(file, line, ALLOWED, &mut used).len()
    };
    for sample in [
        "\tcargo clippy -- -D warnings || true",
        "\tcargo clippy -- -D warnings ||:",
        "\tcargo clippy -- -D warnings || exit 0",
        "\t-cargo test",
        "\t@-cargo test",
        ".IGNORE: rust-clippy",
        "\t$(MAKE) -i rust-test",
        "\t$(MAKE) --ignore-errors rust-test",
        "        continue-on-error: true",
        "    if: false",
        "    if: ${{ false }}",
        "    if: 'false'",
        "    if: \"${{ false }}\"",
        "\tcargo clippy --workspace",
        "\tcargo test --no-run",
        "\t$(CARGO) test -- --skip broker",
    ] {
        assert_eq!(hits("Makefile", sample), 1, "{sample:?}");
    }

    let release_filter = r#"{ grep -E "$RELEASE_TAG_REGEX" || true; } | sort -V | tail -n1"#;
    assert_eq!(
        hits(RELEASE_WORKFLOW, release_filter),
        0,
        "the release tag filter is not allowed in release.yaml"
    );
    assert_eq!(
        hits(CI_WORKFLOW, release_filter),
        1,
        "the release tag filter is allowed outside release.yaml"
    );
    assert_eq!(
        hits(RELEASE_WORKFLOW, &format!("{release_filter} || true")),
        1,
        "a second || true beside the allowed one is allowed in release.yaml"
    );
    assert_eq!(
        hits(RELEASE_WORKFLOW, "make rust-test || true"),
        1,
        "an unrelated || true is allowed in release.yaml"
    );
}
