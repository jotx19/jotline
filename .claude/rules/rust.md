# Rust rules

- Target the MSRV in the root `Cargo.toml` (`rust-version`). Do not use newer standard library APIs or language features; CI builds with stable, so check the stabilization version of anything new.
- `unsafe` is forbidden workspace-wide (`unsafe_code = "forbid"`). Do not add it.
- Format with `cargo fmt`. Code must pass `cargo clippy --workspace --all-targets -- -D warnings`; fix lints instead of adding `#[allow]`, and justify any unavoidable `allow` in a comment.
- Errors: library code in `jotline-core` returns typed errors or `anyhow::Result` with `.context(...)` naming the operation and the path or value involved. The binary may use `anyhow` freely. Never `unwrap()` or `expect()` outside tests except on invariants that are proven locally, with a comment.
- Tools never panic on model input. Validate arguments through `tools::args::Args` and return `fail("...")` with a message that tells the model how to fix the call.
- Represent known shapes with structs and enums (`Intent`, `Action`, `ToolStatus`, `Effect`). Use `serde_json::Value` only at the JSON boundary and for genuinely free-form data such as tool arguments.
- Truncate text with `tools::clip` or char-based logic; never slice a `String` at a byte index that might not be a character boundary.
- Child processes go through `tools::process::run_process`, which drains output concurrently and kills the process group on timeout. Do not call `Command::output()` for model-controlled commands.
- Keep the engine free of terminal I/O. Report progress through `Reporter` and ask for approval through `Approver`.
- Prefer the standard library and existing dependencies. A new dependency needs a reason in the PR (what it replaces, its size, its maintenance status). Avoid dependencies that need system libraries at build time.
- Public items in `jotline-core` get doc comments that tell a caller what the name and type do not: valid values, defaults, units, error conditions, and relationships to other items. Delete comments that restate the signature.
