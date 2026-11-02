# Testing rules

- Every behavior change ships with a test that would fail without it. For a bug fix, write the failing test first and confirm it fails.
- Unit tests go in a sibling `tests.rs` module declared with `#[cfg(test)] mod tests;`. Integration tests go in the crate's `tests/` directory; shared helpers in `tests/support/mod.rs`.
- Tests never call a real model or the network. Script model replies with `StubLlm` (core) or the fake Ollama server in `crates/jotline/tests/cli.rs` (binary), and assert on the requests the model received when the prompt matters.
- Use `tempfile::TempDir` for every file-system test, and initialize git repositories with `support::init_repo`. Never touch the developer's real home directory or the repository checkout.
- Cover the normal case, boundaries (empty, one, many, limits), malformed input from the model (wrong types, missing fields, prose around JSON), and the error message the model receives.
- Security-relevant behavior (workspace escapes, symlinks, unsafe patch paths, denied approvals) needs explicit negative tests.
- Tests must be deterministic and fast: no sleeps longer than needed to exercise a timeout, no dependence on test order or wall-clock time, and Unix-only behavior gated with `#[cfg(unix)]`.
- Do not weaken, skip, or delete a failing test to make a change pass. Report it instead.
- Run `cargo test --workspace --locked` before handing off and report the exact result.
