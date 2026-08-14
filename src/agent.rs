// Concern: how a named agent is invoked — its argv, its system prompt, its resume, and where its answer and identity are read back | Non-concern: what it is asked | IO: (system, prompt) -> answer

use crate::git;
use std::io::Write;
use std::process::{Command, Stdio};

// What a run gives back. The model and the session are read from the agent rather than asked of it: one it would have to guess at, the other it cannot know.
pub struct Answer {
    pub text: String,
    pub reviewer: String,
    pub session: String,
}

// What an agent is being asked for, not which model answers it: which model is cheap enough for a one-line question is knowledge about that agent, and it lives with the code that drives it.
#[derive(Clone, Copy)]
pub enum Role {
    Review,
    JudgeIntent,
}

// Named, not spelled out: resuming, system prompts and machine-readable output differ enough between agents that a repo cannot express them in one command line, so the difference lives here. One name so far.
pub struct Agent;

impl Agent {
    pub fn named(name: &str) -> Result<Agent, String> {
        if name.trim() != "claude" {
            return Err(format!(
                "unknown agent '{}': this build knows claude.\n  git config --global agent-verdict.runner claude",
                name.trim()
            ));
        }
        Ok(Agent)
    }

    pub fn run(
        &self,
        role: Role,
        system: &str,
        prompt: &str,
        resume: Option<&str>,
    ) -> Result<Answer, String> {
        claude(role, system, prompt, resume)
    }
}

// Through a file, because argv and a pipe both have a ceiling the standing instructions do not: they carry every rubric inlined.
fn system_file(text: &str) -> Result<std::path::PathBuf, String> {
    let path = git::git_path("AGENT_VERDICT_SYSTEM")?;
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

// Judging one line of text is not worth the model a review is worth, and which model is small enough to do it is claude's own business.
fn claude_model(role: Role) -> Option<&'static str> {
    match role {
        Role::Review => None,
        Role::JudgeIntent => Some("haiku"),
    }
}

fn claude(role: Role, system: &str, prompt: &str, resume: Option<&str>) -> Result<Answer, String> {
    let file = system_file(system)?;
    let mut command = Command::new("claude");
    command.args(["-p", "--output-format", "json"]);
    command.arg("--append-system-prompt-file").arg(&file);
    if let Some(session) = resume {
        command.args(["--resume", session]);
    }
    if let Some(model) = claude_model(role) {
        command.args(["--model", model]);
    }
    let out = piped(command, prompt)?;
    let _ = std::fs::remove_file(&file);
    read_claude(&out)
}

// Written from its own thread, because both pipes are bounded: an agent that talks while the prompt goes in fills stdout and waits for a read this side cannot reach until its own write finishes.
fn piped(mut command: Command, prompt: &str) -> Result<String, String> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run the reviewer: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("the reviewer took no stdin")?;
    let text = prompt.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(text.as_bytes()));
    let out = child
        .wait_with_output()
        .map_err(|e| format!("the reviewer did not finish: {e}"))?;
    // An agent that answers without reading closes the pipe first; that is its business, and the answer it prints is still the answer.
    match writer.join() {
        Err(_) => return Err("the prompt was never written to the reviewer".to_string()),
        Ok(Err(e)) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            return Err(format!("cannot brief the reviewer: {e}"));
        }
        Ok(_) => {}
    }
    // Its own stderr is inherited, so whatever it said is already above this line.
    if !out.status.success() {
        return Err(format!("the reviewer exited {}", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

fn read_claude(out: &str) -> Result<Answer, String> {
    let json: serde_json::Value =
        serde_json::from_str(out).map_err(|e| format!("the reviewer's answer is not JSON: {e}"))?;
    if json["is_error"].as_bool().unwrap_or(false) {
        let said = json["result"].as_str().unwrap_or("no reason given");
        return Err(format!("the reviewer reported an error: {said}"));
    }
    let text = json["result"]
        .as_str()
        .ok_or("the reviewer's answer carries no result")?;
    let session = json["session_id"]
        .as_str()
        .ok_or("the reviewer's answer carries no session_id")?;
    // Which model actually answered, rather than which one was asked for: a fallback would otherwise reach the trailer under the name of the model that never ran.
    let reviewer = json["modelUsage"]
        .as_object()
        .and_then(|used| used.keys().next().cloned())
        .unwrap_or_else(|| "claude".to_string());
    Ok(Answer {
        text: text.to_string(),
        reviewer,
        session: session.to_string(),
    })
}
