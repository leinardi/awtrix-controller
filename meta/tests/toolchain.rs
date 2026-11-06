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

//! The repository's tooling pins: one Rust release across rust-toolchain.toml, Cargo.toml's line,
//! the image's line and every CI toolchain step; the image's toolchain patch and its musl standard
//! libraries; one zig and one cargo-zigbuild between the image and CI; one droast and one
//! shellcheck between the hooks and CI; the rustdoc gate; and no flag that makes a gate pass on
//! nothing. The checks are `repo_meta::toolchain`'s, `repo_meta::ci`'s and `repo_meta::vacuous`'s,
//! with this repository's parameters; the negative controls run them on altered copies of the real
//! files and on samples.

use repo_meta::ci::MAKE_COMMON_RUSTDOC;
use repo_meta::toolchain::{
    PinSource, RustVersion, ToolchainRef, ci_toolchain_problems, image_rust_problems, one_value,
    rust_release, rust_version_problems, shellcheck_problems,
};
use repo_meta::vacuous::{AllowedFragment, VacuousFlags};
use repo_meta::yaml::{self, Value};
use repo_meta::{Problem, report, strings};

const DOCKERFILE: &str = "deployments/docker/Dockerfile";
const RUST_TOOLCHAIN: &str = "rust-toolchain.toml";
const SETUP_ZIGBUILD: &str = ".github/actions/setup-zigbuild/action.yaml";
const SETUP_DROAST: &str = ".github/actions/setup-droast/action.yaml";
const SETUP_SHELLCHECK: &str = ".github/actions/setup-shellcheck/action.yaml";
const PRE_COMMIT: &str = ".pre-commit-config.yaml";
const CI_WORKFLOW: &str = ".github/workflows/ci.yaml";
const RELEASE_WORKFLOW: &str = ".github/workflows/release.yaml";

const ZIG_DOCKERFILE: &str = r"(?m)^ARG ZIG_VERSION=(\S+)$";
const ZIG_ACTION: &str = r#"ZIG_VERSION:\s*"?([\d.]+)"#;
const ZIGBUILD_DOCKERFILE: &str = r"(?m)^ARG CARGO_ZIGBUILD_VERSION=(\S+)$";
const ZIGBUILD_ACTION: &str = r#"CARGO_ZIGBUILD_VERSION:\s*"?([\d.]+)"#;
const DROAST_HOOK: &str =
    r"repo: https://github\.com/immanuwell/dockerfile-roast\s+rev:\s*([\d.]+)";
const DROAST_ACTION: &str = r#"DROAST_VERSION:\s*["']?([\d.]+)"#;

/// The exact release rust-toolchain.toml pins.
fn release() -> String {
    rust_release(RUST_TOOLCHAIN, &meta::repo().read(RUST_TOOLCHAIN))
        .unwrap_or_else(|problem| panic!("{problem}"))
}

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

/// `files` parsed.
fn documents(files: &[(String, String)]) -> Vec<(String, Value)> {
    files
        .iter()
        .map(|(file, content)| {
            let document = yaml::parse(file, content).unwrap_or_else(|problem| panic!("{problem}"));
            (file.clone(), document)
        })
        .collect()
}

/// `content` with its first `from` replaced by `to`; `from` must be there, so an edit cannot
/// silently change nothing.
fn edited(content: &str, from: &str, to: &str) -> String {
    assert!(content.contains(from), "{from:?} is gone");
    content.replacen(from, to, 1)
}

/// What is wrong with the rustdoc gate of `fragment` (make-common's rust.mk), `makefile` and the
/// CI `workflow`.
fn rustdoc_problems(fragment: &str, makefile: &str, workflow: &str) -> Vec<Problem> {
    let workflow = yaml::parse(CI_WORKFLOW, workflow).unwrap_or_else(|problem| panic!("{problem}"));
    MAKE_COMMON_RUSTDOC.problems(fragment, makefile, &workflow)
}

/// Rustdoc warnings must fail local verification (`verify-rust` runs `rust-doc`, whose recipe
/// denies warnings over every feature) and the required Rust CI job (a `make rust-doc` step with
/// no condition and no `continue-on-error`): `repo_meta::ci::MAKE_COMMON_RUSTDOC`.
#[test]
fn rust_documentation_is_a_required_gate() {
    let repo = meta::repo();
    let problems = rustdoc_problems(
        &repo.read(".mk/rust.mk"),
        &repo.read("Makefile"),
        &repo.read(CI_WORKFLOW),
    );
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// Negative controls reject gates missing from verify or CI, allowing warnings, or narrowed to
/// fewer features: each edit is exactly one problem.
#[test]
fn weakened_rustdoc_gates_are_refused() {
    let repo = meta::repo();
    let fragment = repo.read(".mk/rust.mk");
    let makefile = repo.read("Makefile");
    let workflow = repo.read(CI_WORKFLOW);
    for (name, fragment, makefile, workflow) in [
        (
            "verify without rust-doc",
            edited(&fragment, "rust-test rust-doc", "rust-test"),
            makefile.clone(),
            workflow.clone(),
        ),
        (
            "warnings allowed",
            edited(
                &fragment,
                "RUSTDOCFLAGS=\"$(RUSTDOCFLAGS) -D warnings\"",
                "RUSTDOCFLAGS=\"$(RUSTDOCFLAGS)\"",
            ),
            makefile.clone(),
            workflow.clone(),
        ),
        (
            "the features narrowed",
            fragment.clone(),
            format!("{makefile}\nRUST_FEATURES := --features x\n"),
            workflow.clone(),
        ),
        (
            "no CI step",
            fragment.clone(),
            makefile.clone(),
            edited(
                &workflow,
                "run: make rust-doc",
                "run: echo missing documentation gate",
            ),
        ),
        (
            "a CI step that never runs",
            fragment.clone(),
            makefile.clone(),
            edited(
                &workflow,
                "run: make rust-doc",
                "if: false\n        run: make rust-doc",
            ),
        ),
        (
            "a CI step that may fail",
            fragment.clone(),
            makefile.clone(),
            edited(
                &workflow,
                "run: make rust-doc",
                "continue-on-error: true\n        run: make rust-doc",
            ),
        ),
    ] {
        let problems = rustdoc_problems(&fragment, &makefile, &workflow);
        assert_eq!(problems.len(), 1, "{name}: {}", report(&problems));
    }
}

/// Go: `TestGoLineIsOneValue`. rust-toolchain.toml pins an exact release, and the MSRV in
/// Cargo.toml is its line (or a release of that line): the MSRV names no patch, and a consumer on
/// an earlier patch release still builds (`RustVersion::Line`).
#[test]
fn rust_line_is_one_value() {
    let problems = rust_version_problems(
        "Cargo.toml",
        &meta::repo().read("Cargo.toml"),
        &release(),
        RustVersion::Line,
    );
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// The build stage's standard libraries are written for the image's exact toolchain: the
/// `dhi.io/rust` tag and `ARG RUST_VERSION` are each written once, as an exact release, and are
/// the same release, of rust-toolchain.toml's line (the image may carry its own patch release:
/// `RustVersion::Line`); every other `dhi.io/rust` reference has that release too.
#[test]
fn image_toolchain_patch_is_the_standard_library_version() {
    let problems = image_rust_problems(
        DOCKERFILE,
        &meta::repo().read(DOCKERFILE),
        &release(),
        RustVersion::Line,
    );
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// Go: `TestActionStepsArePinned`. Every Rust toolchain step under .github names exactly
/// rust-toolchain.toml's release, never a channel like stable or a release without its patch,
/// which the line rules would not see move; and some step sets up Rust, or the rule would check
/// nothing.
#[test]
fn rust_toolchain_steps_are_pinned() {
    let problems =
        ci_toolchain_problems(&documents(&github_yaml()), &release(), ToolchainRef::Input);
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// Go: `TestActionPinRules`, the negative control of the step rules: each unpinned form is a
/// problem, another action's toolchain input included, and no Rust step at all is a problem.
#[test]
fn rust_toolchain_pin_rules() {
    let sample = r"jobs:
  test:
    steps:
      - uses: dtolnay/rust-toolchain@0123456789abcdef0123456789abcdef01234567 # master
        with:
          toolchain: 1.98.1
      - uses: dtolnay/rust-toolchain@0123456789abcdef0123456789abcdef01234567
        with:
          toolchain: stable
      - uses: dtolnay/rust-toolchain@0123456789abcdef0123456789abcdef01234567
        with:
          toolchain: 1.98
      - uses: dtolnay/rust-toolchain@0123456789abcdef0123456789abcdef01234567
      - uses: dtolnay/rust-toolchain@0123456789abcdef0123456789abcdef01234567
        with:
          toolchain: 1.97.0
      - uses: some/other-action@0123456789abcdef0123456789abcdef01234567
        with:
          toolchain: nightly
";
    let documents = documents(&[("sample.yaml".to_owned(), sample.to_owned())]);
    let problems = ci_toolchain_problems(&documents, "1.98.1", ToolchainRef::Input);
    assert_eq!(
        strings(&problems),
        [
            "sample.yaml: dtolnay/rust-toolchain names no toolchain, and does not read \
             rust-toolchain.toml",
            "sample.yaml: installs toolchain \"stable\"; it must be 1.98.1, the release of \
             rust-toolchain.toml",
            "sample.yaml: installs toolchain \"1.98\"; it must be 1.98.1, the release of \
             rust-toolchain.toml",
            "sample.yaml: installs toolchain \"1.97.0\"; it must be 1.98.1, the release of \
             rust-toolchain.toml",
            "sample.yaml: installs toolchain \"nightly\"; it must be 1.98.1, the release of \
             rust-toolchain.toml",
        ]
    );

    let none = self::documents(&[(
        "none.yaml".to_owned(),
        "jobs:\n  test:\n    steps:\n      - run: make rust-test\n".to_owned(),
    )]);
    assert_eq!(
        ci_toolchain_problems(&none, "1.98.1", ToolchainRef::Input).len(),
        1,
        "no Rust step"
    );
}

/// The negative control of the Rust release rules, on the real files: a channel without its
/// patch, an MSRV of another line, and an image whose standard libraries or toolchain are another
/// release's are each refused with exactly one problem.
#[test]
fn rust_release_rules_refuse_each_edit() {
    let repo = meta::repo();
    let release = release();
    let line = release
        .rsplit_once('.')
        .map_or(release.as_str(), |(line, _)| line);

    let toolchain = repo.read(RUST_TOOLCHAIN);
    let channel = format!("channel = \"{release}\"");
    for broken in [
        edited(&toolchain, &channel, &format!("channel = \"{line}\"")),
        edited(&toolchain, &channel, "channel = \"stable\""),
    ] {
        assert!(rust_release(RUST_TOOLCHAIN, &broken).is_err(), "{broken}");
    }

    let manifest = repo.read("Cargo.toml");
    let msrv = format!("rust-version = \"{line}\"");
    for broken in [
        edited(&manifest, &msrv, "rust-version = \"1.97\""),
        edited(&manifest, &msrv, "rust-version = \"1.97.0\""),
    ] {
        let problems = rust_version_problems("Cargo.toml", &broken, &release, RustVersion::Line);
        assert_eq!(problems.len(), 1, "{}", report(&problems));
    }

    let dockerfile = repo.read(DOCKERFILE);
    let argument = format!("ARG RUST_VERSION={release}\n");
    for (name, broken, want) in [
        (
            "standard libraries of another patch",
            edited(
                &dockerfile,
                &argument,
                &format!("ARG RUST_VERSION={line}.99\n"),
            ),
            1,
        ),
        (
            "an image of another line",
            edited(
                &edited(&dockerfile, &argument, "ARG RUST_VERSION=1.97.0\n"),
                &format!("dhi.io/rust:{release}-"),
                "dhi.io/rust:1.97.0-",
            ),
            1,
        ),
        (
            "a second RUST_VERSION",
            edited(&dockerfile, &argument, &format!("{argument}{argument}")),
            1,
        ),
    ] {
        let problems = image_rust_problems(DOCKERFILE, &broken, &release, RustVersion::Line);
        assert_eq!(problems.len(), want, "{name}: {}", report(&problems));
    }
}

/// The pin sources of the zig, cargo-zigbuild and droast rules, with `edit` applied to the
/// content of each.
fn pin_problems(edit: impl Fn(&str, String) -> String) -> Vec<Problem> {
    let repo = meta::repo();
    let source =
        |file: &str, pattern: &str| PinSource::new(file, &edit(file, repo.read(file)), pattern);
    let mut problems = one_value(
        "zig",
        &[
            source(DOCKERFILE, ZIG_DOCKERFILE),
            source(SETUP_ZIGBUILD, ZIG_ACTION),
        ],
        &[],
    );
    problems.extend(one_value(
        "cargo-zigbuild",
        &[
            source(DOCKERFILE, ZIGBUILD_DOCKERFILE),
            source(SETUP_ZIGBUILD, ZIGBUILD_ACTION),
        ],
        &[],
    ));
    problems.extend(one_value(
        "droast",
        &[
            source(PRE_COMMIT, DROAST_HOOK),
            source(SETUP_DROAST, DROAST_ACTION),
        ],
        &[],
    ));
    problems
}

/// The release binaries and the image are built with the same zig and the same cargo-zigbuild,
/// and the meta crate's Dockerfile test runs the droast CI installs, which must be the release the
/// pre-commit hook builds from source, whose rev Dependabot moves.
#[test]
fn zig_cargo_zigbuild_and_droast_are_one_version() {
    let problems = pin_problems(|_, content| content);
    assert!(problems.is_empty(), "{}", report(&problems));
}

/// The negative control of the pin rules: a second value, or a source that no longer writes the
/// pin, is exactly one problem.
#[test]
fn pin_rules_refuse_each_edit() {
    for (name, file, from, to) in [
        ("zig", DOCKERFILE, "ARG ZIG_VERSION=", "ARG ZIG_VERSION=9"),
        (
            "zig in CI",
            SETUP_ZIGBUILD,
            "ZIG_VERSION:",
            "ZIG_VERSION_GONE:",
        ),
        (
            "cargo-zigbuild",
            SETUP_ZIGBUILD,
            "CARGO_ZIGBUILD_VERSION: \"",
            "CARGO_ZIGBUILD_VERSION: \"9",
        ),
        (
            "droast",
            SETUP_DROAST,
            "DROAST_VERSION:",
            "DROAST_VERSION: 9",
        ),
    ] {
        let problems = pin_problems(|path, content| {
            if path == file {
                edited(&content, from, to)
            } else {
                content
            }
        });
        assert_eq!(problems.len(), 1, "{name}: {}", report(&problems));
    }
}

/// droast's required shell-linter bridge runs the shellcheck CI installs, which must be the
/// release the shellcheck-py hook wraps. shellcheck-py adds a fourth component of its own
/// (v0.11.0.1), so only the first three are compared.
#[test]
fn shellcheck_is_one_version() {
    let repo = meta::repo();
    let problems = shellcheck_problems(
        PRE_COMMIT,
        &repo.read(PRE_COMMIT),
        SETUP_SHELLCHECK,
        &repo.read(SETUP_SHELLCHECK),
    );
    assert!(problems.is_empty(), "{}", report(&problems));
    let action = edited(
        &repo.read(SETUP_SHELLCHECK),
        "SHELLCHECK_VERSION:",
        "SHELLCHECK_VERSION: v9",
    );
    assert_eq!(
        shellcheck_problems(
            PRE_COMMIT,
            &repo.read(PRE_COMMIT),
            SETUP_SHELLCHECK,
            &action
        )
        .len(),
        1,
        "another shellcheck in CI"
    );
}

/// The flags no gate may carry: the core set, the Rust set with the rule that clippy denies
/// warnings, and the JavaScript set this repository's scan has always held.
fn vacuous_flags() -> VacuousFlags {
    VacuousFlags::core().with_rust().with_javascript()
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
        r#""test": "vitest run --passWithNoTests""#,
        r#""lint": "eslint src --no-error-on-unmatched-pattern""#,
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
