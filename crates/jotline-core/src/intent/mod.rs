//! Request intent classification and per-intent playbooks.
//!
//! Small local models follow a short, explicit procedure far better than an open-ended
//! instruction. Every top-level request is classified into an [`Intent`] with a fast,
//! deterministic keyword scorer; the intent selects a [`Intent::playbook`] that is attached to
//! the request, and decides whether the orchestrator's verification gate applies. The model may
//! override the classification by setting `intent` in its plan.

#[cfg(test)]
mod tests;

use std::fmt;

/// The kind of work a request asks for.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Intent {
    /// Explain or locate something; no file changes expected.
    Question,
    /// Add new behavior or generate new code.
    Feature,
    /// Diagnose and fix incorrect behavior.
    BugFix,
    /// Restructure code without changing behavior.
    Refactor,
    /// Add or repair tests.
    Test,
    /// Review code or changes and report findings; no file changes expected.
    Review,
    /// Write or update documentation.
    Docs,
    /// Run commands: build, install, git, formatting, and similar operations.
    Ops,
    /// Anything that matched no specific intent.
    General,
}

impl Intent {
    /// Every intent, in tie-break priority order (earlier wins on equal scores).
    pub const ALL: [Intent; 9] = [
        Intent::BugFix,
        Intent::Test,
        Intent::Refactor,
        Intent::Review,
        Intent::Docs,
        Intent::Feature,
        Intent::Ops,
        Intent::Question,
        Intent::General,
    ];

    /// The stable lowercase name used in prompts, plans, and session files.
    pub fn as_str(self) -> &'static str {
        match self {
            Intent::Question => "question",
            Intent::Feature => "feature",
            Intent::BugFix => "bugfix",
            Intent::Refactor => "refactor",
            Intent::Test => "test",
            Intent::Review => "review",
            Intent::Docs => "docs",
            Intent::Ops => "ops",
            Intent::General => "general",
        }
    }

    /// Parses a name produced by [`Intent::as_str`]; also accepts common aliases such as
    /// `fix`, `bug`, `explain`, and `shell`. Returns `None` for anything else.
    pub fn parse(value: &str) -> Option<Intent> {
        let normalized = value
            .trim()
            .to_ascii_lowercase()
            .replace(['-', '_', ' '], "");
        let intent = match normalized.as_str() {
            "question" | "explain" | "ask" | "qa" => Intent::Question,
            "feature" | "codegen" | "generate" | "implement" | "build" => Intent::Feature,
            "bugfix" | "fix" | "bug" | "debug" => Intent::BugFix,
            "refactor" | "cleanup" => Intent::Refactor,
            "test" | "tests" | "testing" => Intent::Test,
            "review" | "audit" => Intent::Review,
            "docs" | "doc" | "documentation" => Intent::Docs,
            "ops" | "shell" | "command" | "run" => Intent::Ops,
            "general" | "other" => Intent::General,
            _ => return None,
        };
        Some(intent)
    }

    /// Whether the task is expected to leave files unchanged.
    pub fn is_read_only(self) -> bool {
        matches!(self, Intent::Question | Intent::Review)
    }

    /// Whether the orchestrator requires a verification command after file changes before it
    /// accepts the final handoff.
    pub fn requires_verification(self) -> bool {
        matches!(
            self,
            Intent::Feature | Intent::BugFix | Intent::Refactor | Intent::Test | Intent::General
        )
    }

    /// The step-by-step procedure attached to requests of this intent.
    pub fn playbook(self) -> &'static [&'static str] {
        match self {
            Intent::Question => &[
                "Locate the relevant code with search_files/find_files before reading whole files.",
                "Read only the sections you need and cite paths with line numbers.",
                "Answer directly; do not modify files.",
            ],
            Intent::Feature => &[
                "Find where similar behavior already lives and follow its conventions, naming, and error handling.",
                "Design the smallest change that fully delivers the feature; list every file you will touch in the plan.",
                "Write complete code: no placeholders, TODOs, or elided sections. Use edit_file for existing files.",
                "Add or update tests that exercise the new behavior.",
                "Run the project's build and tests with run_shell; fix failures before finishing.",
            ],
            Intent::BugFix => &[
                "Reproduce or pinpoint the failure first: read the error, search for the failing code path.",
                "State the root cause in the plan before editing.",
                "Make the minimal fix at the root cause, not a workaround at the symptom.",
                "Add a regression test that fails without the fix.",
                "Run the tests with run_shell and confirm the failure is gone.",
            ],
            Intent::Refactor => &[
                "Record the current behavior and run the existing tests before editing.",
                "Change structure only; keep public behavior and outputs identical.",
                "Update every caller of moved or renamed items (search_files for each name).",
                "Run the full test suite with run_shell and confirm it still passes.",
            ],
            Intent::Test => &[
                "Read the code under test and its existing tests; follow the existing test style and location.",
                "Cover the normal case, boundaries, and error paths with precise assertions.",
                "Run the new tests with run_shell; a test that was never executed does not count.",
            ],
            Intent::Review => &[
                "Inspect the changes (git_diff, git_status) or the named files.",
                "Report concrete findings with file:line, the failing input, and the expected behavior.",
                "Rank findings by severity; do not modify files unless asked.",
            ],
            Intent::Docs => &[
                "Read the code the documentation describes and check every claim against it.",
                "Write for a reader new to the project: purpose, setup, a runnable example, limitations.",
                "Keep commands and paths copy-pasteable and repository-relative.",
            ],
            Intent::Ops => &[
                "Inspect the project files that define the command (Cargo.toml, package.json, Makefile, ...).",
                "Run the command with run_shell and report the exit code and relevant output.",
                "If it fails, diagnose from the output before retrying.",
            ],
            Intent::General => &[
                "Clarify the goal from the workspace before acting; inspect relevant files first.",
                "Prefer minimal, safe changes and verify any change you make.",
            ],
        }
    }
}

impl fmt::Display for Intent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Keyword weights per intent. Single words are matched as whole words; entries containing a
/// space are matched as phrases.
fn signals(intent: Intent) -> &'static [(&'static str, u32)] {
    match intent {
        Intent::BugFix => &[
            ("fix", 3),
            ("bug", 3),
            ("broken", 3),
            ("crash", 3),
            ("crashes", 3),
            ("panic", 3),
            ("panics", 3),
            ("error", 2),
            ("errors", 2),
            ("failing", 3),
            ("fails", 3),
            ("exception", 3),
            ("regression", 3),
            ("wrong", 2),
            ("incorrect", 2),
            ("debug", 2),
            ("doesn't work", 3),
            ("does not work", 3),
            ("not working", 3),
        ],
        Intent::Test => &[
            ("test", 3),
            ("tests", 3),
            ("unit test", 2),
            ("coverage", 3),
            ("assert", 2),
            ("spec", 1),
        ],
        Intent::Refactor => &[
            ("refactor", 4),
            ("rename", 3),
            ("restructure", 3),
            ("reorganize", 3),
            ("simplify", 3),
            ("extract", 2),
            ("clean up", 3),
            ("cleanup", 3),
            ("deduplicate", 3),
            ("split", 1),
            ("move", 1),
        ],
        Intent::Review => &[
            ("review", 4),
            ("audit", 4),
            ("critique", 3),
            ("code review", 2),
            ("look over", 2),
            ("any issues", 2),
        ],
        Intent::Docs => &[
            ("readme", 4),
            ("docs", 3),
            ("documentation", 4),
            ("document", 2),
            ("docstring", 3),
            ("docstrings", 3),
            ("comment", 1),
            ("comments", 1),
            ("changelog", 3),
            ("guide", 1),
        ],
        Intent::Feature => &[
            ("add", 2),
            ("implement", 3),
            ("create", 2),
            ("build", 1),
            ("generate", 2),
            ("write", 1),
            ("make", 1),
            ("support", 1),
            ("feature", 3),
            ("endpoint", 1),
            ("scaffold", 3),
            ("new", 1),
            ("function", 1),
            ("component", 1),
        ],
        Intent::Ops => &[
            ("run", 2),
            ("install", 3),
            ("commit", 3),
            ("push", 2),
            ("deploy", 3),
            ("lint", 2),
            ("format", 2),
            ("compile", 2),
            ("start", 1),
            ("git", 2),
            ("cargo", 1),
            ("npm", 1),
        ],
        Intent::Question => &[
            ("what", 2),
            ("why", 2),
            ("how", 2),
            ("where", 2),
            ("which", 1),
            ("explain", 4),
            ("describe", 3),
            ("summarize", 3),
            ("understand", 2),
            ("show me", 2),
            ("tell me", 2),
            ("does", 1),
        ],
        Intent::General => &[],
    }
}

/// Scores every intent for `prompt`; higher is a stronger match.
pub fn scores(prompt: &str) -> Vec<(Intent, u32)> {
    let lowered = prompt.to_lowercase();
    let words = lowered
        .split(|ch: char| !(ch.is_alphanumeric() || ch == '\'' || ch == '_'))
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();

    Intent::ALL
        .iter()
        .map(|&intent| {
            let score = signals(intent)
                .iter()
                .map(|&(signal, weight)| {
                    let hit = if signal.contains(' ') {
                        lowered.contains(signal)
                    } else {
                        words.contains(&signal)
                    };
                    if hit {
                        weight
                    } else {
                        0
                    }
                })
                .sum::<u32>();
            (intent, score)
        })
        .collect()
}

/// Classifies a request. Ties are broken by [`Intent::ALL`] order; a request with no signal is
/// a [`Intent::Question`] when it ends with `?` and [`Intent::General`] otherwise.
pub fn classify(prompt: &str) -> Intent {
    let mut best = (Intent::General, 0u32);
    for (intent, score) in scores(prompt) {
        if score > best.1 {
            best = (intent, score);
        }
    }

    // A trailing question mark with only weak action words ("how do I build this?") is a
    // question, not a request to act.
    if prompt.trim_end().ends_with('?') && best.0 != Intent::Question && best.1 <= 2 {
        return Intent::Question;
    }

    best.0
}
