use super::*;

#[test]
fn classifies_common_requests() {
    let cases = [
        ("fix the panic in the parser", Intent::BugFix),
        (
            "the login form is broken when the email is empty",
            Intent::BugFix,
        ),
        ("add a --json flag to the export command", Intent::Feature),
        ("implement a LRU cache module", Intent::Feature),
        ("refactor tools.rs into smaller modules", Intent::Refactor),
        ("write unit tests for the tokenizer", Intent::Test),
        ("review my staged changes", Intent::Review),
        ("update the README with install steps", Intent::Docs),
        ("explain how the session file is loaded", Intent::Question),
        ("where is the retry logic?", Intent::Question),
        ("run cargo clippy and commit", Intent::Ops),
        ("hello there", Intent::General),
    ];

    for (prompt, expected) in cases {
        assert_eq!(classify(prompt), expected, "prompt: {prompt}");
    }
}

#[test]
fn weak_action_words_in_a_question_stay_a_question() {
    assert_eq!(classify("how do I build this?"), Intent::Question);
}

#[test]
fn strong_signal_overrides_question_mark() {
    assert_eq!(classify("can you fix the failing test?"), Intent::BugFix);
}

#[test]
fn matches_whole_words_only() {
    // "address" contains "add" and "prefix" contains "fix"; neither may count.
    let scored = scores("print the address prefix");
    let feature = scored
        .iter()
        .find(|(i, _)| *i == Intent::Feature)
        .unwrap()
        .1;
    let bugfix = scored.iter().find(|(i, _)| *i == Intent::BugFix).unwrap().1;
    assert_eq!(feature, 0);
    assert_eq!(bugfix, 0);
}

#[test]
fn parse_round_trips_and_accepts_aliases() {
    for intent in Intent::ALL {
        assert_eq!(Intent::parse(intent.as_str()), Some(intent));
    }
    assert_eq!(Intent::parse("Bug-Fix"), Some(Intent::BugFix));
    assert_eq!(Intent::parse("shell"), Some(Intent::Ops));
    assert_eq!(Intent::parse("nonsense"), None);
}

#[test]
fn read_only_intents_skip_verification() {
    for intent in Intent::ALL {
        if intent.is_read_only() {
            assert!(!intent.requires_verification(), "{intent}");
        }
        assert!(!intent.playbook().is_empty() || intent == Intent::General);
    }
}
