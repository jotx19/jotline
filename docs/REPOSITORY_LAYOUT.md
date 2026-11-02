# Repository layout rules

Choose a location by responsibility. Extend an existing crate when the code belongs to its current responsibility; create a new crate only when it needs its own public interface, dependencies, or packaging boundary. Keep the repository root for workspace configuration, shared tooling, and repository metadata.

## Project placement

| Responsibility | Location | Reason |
|---|---|---|
| Orchestration engine: intent, actions, prompts, rules, tools, model clients | `crates/jotline-core/` | A library with no terminal I/O, usable by other front ends and testable without a terminal. |
| Command-line front end | `crates/jotline/` | Argument parsing, REPL, rendering, and approvals. Depends on `jotline-core`. |
| Front ends in other languages that talk to `jotline --stdio` | `packages/<name>/` | Each has its own package manifest, lockfile, tests, and README. The TypeScript terminal UI is `packages/tui/`. They depend only on the protocol in [PROTOCOL.md](PROTOCOL.md), never on Rust internals. |
| Built-in rule files compiled into the engine | `crates/jotline-core/rules/` | Embedded with `include_str!`, so they ship inside the binary and are packaged with the crate. |
| User and contributor guides | `docs/` | One guide per topic, linked from the README. |
| Sample project rules and plugins for users to copy | `examples/` | Not built or loaded by jotline itself. |
| Maintainer scripts used by CI | `tools/` | Standard-library Python only, so CI needs no package installation. |
| Agent instructions | `AGENTS.md`, `CLAUDE.md`, `.claude/`, `.codex/` | Shared guidance in `AGENTS.md`; topic rules in `.claude/rules/`; reviewer agents in `.claude/agents/` with Codex equivalents in `.codex/agents/`. |
| Automation | `.github/` | Workflows, dependency updates, and issue and pull request templates. |

## Crate structure

Keep each crate's production code in `src/`, integration tests in `tests/`, and shared integration-test helpers in `tests/support/mod.rs`. Put unit tests in a sibling `tests.rs` file declared with `#[cfg(test)] mod tests;` next to the module it tests; small, self-contained helpers may keep an inline `mod tests` block. A module that grows past one responsibility becomes a directory with a `mod.rs` and focused submodules, as `tools/` and `orchestrator/` are.

## Cargo ownership

The root [Cargo.toml](../Cargo.toml) is a virtual workspace. It owns membership, `[workspace.package]` metadata (version, edition, MSRV, license), shared dependency versions in `[workspace.dependencies]`, lint configuration, and build profiles. Member manifests inherit with `version.workspace = true` and `dep.workspace = true`. All members share the root `Cargo.lock` and `target/`.

Release tooling updates the version in the root manifest, the `jotline-core` dependency requirement in `crates/jotline/Cargo.toml`, and `Cargo.lock` together; see [releasing](RELEASING.md).

## Generated output

Build output and temporary files go under ignored `target/`. jotline's own runtime state (`.jotline/`) is ignored in this repository. Never commit session files, memory files, or machine-specific paths.

## Adding or moving a crate or directory

Update Cargo membership and path dependencies together with the files. Then reconcile CI workflow paths, `release-please-config.json` extra files, documentation links, and this guide. Run the full check list in [AGENTS.md](../AGENTS.md#testing).
