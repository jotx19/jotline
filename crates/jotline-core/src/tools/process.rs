use super::{clip, fail, ToolResult};
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;
use wait_timeout::ChildExt;

const MAX_STREAM_CHARS: usize = 12_000;
/// How long to wait for the output readers after the process exits or is killed. A background
/// grandchild (`server &`) can hold the pipes open forever; jotline then returns without that
/// stream instead of hanging.
const DRAIN_GRACE: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub(crate) struct ProcessResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
}

/// Runs `command` with piped output, feeding `stdin_payload` if given, and kills it (and, on
/// Unix, its whole process group) after `timeout_seconds`.
///
/// stdout and stderr are drained on separate threads while the process runs, so commands that
/// print more than the OS pipe buffer do not block.
pub(crate) fn run_process(
    command: &mut Command,
    stdin_payload: Option<&str>,
    timeout_seconds: u64,
) -> io::Result<ProcessResult> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(if stdin_payload.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        });
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }

    let mut child = command.spawn()?;

    if let (Some(payload), Some(mut stdin)) = (stdin_payload, child.stdin.take()) {
        let payload = payload.to_string();
        thread::spawn(move || {
            let _ = stdin.write_all(payload.as_bytes());
        });
    }

    let stdout_rx = drain(child.stdout.take());
    let stderr_rx = drain(child.stderr.take());

    let timeout = Duration::from_secs(timeout_seconds.max(1));
    let (exit_code, timed_out) = match child.wait_timeout(timeout)? {
        Some(status) => (status.code().unwrap_or(-1), false),
        None => {
            kill_tree(&mut child);
            let _ = child.wait();
            (-1, true)
        }
    };

    Ok(ProcessResult {
        exit_code,
        stdout: stdout_rx.recv_timeout(DRAIN_GRACE).unwrap_or_default(),
        stderr: stderr_rx.recv_timeout(DRAIN_GRACE).unwrap_or_default(),
        timed_out,
    })
}

fn drain<R: Read + Send + 'static>(stream: Option<R>) -> mpsc::Receiver<String> {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        if let Some(mut stream) = stream {
            let _ = stream.read_to_end(&mut bytes);
        }
        let _ = tx.send(String::from_utf8_lossy(&bytes).into_owned());
    });
    rx
}

fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        // The child leads its own process group (process_group(0) above); signal the group.
        let _ = Command::new("kill")
            .arg("-9")
            .arg(format!("-{}", child.id()))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

pub(crate) fn format_output(result: &ProcessResult) -> String {
    let stdout = clip(result.stdout.trim(), MAX_STREAM_CHARS, "stdout truncated");
    let stderr = clip(result.stderr.trim(), MAX_STREAM_CHARS, "stderr truncated");
    let mut out = format!(
        "EXIT_CODE: {}\nSTDOUT:\n{}\nSTDERR:\n{}",
        result.exit_code,
        if stdout.is_empty() {
            "<empty>"
        } else {
            &stdout
        },
        if stderr.is_empty() {
            "<empty>"
        } else {
            &stderr
        },
    );
    if result.timed_out {
        out.push_str("\nNOTE: process timed out and was killed");
    }
    out
}

pub(crate) fn run_shell(root: &Path, command: &str, timeout_seconds: u64) -> ToolResult {
    if command.trim().is_empty() {
        return fail("command cannot be empty");
    }
    let mut cmd = Command::new("bash");
    cmd.arg("-lc").arg(command).current_dir(root);
    match run_process(&mut cmd, None, timeout_seconds) {
        Ok(result) => Ok(format_output(&result)),
        Err(err) => fail(format!("failed to start bash: {err}")),
    }
}
