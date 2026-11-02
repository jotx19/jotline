//! Live progress (spinner and events) and interactive approvals.

use super::term::{self, clear_live_line, paint, Tone};
use super::{print_event, print_plan};
use jotline_core::intent::Intent;
use jotline_core::orchestrator::TaskPlan;
use jotline_core::reporter::{EventKind, Reporter};
use jotline_core::tools::Approver;
use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

const SPINNER_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

struct Spinner {
    label: String,
    started: Instant,
    stop: Option<Arc<AtomicBool>>,
    handle: Option<JoinHandle<()>>,
}

impl Spinner {
    fn start(label: &str, message: &str) -> Self {
        let started = Instant::now();
        if !term::supports_live_output() {
            return Self {
                label: label.to_string(),
                started,
                stop: None,
                handle: None,
            };
        }
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread_label = label.to_string();
        let message = message.split_whitespace().collect::<Vec<_>>().join(" ");
        let handle = thread::spawn(move || {
            let mut frame = 0usize;
            while !thread_stop.load(Ordering::SeqCst) {
                let line = format!(
                    "{} {} {} {}",
                    paint(
                        SPINNER_FRAMES[frame % SPINNER_FRAMES.len()],
                        Tone::Accent,
                        true
                    ),
                    paint(&thread_label, Tone::Accent, true),
                    paint(&term::compact(&message, 70), Tone::Muted, false),
                    paint(&term::format_elapsed(started.elapsed()), Tone::Muted, false),
                );
                print!("\r\x1b[2K{line}");
                let _ = io::stdout().flush();
                frame += 1;
                thread::sleep(Duration::from_millis(90));
            }
        });
        Self {
            label: label.to_string(),
            started,
            stop: Some(stop),
            handle: Some(handle),
        }
    }

    /// Stops the animation; prints a completion line when `announce` is set.
    fn finish(mut self, ok: bool, announce: bool) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::SeqCst);
        }
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
        clear_live_line();
        if !announce {
            return;
        }
        let elapsed = term::format_elapsed(self.started.elapsed());
        if ok {
            print_event(
                &self.label,
                Tone::Muted,
                &format!("model replied · {elapsed}"),
            );
        } else {
            print_event(
                &self.label,
                Tone::Danger,
                &format!("model call failed · {elapsed}"),
            );
        }
    }
}

/// Renders orchestration progress in the terminal. With `verbose` off, only the spinner and
/// final output are shown.
pub struct TerminalReporter {
    verbose: bool,
    spinner: Mutex<Option<Spinner>>,
}

impl TerminalReporter {
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            spinner: Mutex::new(None),
        }
    }
}

impl Reporter for TerminalReporter {
    fn intent(&self, _agent: &str, intent: Intent) {
        if self.verbose {
            print_event("intent", Tone::Accent, intent.as_str());
        }
    }

    fn plan(&self, agent: &str, plan: &TaskPlan) {
        if self.verbose {
            print_plan(agent, plan);
        }
    }

    fn thinking_started(&self, label: &str, message: &str) {
        let spinner = Spinner::start(label, message);
        if let Ok(mut slot) = self.spinner.lock() {
            if let Some(previous) = slot.replace(spinner) {
                previous.finish(true, false);
            }
        }
    }

    fn thinking_finished(&self, ok: bool) {
        let spinner = self.spinner.lock().ok().and_then(|mut slot| slot.take());
        if let Some(spinner) = spinner {
            spinner.finish(ok, self.verbose || !ok);
        }
    }

    fn event(&self, kind: EventKind, agent: &str, message: &str) {
        if !self.verbose {
            return;
        }
        let (label, tone) = match kind {
            EventKind::ToolCall => ("tool", Tone::Info),
            EventKind::ToolResult => ("result", Tone::Muted),
            EventKind::ToolError => ("error", Tone::Warning),
            EventKind::Correction => ("retry", Tone::Warning),
        };
        print_event(label, tone, &format!("{agent} {message}"));
    }
}

/// Asks on stdin before shell commands, plugins, and commits.
pub struct TerminalApprover;

impl Approver for TerminalApprover {
    fn approve(&self, request: &str) -> bool {
        clear_live_line();
        println!("{}", paint("approval needed", Tone::Warning, true));
        for line in request.lines() {
            println!("  {line}");
        }
        print!("{}", paint("allow? [y/N] ", Tone::Warning, true));
        let _ = io::stdout().flush();
        let mut answer = String::new();
        match io::stdin().read_line(&mut answer) {
            Ok(_) => matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes"),
            Err(_) => false,
        }
    }
}
