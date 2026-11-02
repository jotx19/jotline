//! Core engine for the `jotline` coding agent.
//!
//! The engine drives a local or remote chat model through a typed JSON action protocol
//! (`plan -> tool/parallel_tools/delegate -> final`). Each request is classified into an
//! [`intent::Intent`] that selects a playbook, project rules from Markdown files are injected
//! into the system prompt, and every tool runs inside the workspace boundary.
//!
//! The terminal front end lives in the `jotline` crate; this crate has no terminal output of
//! its own and reports progress through [`reporter::Reporter`].

pub mod config;
pub mod intent;
pub mod llm;
pub mod orchestrator;
pub mod reporter;
pub mod rules;
pub mod tools;

/// Directory, relative to the workspace root, that holds jotline state: memory, sessions,
/// plugins, and project rule files.
pub const STATE_DIR: &str = ".jotline";
