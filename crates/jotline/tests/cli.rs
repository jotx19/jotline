//! End-to-end tests of the `jotline` binary against a scripted fake Ollama server.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use tempfile::TempDir;

const BIN: &str = env!("CARGO_BIN_EXE_jotline");

/// Serves `/api/chat` replies in order and records request bodies. Each reply is
/// `(status, assistant_content_or_body)`.
fn fake_ollama(replies: Vec<(u16, String)>) -> (String, Arc<Mutex<Vec<String>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let endpoint = format!("http://{}/api/chat", listener.local_addr().unwrap());
    let bodies = Arc::new(Mutex::new(Vec::new()));
    let recorded = Arc::clone(&bodies);
    thread::spawn(move || {
        let mut replies = replies.into_iter();
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut content_length = 0usize;
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    content_length = value.trim().parse().unwrap_or(0);
                }
            }
            let mut body = vec![0u8; content_length];
            let _ = reader.read_exact(&mut body);
            recorded
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&body).into_owned());

            let (status, payload) = match replies.next() {
                Some((200, content)) => (
                    200,
                    serde_json::json!({"message": {"role": "assistant", "content": content}})
                        .to_string(),
                ),
                Some((status, body)) => (status, body),
                None => (500, "{\"error\":\"script exhausted\"}".to_string()),
            };
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                payload.len()
            );
            let _ = stream.write_all(response.as_bytes());
        }
    });
    (endpoint, bodies)
}

#[test]
fn help_and_version() {
    let help = Command::new(BIN).arg("--help").output().unwrap();
    assert!(help.status.success());
    let text = String::from_utf8_lossy(&help.stdout);
    assert!(text.contains("--no-project-rules"));
    assert!(text.contains("JOTLINE_MODEL"));

    let version = Command::new(BIN).arg("--version").output().unwrap();
    assert!(String::from_utf8_lossy(&version.stdout).starts_with("jotline "));
}

#[test]
fn one_shot_prompt_edits_verifies_and_hands_off() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("JOTLINE.md"),
        "Project rule: keep greetings short.",
    )
    .unwrap();
    let (endpoint, bodies) = fake_ollama(vec![
        (200, r#"{"type":"plan","needs_plan":false,"summary":"create greet.txt","steps":["write","check"]}"#.into()),
        (200, r#"{"type":"tool","tool":"write_file","args":{"path":"greet.txt","content":"hi\n"}}"#.into()),
        (200, r#"{"type":"tool","tool":"run_shell","args":{"command":"cat greet.txt"}}"#.into()),
        (200, r#"{"type":"final","message":"Created greet.txt","completed":["wrote greet.txt"],"verified":["cat greet.txt -> hi"]}"#.into()),
    ]);

    let output = Command::new(BIN)
        .args([
            "--endpoint",
            &endpoint,
            "--model",
            "test-model",
            "--prompt",
            "create a greet.txt file",
        ])
        .arg("--workspace")
        .arg(tmp.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        output.status.success(),
        "stdout:\n{stdout}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(stdout.contains("Created greet.txt"), "{stdout}");
    assert!(stdout.contains("cat greet.txt -> hi"));
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("greet.txt")).unwrap(),
        "hi\n"
    );

    let bodies = bodies.lock().unwrap();
    assert_eq!(bodies.len(), 4);
    let first: serde_json::Value = serde_json::from_str(&bodies[0]).unwrap();
    assert_eq!(first["model"], "test-model");
    let system = first["messages"][0]["content"].as_str().unwrap();
    assert!(system.contains("keep greetings short"));
}

#[test]
fn missing_model_explains_how_to_install_it() {
    let tmp = TempDir::new().unwrap();
    let (endpoint, _) = fake_ollama(vec![(404, r#"{"error":"model 'nope' not found"}"#.into())]);
    let output = Command::new(BIN)
        .args([
            "--endpoint",
            &endpoint,
            "--model",
            "nope",
            "--prompt",
            "hi",
            "--quiet",
        ])
        .arg("--workspace")
        .arg(tmp.path())
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("ollama pull nope"), "{stdout}");
}

#[test]
fn invalid_workspace_fails_cleanly() {
    let output = Command::new(BIN)
        .args(["--workspace", "/definitely/not/a/dir", "--prompt", "hi"])
        .env("NO_COLOR", "1")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("workspace not found"));
}
