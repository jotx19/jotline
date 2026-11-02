use super::action::{extract_json_object, parse_action, strip_think_blocks, Action, ParseError};
use super::session::context_window;
use crate::llm::{ChatMessage, ChatRole};

#[test]
fn parses_plain_fenced_and_embedded_objects() {
    for text in [
        r#"{"type":"final","message":"ok"}"#,
        "```json\n{\"type\":\"final\",\"message\":\"ok\"}\n```",
        "Sure! Here it is:\n```\n{\"type\":\"final\",\"message\":\"ok\"}\n```\nDone.",
        r#"I will answer now. {"type":"final","message":"ok"} trailing words"#,
    ] {
        let (action, _) = parse_action(text).unwrap_or_else(|err| panic!("{text}: {err}"));
        assert!(
            matches!(action, Action::Final(ref f) if f.message == "ok"),
            "{text}"
        );
    }
}

#[test]
fn ignores_think_blocks_with_braces() {
    let text = "<think>maybe {\"type\":\"tool\"} or {not json}</think>\n{\"type\":\"final\",\"message\":\"real\"}";
    let (action, _) = parse_action(text).unwrap();
    assert!(matches!(action, Action::Final(ref f) if f.message == "real"));
    assert_eq!(strip_think_blocks("<think>unterminated"), "");
    assert_eq!(strip_think_blocks("a<think>x</think>b"), "ab");
}

#[test]
fn prefers_an_object_with_a_type_field() {
    let text =
        r#"context {"path":"a.rs"} then {"type":"tool","tool":"read_file","args":{"path":"a.rs"}}"#;
    let value = extract_json_object(text).unwrap();
    assert_eq!(value["type"], "tool");
}

#[test]
fn accepts_aliases_and_coerces_scalars() {
    let (action, canonical) =
        parse_action(r#"{"type":"tool_call","name":"read_file","arguments":{"path":"x"}}"#)
            .unwrap();
    assert_eq!(canonical["type"], "tool");
    match action {
        Action::Tool(tool) => {
            assert_eq!(tool.tool, "read_file");
            assert_eq!(tool.args["path"], "x");
        }
        other => panic!("unexpected {other:?}"),
    }

    let (action, _) = parse_action(
        r#"{"type":"plan","needs_plan":"true","summary":7,"steps":"only step","intent":"Bug-Fix"}"#,
    )
    .unwrap();
    match action {
        Action::Plan(plan) => {
            assert_eq!(plan.needs_plan, Some(true));
            assert_eq!(plan.summary, "7");
            assert_eq!(plan.steps, ["only step"]);
            assert_eq!(plan.intent.as_deref(), Some("Bug-Fix"));
        }
        other => panic!("unexpected {other:?}"),
    }

    let (action, _) =
        parse_action(r#"{"type":"delegate","task":"do it","max_steps":"3"}"#).unwrap();
    assert!(
        matches!(action, Action::Delegate(ref d) if d.max_steps == Some(3) && d.prompt == "do it")
    );
}

#[test]
fn infers_missing_type_from_fields() {
    let (action, canonical) = parse_action(r#"{"tool":"list_files","args":{}}"#).unwrap();
    assert_eq!(action.type_name(), "tool");
    assert_eq!(canonical["type"], "tool");
    assert_eq!(
        parse_action(r#"{"foo":1}"#).unwrap_err(),
        ParseError::MissingType
    );
}

#[test]
fn reports_specific_errors() {
    assert_eq!(
        parse_action("no json here").unwrap_err(),
        ParseError::NoJson
    );
    assert_eq!(
        parse_action(r#"{"type":"dance"}"#).unwrap_err(),
        ParseError::UnknownType("dance".to_string())
    );
    let invalid = parse_action(r#"{"type":"tool","args":{}}"#).unwrap_err();
    assert!(
        matches!(invalid, ParseError::Invalid { ref action_type, .. } if action_type == "tool")
    );
    let empty_calls = parse_action(r#"{"type":"parallel_tools","calls":[]}"#).unwrap_err();
    assert!(empty_calls.to_string().contains("at least one"));
    let bad_args = parse_action(r#"{"type":"tool","tool":"x","args":[1]}"#).unwrap_err();
    assert!(bad_args.to_string().contains("JSON object"));
}

#[test]
fn string_lists_accept_objects_and_drop_empty_entries() {
    let (action, _) =
        parse_action(r#"{"type":"final","message":"m","completed":[{"step":"a"}," ",null,"b",3]}"#)
            .unwrap();
    match action {
        Action::Final(final_action) => assert_eq!(final_action.completed, ["a", "b", "3"]),
        other => panic!("unexpected {other:?}"),
    }
}

fn conversation(roles: &[ChatRole]) -> Vec<ChatMessage> {
    let mut messages = vec![ChatMessage::system("sys")];
    for (index, role) in roles.iter().enumerate() {
        messages.push(ChatMessage {
            role: *role,
            content: index.to_string(),
        });
    }
    messages
}

#[test]
fn context_window_keeps_system_and_starts_on_user() {
    use ChatRole::{Assistant, User};
    let messages = conversation(&[User, Assistant, User, Assistant, User, Assistant]);
    let window = context_window(&messages, 3, None);
    // Last 3 would start on an Assistant message ("3"); it is skipped.
    let contents = window
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>();
    assert_eq!(contents, ["sys", "4", "5"]);
    assert_eq!(context_window(&messages, 100, None).len(), messages.len());
}

#[test]
fn context_window_keeps_pinned_request() {
    use ChatRole::{Assistant, User};
    let messages = conversation(&[User, Assistant, User, Assistant, User, Assistant]);
    // messages[1] is the request ("0").
    let window = context_window(&messages, 2, Some(1));
    let contents = window
        .iter()
        .map(|m| m.content.as_str())
        .collect::<Vec<_>>();
    assert_eq!(contents, ["sys", "0", "4", "5"]);
    // A pinned message already inside the window is not duplicated.
    let window = context_window(&messages, 100, Some(1));
    assert_eq!(window.len(), messages.len());
}
