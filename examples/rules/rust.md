# Rust rules

Copy to `.jotline/rules/rust.md` in a Rust project.

- Verify with `cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test`.
- Library crates return `thiserror` errors; binaries use `anyhow` with `.context(...)`.
- No `unwrap()` or `expect()` outside tests.
- Put unit tests in a `#[cfg(test)] mod tests` block in the same file.
