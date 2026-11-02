use super::*;
use tempfile::TempDir;

fn workspace() -> (TempDir, PathBuf) {
    let tmp = TempDir::new().expect("temp dir");
    let rules_dir = tmp.path().join(".jotline").join("rules");
    (tmp, rules_dir)
}

#[test]
fn builtin_rules_are_embedded_and_non_empty() {
    let set = RuleSet::builtin_only();
    assert_eq!(set.sources().len(), BUILTIN.len());
    for source in set.sources() {
        assert_eq!(source.origin, RuleOrigin::Builtin);
        assert!(
            source.content.starts_with('#'),
            "{} has no heading",
            source.name
        );
    }
}

#[test]
fn project_rules_load_in_documented_order_after_builtin() {
    let (tmp, rules_dir) = workspace();
    fs::create_dir_all(&rules_dir).unwrap();
    fs::write(tmp.path().join("AGENTS.md"), "# agents\nuse tabs").unwrap();
    fs::write(tmp.path().join("JOTLINE.md"), "# jotline\nuse spaces").unwrap();
    fs::write(rules_dir.join("b.md"), "rule b").unwrap();
    fs::write(rules_dir.join("a.md"), "rule a").unwrap();
    fs::write(rules_dir.join("notes.txt"), "ignored").unwrap();
    fs::write(rules_dir.join("empty.md"), "   \n").unwrap();

    let set = RuleSet::load(tmp.path(), &rules_dir, RuleOptions::default());
    let project = set
        .sources()
        .iter()
        .filter(|s| s.origin == RuleOrigin::Project)
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        project,
        [
            "JOTLINE.md",
            "AGENTS.md",
            ".jotline/rules/a.md",
            ".jotline/rules/b.md"
        ]
    );

    let rendered = set.render_for_prompt();
    let builtin_at = rendered.find("built-in: coding-principles").unwrap();
    let project_at = rendered.find("project: JOTLINE.md").unwrap();
    assert!(builtin_at < project_at);
}

#[test]
fn options_disable_layers() {
    let (tmp, rules_dir) = workspace();
    fs::write(tmp.path().join("JOTLINE.md"), "rule").unwrap();
    let none = RuleSet::load(
        tmp.path(),
        &rules_dir,
        RuleOptions {
            builtin: false,
            project: false,
        },
    );
    assert!(none.is_empty());
    assert_eq!(none.render_for_prompt(), "");

    let project_only = RuleSet::load(
        tmp.path(),
        &rules_dir,
        RuleOptions {
            builtin: false,
            project: true,
        },
    );
    assert_eq!(project_only.sources().len(), 1);
}

#[test]
fn oversized_project_files_are_truncated_and_marked() {
    let (tmp, rules_dir) = workspace();
    fs::write(
        tmp.path().join("JOTLINE.md"),
        "x".repeat(MAX_FILE_CHARS + 50),
    )
    .unwrap();
    let set = RuleSet::load(
        tmp.path(),
        &rules_dir,
        RuleOptions {
            builtin: false,
            project: true,
        },
    );
    let source = &set.sources()[0];
    assert!(source.truncated);
    assert_eq!(source.content.chars().count(), MAX_FILE_CHARS);
    assert!(set.render_for_prompt().contains("(truncated"));
}

#[test]
fn total_project_budget_is_enforced() {
    let (tmp, rules_dir) = workspace();
    fs::create_dir_all(&rules_dir).unwrap();
    for name in ["a.md", "b.md", "c.md"] {
        fs::write(rules_dir.join(name), "y".repeat(MAX_FILE_CHARS)).unwrap();
    }
    let set = RuleSet::load(
        tmp.path(),
        &rules_dir,
        RuleOptions {
            builtin: false,
            project: true,
        },
    );
    let total = set
        .sources()
        .iter()
        .map(|s| s.content.chars().count())
        .sum::<usize>();
    assert_eq!(total, MAX_PROJECT_CHARS);
    assert_eq!(set.sources().len(), 2);
}

#[test]
fn add_user_rule_creates_file_with_header_then_appends() {
    let (tmp, rules_dir) = workspace();
    let path = add_user_rule(&rules_dir, "Use anyhow for errors\nin binaries").unwrap();
    add_user_rule(&rules_dir, "- Prefer iterators").unwrap();

    let text = fs::read_to_string(&path).unwrap();
    assert_eq!(text.matches("# User rules").count(), 1);
    assert!(text.contains("- Use anyhow for errors in binaries\n"));
    assert!(text.contains("- Prefer iterators\n"));

    let set = RuleSet::load(tmp.path(), &rules_dir, RuleOptions::default());
    assert!(set.render_for_prompt().contains("Prefer iterators"));
}

#[test]
fn add_user_rule_rejects_empty_and_oversized() {
    let (_tmp, rules_dir) = workspace();
    assert!(add_user_rule(&rules_dir, "  \n ").is_err());
    assert!(add_user_rule(&rules_dir, &"z".repeat(MAX_RULE_CHARS + 1)).is_err());
    assert!(!rules_dir.join(USER_RULES_FILE).exists());
}
