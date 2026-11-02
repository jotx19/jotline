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
        ParseError::MissingType(vec!["foo".to_string()])
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

fn shape(name: &str, required: &[&str], optional: &[&str]) -> super::action::ToolShape {
    super::action::ToolShape {
        name: name.to_string(),
        required: required.iter().map(|a| a.to_string()).collect(),
        optional: optional.iter().map(|a| a.to_string()).collect(),
    }
}

fn names(list: &[&str]) -> Vec<super::action::ToolShape> {
    list.iter().map(|name| shape(name, &[], &[])).collect()
}

fn builtin_shapes() -> Vec<super::action::ToolShape> {
    crate::tools::ToolExecutor::new(crate::tools::ToolConfig::for_workspace("."))
        .list_specs()
        .iter()
        .map(super::action::ToolShape::from)
        .collect()
}

#[test]
fn bare_arguments_matching_one_tool_become_a_tool_call() {
    use super::action::parse_action_with_tools;
    let tools = builtin_shapes();
    for (reply, expected) in [
        (r#"{"command":"pytest -q"}"#, "run_shell"),
        (r#"{"path":"a.py","content":"x = 1"}"#, "write_file"),
        (
            r#"{"path":"a.py","old":"a","new":"b","reason":"fix"}"#,
            "edit_file",
        ),
        (r#"{"pattern":"def add","glob":"*.py"}"#, "search_files"),
        (r#"{"rule":"Always use pytest"}"#, "rules_add"),
    ] {
        let (action, canonical) =
            parse_action_with_tools(reply, &tools).unwrap_or_else(|err| panic!("{reply}: {err}"));
        assert!(
            matches!(action, Action::Tool(ref t) if t.tool == expected),
            "{reply}"
        );
        assert_eq!(canonical["type"], "tool");
        assert!(canonical["args"].get("reason").is_none(), "{reply}");
    }
}

#[test]
fn ambiguous_or_unknown_bare_arguments_are_rejected_with_their_fields() {
    use super::action::parse_action_with_tools;
    let tools = builtin_shapes();
    // read_file, python_symbol_overview, ... all accept just "path".
    let err = parse_action_with_tools(r#"{"path":"calc.py"}"#, &tools).unwrap_err();
    assert_eq!(err, ParseError::MissingType(vec!["path".to_string()]));
    assert!(err.to_string().contains("(it has: path)"));
    // "pattern" plus an unknown field fits no tool.
    let err = parse_action_with_tools(r#"{"pattern":"x","colour":"red"}"#, &tools).unwrap_err();
    assert!(matches!(err, ParseError::MissingType(_)));
    let err = parse_action_with_tools(r#"{}"#, &tools).unwrap_err();
    assert!(err.to_string().contains("no fields"));
}

#[test]
fn tool_name_in_type_field_becomes_a_tool_call() {
    use super::action::parse_action_with_tools;
    let tools = names(&["run_shell", "read_file", "plugin.echo"]);

    // Arguments nested under "args".
    let (action, canonical) = parse_action_with_tools(
        r#"{"type":"run_shell","args":{"command":"ls"},"reason":"look"}"#,
        &tools,
    )
    .unwrap();
    assert_eq!(canonical["type"], "tool");
    match action {
        Action::Tool(tool) => {
            assert_eq!(tool.tool, "run_shell");
            assert_eq!(tool.args["command"], "ls");
            assert_eq!(tool.reason, "look");
        }
        other => panic!("unexpected {other:?}"),
    }

    // Arguments at the top level, and a plugin name containing a dot.
    let (action, _) =
        parse_action_with_tools(r#"{"type":"read_file","path":"a.rs","end_line":5}"#, &tools)
            .unwrap();
    assert!(
        matches!(action, Action::Tool(ref t) if t.tool == "read_file" && t.args["path"] == "a.rs" && t.args["end_line"] == 5)
    );
    let (action, _) = parse_action_with_tools(
        r#"{"type":"plugin.echo","arguments":{"value":"x"}}"#,
        &tools,
    )
    .unwrap();
    assert!(
        matches!(action, Action::Tool(ref t) if t.tool == "plugin.echo" && t.args["value"] == "x")
    );
}

#[test]
fn unknown_names_and_real_action_types_are_unchanged() {
    use super::action::parse_action_with_tools;
    let tools = names(&["read_file"]);
    assert_eq!(
        parse_action_with_tools(r#"{"type":"teleport"}"#, &tools).unwrap_err(),
        ParseError::UnknownType("teleport".to_string())
    );
    let (action, _) =
        parse_action_with_tools(r#"{"type":"final","message":"ok"}"#, &tools).unwrap();
    assert!(matches!(action, Action::Final(_)));
    // Without a tool list, tool names in "type" are still rejected.
    assert!(matches!(
        parse_action(r#"{"type":"read_file","path":"a"}"#).unwrap_err(),
        ParseError::UnknownType(_)
    ));
}
