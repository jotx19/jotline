//! Markdown rule files that shape how the agent writes code.
//!
//! A [`RuleSet`] combines two layers, rendered into every agent's system prompt in this order:
//!
//! 1. **Built-in rules** compiled into the binary from `crates/jotline-core/rules/*.md`.
//! 2. **Project rules** read from the workspace: `JOTLINE.md`, `AGENTS.md`, and every
//!    `*.md` file in `.jotline/rules/` (sorted by file name).
//!
//! Project rules come last and the prompt tells the model they win on conflict. Users add a
//! rule from the REPL with `/rules add <text>`, and the model can persist one with the
//! `rules_add` tool when the user says "from now on ..."; both append to
//! `.jotline/rules/user.md` through [`add_user_rule`].

#[cfg(test)]
mod tests;

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Maximum characters read from one project rule file; the rest is dropped with a note.
pub const MAX_FILE_CHARS: usize = 12_000;
/// Maximum characters of project rules in total, so rules cannot crowd out the task on small
/// context windows.
pub const MAX_PROJECT_CHARS: usize = 24_000;
/// Maximum length of one rule added through [`add_user_rule`].
pub const MAX_RULE_CHARS: usize = 500;
/// File that [`add_user_rule`] appends to, inside the rules directory.
pub const USER_RULES_FILE: &str = "user.md";
/// Workspace-root files read as project rules, in this order.
pub const ROOT_RULE_FILES: [&str; 2] = ["JOTLINE.md", "AGENTS.md"];

const BUILTIN: [(&str, &str); 4] = [
    (
        "coding-principles",
        include_str!("../../rules/coding-principles.md"),
    ),
    (
        "code-generation",
        include_str!("../../rules/code-generation.md"),
    ),
    ("testing", include_str!("../../rules/testing.md")),
    ("security", include_str!("../../rules/security.md")),
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuleOrigin {
    Builtin,
    Project,
}

/// One rule file as loaded.
#[derive(Clone, Debug)]
pub struct RuleSource {
    pub origin: RuleOrigin,
    /// Built-in rule name, or the workspace-relative path of a project file.
    pub name: String,
    pub content: String,
    /// True when the file exceeded [`MAX_FILE_CHARS`] or the project budget and was cut.
    pub truncated: bool,
}

/// Which rule layers to load.
#[derive(Clone, Copy, Debug)]
pub struct RuleOptions {
    pub builtin: bool,
    pub project: bool,
}

impl Default for RuleOptions {
    fn default() -> Self {
        Self {
            builtin: true,
            project: true,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct RuleSet {
    sources: Vec<RuleSource>,
}

impl RuleSet {
    /// Loads rules for `workspace`. Unreadable or empty files are skipped; loading never fails.
    pub fn load(workspace: &Path, rules_dir: &Path, options: RuleOptions) -> Self {
        let mut sources = Vec::new();

        if options.builtin {
            for (name, content) in BUILTIN {
                sources.push(RuleSource {
                    origin: RuleOrigin::Builtin,
                    name: name.to_string(),
                    content: content.trim().to_string(),
                    truncated: false,
                });
            }
        }

        if options.project {
            let mut budget = MAX_PROJECT_CHARS;
            for path in project_rule_paths(workspace, rules_dir) {
                if budget == 0 {
                    break;
                }
                let Ok(text) = fs::read_to_string(&path) else {
                    continue;
                };
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }
                let limit = MAX_FILE_CHARS.min(budget);
                let (content, truncated) = truncate_chars(text, limit);
                budget = budget.saturating_sub(content.chars().count());
                sources.push(RuleSource {
                    origin: RuleOrigin::Project,
                    name: display_path(workspace, &path),
                    content,
                    truncated,
                });
            }
        }

        Self { sources }
    }

    /// A rule set with only the built-in rules.
    pub fn builtin_only() -> Self {
        Self::load(
            Path::new("."),
            Path::new("."),
            RuleOptions {
                builtin: true,
                project: false,
            },
        )
    }

    pub fn sources(&self) -> &[RuleSource] {
        &self.sources
    }

    pub fn is_empty(&self) -> bool {
        self.sources.is_empty()
    }

    /// The rules section of the system prompt, or an empty string when no rules are loaded.
    pub fn render_for_prompt(&self) -> String {
        if self.sources.is_empty() {
            return String::new();
        }

        let mut out = String::from(
            "Follow every rule below. Project rules come after built-in rules and take precedence when they conflict.\n",
        );
        for source in &self.sources {
            let label = match source.origin {
                RuleOrigin::Builtin => format!("built-in: {}", source.name),
                RuleOrigin::Project => format!("project: {}", source.name),
            };
            out.push_str(&format!("\n### {label}\n{}\n", source.content));
            if source.truncated {
                out.push_str("(truncated: read the file for the rest)\n");
            }
        }
        out
    }
}

/// Project rule files that exist, in load order.
pub fn project_rule_paths(workspace: &Path, rules_dir: &Path) -> Vec<PathBuf> {
    let mut paths = ROOT_RULE_FILES
        .iter()
        .map(|name| workspace.join(name))
        .filter(|path| path.is_file())
        .collect::<Vec<_>>();

    if let Ok(entries) = fs::read_dir(rules_dir) {
        let mut files = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "md"))
            .collect::<Vec<_>>();
        files.sort();
        paths.extend(files);
    }

    paths
}

/// Appends `rule` as a Markdown bullet to `<rules_dir>/user.md`, creating the directory and file
/// when needed. Newlines in `rule` are collapsed to spaces. Fails when the rule is empty or
/// longer than [`MAX_RULE_CHARS`]. Returns the file path.
pub fn add_user_rule(rules_dir: &Path, rule: &str) -> io::Result<PathBuf> {
    let normalized = rule.split_whitespace().collect::<Vec<_>>().join(" ");
    let normalized = normalized.trim_start_matches(['-', '*', ' ']).trim();
    if normalized.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "rule cannot be empty",
        ));
    }
    if normalized.chars().count() > MAX_RULE_CHARS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("rule is longer than {MAX_RULE_CHARS} characters; put long rules in a file under .jotline/rules/"),
        ));
    }

    fs::create_dir_all(rules_dir)?;
    let path = rules_dir.join(USER_RULES_FILE);
    let is_new = !path.exists();
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?;
    if is_new {
        writeln!(
            file,
            "# User rules\n\nRules added with `/rules add` or by the agent at your request. Edit freely.\n"
        )?;
    }
    writeln!(file, "- {normalized}")?;
    Ok(path)
}

fn truncate_chars(text: &str, limit: usize) -> (String, bool) {
    if text.chars().count() <= limit {
        return (text.to_string(), false);
    }
    (text.chars().take(limit).collect(), true)
}

fn display_path(workspace: &Path, path: &Path) -> String {
    path.strip_prefix(workspace)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
