mod support;

use jotline_core::intent::Intent;
use jotline_core::llm::ChatRole;
use support::{build_orchestrator, StubLlm};
use tempfile::TempDir;

const PLAN: &str = r#"{"type":"plan","needs_plan":false,"summary":"do it","steps":["act"]}"#;

#[test]
fn parses_embedded_json_action_and_records_plan() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([
        r#"I will do this now. {"type":"plan","needs_plan":false,"summary":"finish fast","reason":"simple request","steps":["answer directly"]}"#,
        r#"{"type":"final","message":"done","completed":["answered"],"verified":["response produced"],"missed":[]}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm.clone());
    assert_eq!(orchestrator.run_active("explain this"), "done");

    let summary = orchestrator.last_run_summary().unwrap();
    assert_eq!(summary.agent_name, "main");
    assert_eq!(summary.intent, Some(Intent::Question));
    assert_eq!(summary.plan.as_ref().unwrap().summary, "finish fast");
    assert_eq!(summary.steps_used, 2);
    assert!(summary.handoff.is_some());
}

#[test]
fn request_carries_intent_playbook() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([PLAN, r#"{"type":"final","message":"ok"}"#]);
    let mut orchestrator = build_orchestrator(&tmp, llm.clone());
    orchestrator.run_active("fix the crash in parser.rs");

    let first_call = &llm.calls()[0];
    let request = first_call
        .iter()
        .find(|m| m.role == ChatRole::User)
        .unwrap();
    assert!(request.content.starts_with("fix the crash in parser.rs"));
    assert!(request.content.contains("intent: bugfix"));
    assert!(request.content.contains("regression test"));
}

#[test]
fn actions_before_plan_are_rejected() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello\n").unwrap();
    let llm = StubLlm::new([
        r#"{"type":"tool","tool":"read_file","args":{"path":"a.txt"}}"#,
        PLAN,
        r#"{"type":"final","message":"ok"}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm.clone());
    assert_eq!(orchestrator.run_active("read a.txt"), "ok");
    // The premature tool call never ran.
    assert!(orchestrator
        .agent_snapshot("main")
        .unwrap()
        .tool_history
        .is_empty());
    let correction = llm.calls()[1].last().unwrap().content.clone();
    assert!(correction.contains("Start with a plan"));
}

#[test]
fn invalid_json_gets_a_specific_correction() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([
        "just prose",
        r#"{"type":"dance"}"#,
        PLAN,
        r#"{"type":"final","message":"ok"}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm.clone());
    assert_eq!(orchestrator.run_active("hello"), "ok");
    let calls = llm.calls();
    assert!(calls[1].last().unwrap().content.contains("no JSON object"));
    assert!(calls[2]
        .last()
        .unwrap()
        .content
        .contains("Unknown action type \"dance\""));
}

#[test]
fn executes_parallel_action_and_records_history() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("a.txt"), "hello\n").unwrap();
    let llm = StubLlm::new([
        r#"{"type":"plan","needs_plan":true,"summary":"inspect files","steps":["read","list"]}"#,
        "```json\n{\"type\":\"parallel_tools\",\"calls\":[{\"tool\":\"read_file\",\"args\":{\"path\":\"a.txt\"}},{\"tool\":\"list_files\",\"args\":{\"path\":\".\",\"max_depth\":1}}]}\n```",
        r#"{"type":"final","message":"done"}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm.clone());
    assert_eq!(orchestrator.run_active("inspect files in parallel"), "done");

    let snapshot = orchestrator.agent_snapshot("main").unwrap();
    assert_eq!(snapshot.tool_history.len(), 1);
    assert_eq!(snapshot.tool_history[0].tool, "parallel_tools");
    assert!(snapshot.tool_history[0].ok);
    assert!(snapshot.tool_history[0].result.contains("hello"));
}

#[test]
fn verification_gate_requires_a_command_after_edits() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([
        PLAN,
        r#"{"type":"tool","tool":"write_file","args":{"path":"lib.py","content":"x = 1\n"}}"#,
        r#"{"type":"final","message":"premature"}"#,
        r#"{"type":"tool","tool":"run_shell","args":{"command":"python3 -c 'import lib'"}}"#,
        r#"{"type":"final","message":"verified done","verified":["import ok"]}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm.clone());
    assert_eq!(
        orchestrator.run_active("add a constant x to lib.py"),
        "verified done"
    );

    let calls = llm.calls();
    assert!(calls[3]
        .last()
        .unwrap()
        .content
        .contains("have not run any verification"));
    let summary = orchestrator.last_run_summary().unwrap();
    assert!(summary.files_changed);
    assert!(summary.verified);
}

#[test]
fn verification_gate_reminds_only_once() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([
        PLAN,
        r#"{"type":"tool","tool":"write_file","args":{"path":"a.txt","content":"x"}}"#,
        r#"{"type":"final","message":"first"}"#,
        r#"{"type":"final","message":"cannot verify","missed":["no test runner"]}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm);
    assert_eq!(
        orchestrator.run_active("implement the feature"),
        "cannot verify"
    );
    let summary = orchestrator.last_run_summary().unwrap();
    assert!(summary.files_changed);
    assert!(!summary.verified);
}

#[test]
fn read_only_intent_skips_verification_gate() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([
        r#"{"type":"plan","intent":"docs","summary":"write notes","steps":["write"]}"#,
        r##"{"type":"tool","tool":"write_file","args":{"path":"NOTES.md","content":"# notes"}}"##,
        r#"{"type":"final","message":"notes written"}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm);
    // Classified as feature ("add"), but the plan overrides the intent to docs.
    assert_eq!(orchestrator.run_active("add notes"), "notes written");
    assert_eq!(
        orchestrator.last_run_summary().unwrap().intent,
        Some(Intent::Docs)
    );
}

#[test]
fn rules_add_persists_and_updates_system_prompt() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([
        PLAN,
        r#"{"type":"tool","tool":"rules_add","args":{"rule":"Always use pytest for Python tests"}}"#,
        r#"{"type":"final","message":"saved"}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm);
    orchestrator.run_active("from now on always use pytest");

    let user_rules = tmp.path().join(".jotline/rules/user.md");
    assert!(std::fs::read_to_string(user_rules)
        .unwrap()
        .contains("Always use pytest"));
    let system = &orchestrator.agent_snapshot("main").unwrap().messages[0];
    assert_eq!(system.role, ChatRole::System);
    assert!(system.content.contains("Always use pytest"));
}

#[test]
fn project_rule_files_reach_the_system_prompt() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("JOTLINE.md"),
        "Use 4-space indentation in YAML.",
    )
    .unwrap();
    let llm = StubLlm::new([PLAN, r#"{"type":"final","message":"ok"}"#]);
    let mut orchestrator = build_orchestrator(&tmp, llm.clone());
    orchestrator.run_active("hello");
    let system = &llm.calls()[0][0];
    assert!(system.content.contains("project: JOTLINE.md"));
    assert!(system.content.contains("Use 4-space indentation in YAML."));
    assert!(system.content.contains("built-in: code-generation"));
}

#[test]
fn step_limit_stops_the_loop() {
    let tmp = TempDir::new().unwrap();
    let looping = r#"{"type":"tool","tool":"list_files","args":{}}"#;
    let llm = StubLlm::new([PLAN, looping, looping, looping, looping, looping, looping]);
    let mut orchestrator = build_orchestrator(&tmp, llm.clone());
    let result = orchestrator.run_active("loop forever");
    assert!(result.contains("step limit"));
    assert_eq!(llm.calls().len(), 6);
    // The last tool result warns that steps are running out.
    assert!(llm.calls()[5]
        .last()
        .unwrap()
        .content
        .contains("step(s) left"));
}

#[test]
fn delegate_named_agent_and_session_round_trip() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([
        r#"{"type":"plan","needs_plan":true,"summary":"use delegation","steps":["delegate","combine"]}"#,
        r#"{"type":"delegate","agent_name":"research","prompt":"do subtask","max_steps":2}"#,
        r#"{"type":"plan","needs_plan":false,"summary":"handle subtask","steps":["complete"]}"#,
        r#"{"type":"final","message":"subtask done"}"#,
        r#"{"type":"final","message":"all done"}"#,
    ]);
    let mut orchestrator = build_orchestrator(&tmp, llm);
    assert_eq!(
        orchestrator.run_active("solve this with delegation"),
        "all done"
    );
    assert!(orchestrator.has_agent("research"));

    let parent = orchestrator.agent_snapshot("main").unwrap();
    assert_eq!(parent.delegate_history.len(), 1);
    assert_eq!(parent.delegate_history[0].agent_name, "research");
    assert_eq!(parent.delegate_history[0].result, "subtask done");

    let session_file = tmp.path().join("session.json");
    orchestrator.save_session(&session_file).unwrap();
    let saved: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&session_file).unwrap()).unwrap();
    assert_eq!(saved["version"], 4);
    // System prompts are rebuilt on load, never stored.
    assert!(!saved.to_string().contains("\"role\":\"system\""));

    let mut restored = build_orchestrator(&tmp, StubLlm::new([]));
    restored.load_session(&session_file).unwrap();
    assert!(restored.has_agent("research"));
    assert_eq!(restored.active_agent(), "main");
    let restored_main = restored.agent_snapshot("main").unwrap();
    assert_eq!(restored_main.delegate_history.len(), 1);
    assert_eq!(restored_main.messages[0].role, ChatRole::System);
    assert_eq!(restored_main.messages.len(), parent.messages.len());
}

#[test]
fn delegation_depth_limit_is_enforced() {
    let tmp = TempDir::new().unwrap();
    let llm = StubLlm::new([
        PLAN,
        r#"{"type":"delegate","prompt":"nested"}"#,
        r#"{"type":"final","message":"done alone"}"#,
    ]);
    let mut orchestrator = support::build_orchestrator_with(&tmp, llm.clone(), |config| {
        config.max_delegation_depth = 0;
    });
    assert_eq!(orchestrator.run_active("do it"), "done alone");
    assert!(llm.calls()[2]
        .last()
        .unwrap()
        .content
        .contains("delegation depth limit"));
}

#[test]
fn loads_version_3_session_files() {
    let tmp = TempDir::new().unwrap();
    let legacy = serde_json::json!({
        "version": 3,
        "active_agent": "helper",
        "delegate_counter": 5,
        "agents": {
            "main": {"messages": [{"role":"system","content":"old"},{"role":"user","content":"hi"}], "tool_history": [{"tool":"read_file","args":{},"result":"x"}], "delegate_history": []},
            "helper": {"messages": [], "tool_history": [], "delegate_history": []},
            "bad name!": {"messages": []}
        }
    });
    let path = tmp.path().join("old.json");
    std::fs::write(&path, legacy.to_string()).unwrap();

    let mut orchestrator = build_orchestrator(&tmp, StubLlm::new([]));
    orchestrator.load_session(&path).unwrap();
    assert_eq!(orchestrator.list_agents(), ["helper", "main"]);
    assert_eq!(orchestrator.active_agent(), "helper");
    let main = orchestrator.agent_snapshot("main").unwrap();
    assert_eq!(main.messages.len(), 2);
    assert_ne!(main.messages[0].content, "old");
    assert_eq!(main.tool_history[0].tool, "read_file");
}

#[test]
fn llm_errors_end_the_task_with_a_message() {
    let tmp = TempDir::new().unwrap();
    let mut orchestrator = build_orchestrator(&tmp, StubLlm::failing("connection refused"));
    let result = orchestrator.run_active("hello");
    assert!(result.contains("Model call failed"));
    assert!(result.contains("connection refused"));
}
