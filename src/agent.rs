// Concern: how a named agent is invoked — its argv, its session, its ceiling, and where its answer and transcript are read back | Non-concern: what it is asked | IO: (system, prompt) -> answer

use crate::git;
use std::io::{Read, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

// stop_reason is unused until an answer carries no verdict, then says whether the reviewer was cut off or ignored its brief.
pub struct Answer {
    pub text: String,
    pub reviewer: String,
    pub session: String,
    pub stop_reason: String,
}

pub enum Session {
    Fresh(String),
    Resume(String),
}

impl Session {
    pub fn opened() -> Session {
        Session::Fresh(fresh_id())
    }

    pub fn resumed(id: &str) -> Session {
        Session::Resume(id.to_string())
    }

    pub fn id(&self) -> &str {
        let (Session::Fresh(id) | Session::Resume(id)) = self;
        id
    }
}

pub struct Terms<'a> {
    pub model: Option<&'a str>,
    pub ceiling: Duration,
    pub read_only: bool,
}

#[derive(Clone, Copy)]
pub enum Role {
    Review,
    JudgeIntent,
}

pub struct Agent;

impl Agent {
    pub fn named(name: &str) -> Result<Agent, String> {
        if name.trim() != "claude" {
            return Err(format!(
                "unknown agent '{}': this build supports only 'claude'.\n  git config --global agent-verdict.runner claude",
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
        session: &Session,
        terms: &Terms,
    ) -> Result<Answer, String> {
        claude(role, system, prompt, session, terms)
    }
}

fn system_file(text: &str) -> Result<std::path::PathBuf, String> {
    let path = git::git_path("AGENT_VERDICT_SYSTEM")?;
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

fn claude_model(role: Role, asked: Option<&str>) -> Option<&str> {
    match role {
        Role::Review => asked,
        Role::JudgeIntent => Some("haiku"),
    }
}

// Assigned before the spawn, not read back from the answer: a crashed or killed run never produces one, but the id still exists to name its transcript.
pub fn fresh_id() -> String {
    let mut bytes = [0u8; 16];
    // /dev/urandom never reaches EOF, so read_exact rather than a read that waits for one.
    let taken =
        std::fs::File::open("/dev/urandom").and_then(|mut urandom| urandom.read_exact(&mut bytes));
    match taken {
        Ok(()) => {}
        // No /dev/urandom: clock+pid need only avoid colliding with a live session, not be random.
        Err(_) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            bytes[..8].copy_from_slice(&(now as u64).to_le_bytes());
            bytes[8..12].copy_from_slice(&std::process::id().to_le_bytes());
        }
    }
    // Version 4 + variant bits: --session-id requires a real uuid, not merely uuid-shaped bytes.
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    )
}

fn claude(
    role: Role,
    system: &str,
    prompt: &str,
    session: &Session,
    terms: &Terms,
) -> Result<Answer, String> {
    let file = system_file(system)?;
    let mut command = Command::new("claude");
    command.args(["-p", "--output-format", "json"]);
    command.arg("--append-system-prompt-file").arg(&file);
    match session {
        Session::Fresh(id) => command.args(["--session-id", id]),
        Session::Resume(id) => command.args(["--resume", id]),
    };
    if let Some(model) = claude_model(role, terms.model) {
        command.args(["--model", model]);
    }
    // dontAsk denies whatever would have prompted, since a headless run has no one to ask; plan mode additionally keeps a read-only reviewer from writing.
    let mode = if terms.read_only { "plan" } else { "dontAsk" };
    command.args(["--permission-mode", mode]);
    let told = |detail: String| with_transcript(&detail, session.id());
    let said = piped(role, command, prompt, terms.ceiling, session.id()).map_err(told)?;
    let _ = std::fs::remove_file(&file);
    read_claude(&said).map_err(told)
}

// stderr is kept regardless of exit status: an agent can crash on stderr and still exit 0.
struct Said {
    out: String,
    err: String,
}

const HEARTBEAT: Duration = Duration::from_secs(60);

fn heartbeat(ceiling: Duration) -> Duration {
    HEARTBEAT.min(ceiling / 4)
}

const KEPT: usize = 2000;

fn clipped(text: &str) -> String {
    let text = text.trim();
    match text.char_indices().nth(KEPT) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

type Seen = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;

// Both pipes are bounded, so drained on their own threads into a shared buffer readable before EOF — a killed agent's pipe can stay open in whatever it spawned.
fn drain(pipe: Option<impl Read + Send + 'static>) -> (std::sync::mpsc::Receiver<()>, Seen) {
    let seen: Seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let filling = std::sync::Arc::clone(&seen);
    let (done, drained) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        if let Some(mut pipe) = pipe {
            let mut chunk = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut chunk) {
                if n == 0 {
                    break;
                }
                hold(&filling).extend_from_slice(&chunk[..n]);
            }
        }
        let _ = done.send(());
    });
    (drained, seen)
}

fn hold(seen: &Seen) -> std::sync::MutexGuard<'_, Vec<u8>> {
    seen.lock().unwrap_or_else(|held| held.into_inner())
}

const SETTLING: Duration = Duration::from_secs(5);

fn settled(drained: &std::sync::mpsc::Receiver<()>, by: Instant) {
    let _ = drained.recv_timeout(by.saturating_duration_since(Instant::now()));
}

fn text_of(seen: &Seen) -> String {
    String::from_utf8_lossy(&hold(seen)).into_owned()
}

// Read only after the ceiling fires, never to decide a kill: undocumented CLI text, not a contract.
const DENIED: [&str; 3] = [
    "denied by the Claude Code auto mode classifier",
    "doesn't want to proceed with this tool use",
    "Claude requested permissions to use",
];

fn stalled_on(session: &str) -> Option<String> {
    let text = std::fs::read_to_string(transcript(session)?).ok()?;
    let tail: String = text.lines().rev().take(6).collect::<Vec<_>>().join(" ");
    DENIED
        .iter()
        .find(|mark| tail.contains(*mark))
        .map(|mark| (*mark).to_string())
}

enum Unanswered {
    Ceiling,
    Lost,
}

fn awaited(exited: &std::sync::mpsc::Receiver<bool>, ceiling: Duration) -> Result<(), Unanswered> {
    let started = Instant::now();
    loop {
        let left = ceiling.saturating_sub(started.elapsed());
        if left.is_zero() {
            return match exited.try_recv() {
                Ok(true) => Ok(()),
                Ok(false) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Err(Unanswered::Lost)
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => Err(Unanswered::Ceiling),
            };
        }
        match exited.recv_timeout(heartbeat(ceiling).min(left)) {
            Ok(watched) => {
                return if watched {
                    Ok(())
                } else {
                    Err(Unanswered::Lost)
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                crate::report::still_reviewing(started.elapsed().as_secs(), ceiling.as_secs());
            }
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return Err(Unanswered::Lost),
        }
    }
}

// Kills the whole process group (negative pid): the reviewer leads it, and whatever it spawned still holds the repo's claim.
fn kill(pid: u32) {
    unsafe { libc::kill(-(pid as i32), libc::SIGKILL) };
}

fn piped(
    role: Role,
    mut command: Command,
    prompt: &str,
    ceiling: Duration,
    session: &str,
) -> Result<Said, String> {
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run the reviewer: {e}"))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or("the reviewer's stdin is unavailable")?;
    let text = prompt.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(text.as_bytes()));
    let (read_out, out) = drain(child.stdout.take());
    let (read_err, err) = drain(child.stderr.take());
    let pid = child.id();
    crate::signals::spawned(role, pid);
    let started = Instant::now();
    let (exit, exited) = std::sync::mpsc::channel();
    // Not reaped here: reaping frees the pid and its group id, and a kill afterward could hit whatever the kernel reused them for.
    std::thread::spawn(move || {
        let mut seen: libc::siginfo_t = unsafe { std::mem::zeroed() };
        let watched =
            unsafe { libc::waitid(libc::P_PID, pid, &mut seen, libc::WEXITED | libc::WNOWAIT) };
        let _ = exit.send(watched == 0);
    });
    let status = match awaited(&exited, ceiling) {
        Ok(()) => {
            let by = Instant::now() + SETTLING;
            settled(&read_out, by);
            settled(&read_err, by);
            // Killed even on success: a helper the reviewer spawned would otherwise keep holding the repo's claim.
            kill(pid);
            let ended = child.wait();
            crate::signals::done();
            ended.map_err(|e| format!("the reviewer did not finish: {e}"))?
        }
        Err(Unanswered::Lost) => {
            kill(pid);
            let _ = child.wait();
            crate::signals::done();
            return Err(with_noise(
                "the reviewer did not finish, and this side lost track of it",
                &text_of(&err),
            ));
        }
        Err(Unanswered::Ceiling) => {
            kill(pid);
            let _ = child.wait();
            crate::signals::done();
            let mut said = format!(
            "the reviewer ran {}s without answering and was killed at the {}s ceiling.\nRaise it with --timeout <minutes> if a review here is genuinely this long; otherwise this is an agent that has stopped rather than one that is thinking.",
            started.elapsed().as_secs(),
            ceiling.as_secs()
        );
            if let Some(mark) = stalled_on(session) {
                said.push_str(&format!(
                    "\nIts transcript ends on a permission request: \"{mark}\"."
                ));
            }
            return Err(with_noise(&said, &text_of(&err)));
        }
    };
    let said = Said {
        out: text_of(&out),
        err: text_of(&err),
    };
    match writer.join() {
        Err(_) => return Err("the prompt was never written to the reviewer".to_string()),
        Ok(Err(e)) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            return Err(format!("cannot send the prompt to the reviewer: {e}"));
        }
        Ok(_) => {}
    }
    if !status.success() {
        let noise = clipped(&said.err);
        if noise.is_empty() {
            return Err(format!("the reviewer exited {status}"));
        }
        return Err(noise);
    }
    Ok(said)
}

// Mirrors how the agent derives its own project-dir name from cwd; never trusted blindly, only ever checked against the filesystem.
fn slug(dir: &std::path::Path) -> String {
    dir.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

pub fn transcript_path(session: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    Some(
        std::path::Path::new(&home)
            .join(".claude")
            .join("projects")
            .join(slug(&std::env::current_dir().ok()?))
            .join(format!("{session}.jsonl")),
    )
}

pub fn transcript(session: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let projects = std::path::Path::new(&home).join(".claude").join("projects");
    let named = format!("{session}.jsonl");
    let derived = transcript_path(session)?;
    if derived.is_file() {
        return Some(derived);
    }
    // Session ids are unique across every project, so a derived-path miss falls back to scanning rather than re-guessing the layout.
    std::fs::read_dir(&projects)
        .ok()?
        .flatten()
        .map(|project| project.path().join(&named))
        .find(|path| path.is_file())
}

// Closest thing to a time of death this side holds: something that kills the agent outright writes nothing on the way out.
pub fn last_wrote(session: &str) -> Option<u64> {
    let written = transcript(session)?.metadata().ok()?.modified().ok()?;
    Some(written.elapsed().ok()?.as_secs())
}

fn with_transcript(detail: &str, session: &str) -> String {
    match transcript(session) {
        Some(path) => format!(
            "{detail}\n\nwhat the reviewer actually did is in its transcript:\n  {}",
            path.display()
        ),
        None => detail.to_string(),
    }
}

fn with_noise(detail: &str, err: &str) -> String {
    let noise = clipped(err);
    if noise.is_empty() {
        return detail.to_string();
    }
    format!("{detail}\n\nthe reviewer also said:\n{noise}")
}

fn read_claude(said: &Said) -> Result<Answer, String> {
    let out = &said.out;
    let json: serde_json::Value = serde_json::from_str(out).map_err(|e| {
        with_noise(
            &format!("the reviewer's answer is not JSON: {e}"),
            &said.err,
        )
    })?;
    if json["is_error"].as_bool().unwrap_or(false) {
        let reported = json["result"].as_str().unwrap_or("no reason given");
        return Err(with_noise(
            &format!("the reviewer reported an error: {reported}"),
            &said.err,
        ));
    }
    let text = json["result"]
        .as_str()
        .ok_or_else(|| with_noise("the reviewer's answer has no result field", &said.err))?;
    let session = json["session_id"]
        .as_str()
        .ok_or_else(|| with_noise("the reviewer's answer has no session_id field", &said.err))?;
    // The model that actually ran, not the one requested — a fallback shouldn't be attributed to a model that never ran.
    let reviewer = json["modelUsage"]
        .as_object()
        .and_then(|used| used.keys().next().cloned())
        .unwrap_or_else(|| "claude".to_string());
    Ok(Answer {
        text: text.to_string(),
        reviewer,
        session: session.to_string(),
        stop_reason: json["stop_reason"].as_str().unwrap_or_default().to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::slug;

    #[test]
    fn a_directory_is_keyed_with_every_other_character_as_a_hyphen() {
        assert_eq!(
            slug(std::path::Path::new("/home/me/src/my_test.dir v2")),
            "-home-me-src-my-test-dir-v2"
        );
        assert_eq!(
            slug(std::path::Path::new("/home/me/.claude")),
            "-home-me--claude"
        );
    }
}
