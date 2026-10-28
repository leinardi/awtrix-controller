# Conventions (beyond AGENTS.md and user-level Rust skills)

Project rules: `AGENTS.md`; general guidance: user-level Rust skills. Lint traps that
repeatedly bite:

- `min_ident_chars`: no single-letter names, closures included (only `x`, `y` allowed).
- `wildcard_enum_match_arm`: name every variant (`Some(_) | None => ...`), no `_ =>` on an enum.
- `unwrap_used`/`expect_used` denied outside `#[test]`/`#[cfg(test)]`: helpers in `tests/*.rs` are
  not covered (`tests/run.rs` carries a crate-level `#![expect]`); poisoned mutex:
  `.lock().unwrap_or_else(PoisonError::into_inner)`.
- `print_stdout`/`print_stderr` denied: log via `tracing`, `writeln!` for `--version`.
- `disallowed_methods`: no `Timestamp::now`/`Zoned::now`/`SystemTime::now`/`Instant::now` outside
  `src/clock` and the weather simulation; use the injected `Clock`.
- pedantic: `#[must_use]` on pure fns, `# Errors` docs, identifiers in backticks in docs.
- Public items before private ones within an `impl` (review rule, not linted).

Suppressions: only `#[expect(lint, reason = "...")]`, ratcheted per lint against
`.suppressions-baseline`; a commit adding or moving one must run
`scripts/check-suppressions.sh --update` and commit the baseline. The reason says why the fix does
not apply, not which lint fired.

Tests: paused tokio clock (`start_paused = true`) + `TokioClock`; bound every wait with
`tokio::time::timeout`; capture logs with `test_support::Captured::install`; ported tests carry
``/// Go: `TestX` ``.

Commits: Conventional Commits with mandatory scope; type chosen by whether it should ship
(`fix`/`feat` release, others don't); avoid words ending in `fix`/`feat` before a colon in other
subjects (svu matches them).
