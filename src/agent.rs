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

#[derive(Clone, Copy)]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    pub fn named(name: &str) -> Result<Agent, String> {
        match name.trim() {
            "claude" => Ok(Agent::Claude),
            "codex" => Ok(Agent::Codex),
            other => Err(format!(
                "unknown agent '{other}': this build supports 'claude' and 'codex'.\n  git config --global agent-verdict.runner claude\n  git config --global agent-verdict.runner codex"
            )),
        }
    }

    pub fn run(
        self,
        role: Role,
        system: &str,
        prompt: &str,
        session: &Session,
        terms: &Terms,
    ) -> Result<Answer, String> {
        match self {
            Agent::Claude => claude(role, system, prompt, session, terms),
            Agent::Codex => codex(role, system, prompt, session, terms),
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    // Codex keeps the brief its thread started with, so a changed brief needs a fresh thread.
    pub fn resumable(self, session: &str, system: &str) -> bool {
        match self {
            Agent::Claude => self.transcript(session).is_some(),
            Agent::Codex => codex_transcript(session)
                .and_then(|path| std::fs::read_to_string(path).ok())
                .and_then(|log| briefed(&log))
                .is_some_and(|brief| brief == system),
        }
    }

    // Codex names a fresh session only once running, and announces its log then.
    pub fn transcript_path(self, session: &str) -> Option<std::path::PathBuf> {
        match self {
            Agent::Claude => claude_transcript_path(session),
            Agent::Codex => codex_transcript(session),
        }
    }

    pub fn transcript(self, session: &str) -> Option<std::path::PathBuf> {
        match self {
            Agent::Claude => claude_transcript(session),
            Agent::Codex => codex_transcript(session),
        }
    }

    // Closest thing to a time of death this side holds: something that kills the agent outright writes nothing on the way out.
    pub fn last_wrote(self, session: &str) -> Option<u64> {
        let written = self.transcript(session)?.metadata().ok()?.modified().ok()?;
        Some(written.elapsed().ok()?.as_secs())
    }

    // Codex files a session under its thread, once a link names it.
    pub fn resolved(self, session: &str) -> String {
        match self {
            Agent::Claude => session.to_string(),
            Agent::Codex => codex_thread(session),
        }
    }

    // Called once nothing will resume it.
    pub fn discard(self, session: &str) -> Result<(), String> {
        if session.is_empty() || session.starts_with('.') || session.contains('/') {
            return Ok(());
        }
        match self {
            Agent::Claude => claude_transcript(session).map_or(Ok(()), |path| forget_claude(&path)),
            Agent::Codex => {
                // A failed delete keeps the link, so a retry still resolves the handle.
                if codex_transcript(session).is_some() {
                    forget_codex(&codex_thread(session))?;
                }
                if let Ok(dir) = threads() {
                    let _ = std::fs::remove_file(dir.join(session));
                }
                Ok(())
            }
        }
    }

    // Codex threads it still indexes after their rollout went: no file scan finds them.
    pub fn stranded(self, ours: &dyn Fn(&str) -> bool) -> Result<Vec<Leftover>, String> {
        match self {
            Agent::Claude => Ok(Vec::new()),
            Agent::Codex => codex_stranded(ours),
        }
    }

    // Headless transcripts opened on a prompt `ours` recognizes.
    pub fn leftovers(self, ours: &dyn Fn(&str) -> bool) -> Vec<Leftover> {
        match self {
            Agent::Claude => claude_projects()
                .and_then(|projects| std::fs::read_dir(projects).ok())
                .into_iter()
                .flatten()
                .flatten()
                .filter_map(|project| std::fs::read_dir(project.path()).ok())
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
                .filter(|path| claude_opening(path).is_some_and(|first| ours(&first)))
                .map(|path| Leftover {
                    agent: self,
                    thread: String::new(),
                    touched: modified(&path),
                    path,
                })
                .collect(),
            Agent::Codex => codex_rollouts()
                .into_iter()
                .filter_map(|path| {
                    let thread = rollout_thread(path.file_name()?.to_str()?)
                        .filter(|_| path.extension().is_some_and(|ext| ext == "jsonl"))?;
                    codex_opened(&path, ours).then(|| Leftover {
                        agent: self,
                        thread,
                        touched: modified(&path),
                        path,
                    })
                })
                .collect(),
        }
    }

    // A command, not a rendered digest: a format invented here is a format maintained here forever.
    pub fn latest_activity(self) -> &'static str {
        match self {
            Agent::Claude => {
                r#"jq -rc 'select(.type=="assistant") | .timestamp[11:19] as $t | .message.content[] | if .type=="tool_use" then "\($t) \(.name) \(.input|tostring|gsub("\n";" "))" elif .type=="text" then "\($t) » \(.text|gsub("\n";" "))" else empty end'"#
            }
            Agent::Codex => {
                r#"jq -rc 'select(.type=="response_item") | .timestamp[11:19] as $t | .payload | if .type=="function_call" or .type=="custom_tool_call" then "\($t) \(.name) \(.arguments // .input|gsub("\n";" "))" elif .type=="message" and .role=="assistant" then .content[] | select(.type=="output_text") | "\($t) » \(.text|gsub("\n";" "))" else empty end'"#
            }
        }
    }

    // Read only after the ceiling fires, never to decide a kill: undocumented CLI text, not a contract.
    fn denials(self) -> &'static [&'static str] {
        match self {
            Agent::Claude => &[
                "denied by the Claude Code auto mode classifier",
                "doesn't want to proceed with this tool use",
                "Claude requested permissions to use",
            ],
            Agent::Codex => &[
                "rejected by user approval settings",
                "Network access was denied by the Codex sandbox network proxy",
            ],
        }
    }

    fn stalled_on(self, session: &str) -> Option<String> {
        let text = std::fs::read_to_string(self.transcript(session)?).ok()?;
        let tail: String = match self {
            Agent::Claude => text.lines().rev().take(6).collect::<Vec<_>>().join(" "),
            // Codex hands a denial back to the model instead of waiting on it, so only a denial in its last item is where it stopped.
            Agent::Codex => text
                .lines()
                .rev()
                .find(|line| {
                    serde_json::from_str::<serde_json::Value>(line)
                        .is_ok_and(|record| record["type"] == "response_item")
                })
                .unwrap_or_default()
                .to_string(),
        };
        self.denials()
            .iter()
            .find(|mark| tail.contains(*mark))
            .map(|mark| (*mark).to_string())
    }

    fn with_transcript(self, detail: &str, session: &str) -> String {
        match self.transcript(session) {
            Some(path) => format!(
                "{detail}\n\nwhat the reviewer actually did is in its transcript:\n  {}",
                path.display()
            ),
            None => detail.to_string(),
        }
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
    // Asked once and never resumed: a saved session would only crowd the user's own session lists.
    if let Role::JudgeIntent = role {
        command.arg("--no-session-persistence");
    }
    // dontAsk denies whatever would have prompted, since a headless run has no one to ask; plan mode additionally keeps a read-only reviewer from writing.
    let mode = if terms.read_only { "plan" } else { "dontAsk" };
    command.args(["--permission-mode", mode]);
    let told = |detail: String| Agent::Claude.with_transcript(&detail, session.id());
    let said = piped(
        Agent::Claude,
        role,
        command,
        prompt,
        terms.ceiling,
        session.id(),
        None,
    )
    .map_err(told)?;
    let _ = std::fs::remove_file(&file);
    exited(&said).map_err(told)?;
    read_claude(&said).map_err(told)
}

// stderr is kept regardless of exit status: an agent can crash on stderr and still exit 0.
struct Said {
    out: String,
    err: String,
    status: std::process::ExitStatus,
}

fn exited(said: &Said) -> Result<(), String> {
    if said.status.success() {
        return Ok(());
    }
    let noise = clipped(&said.err);
    if noise.is_empty() {
        return Err(format!("the reviewer exited {}", said.status));
    }
    Err(noise)
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

// Shown everything read so far after each read, until it answers true.
type Tap = Box<dyn FnMut(&[u8]) -> bool + Send>;

// Both pipes are bounded, so drained on their own threads into a shared buffer readable before EOF — a killed agent's pipe can stay open in whatever it spawned.
fn drain(
    pipe: Option<impl Read + Send + 'static>,
    mut tap: Option<Tap>,
) -> (std::sync::mpsc::Receiver<()>, Seen) {
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
                let mut held = hold(&filling);
                held.extend_from_slice(&chunk[..n]);
                if tap.as_mut().is_some_and(|tap| tap(&held)) {
                    tap = None;
                }
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
    agent: Agent,
    role: Role,
    mut command: Command,
    prompt: &str,
    ceiling: Duration,
    session: &str,
    tap: Option<Tap>,
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
    let (read_out, out) = drain(child.stdout.take(), tap);
    let (read_err, err) = drain(child.stderr.take(), None);
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
            if let Some(mark) = agent.stalled_on(session) {
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
        status,
    };
    match writer.join() {
        Err(_) => return Err("the prompt was never written to the reviewer".to_string()),
        Ok(Err(e)) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            return Err(format!("cannot send the prompt to the reviewer: {e}"));
        }
        Ok(_) => {}
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

fn claude_home() -> Option<std::path::PathBuf> {
    match std::env::var_os("CLAUDE_CONFIG_DIR").filter(|dir| !dir.is_empty()) {
        Some(dir) => Some(std::path::PathBuf::from(dir)),
        None => Some(std::path::Path::new(&std::env::var("HOME").ok()?).join(".claude")),
    }
}

fn claude_projects() -> Option<std::path::PathBuf> {
    Some(claude_home()?.join("projects"))
}

fn claude_transcript_path(session: &str) -> Option<std::path::PathBuf> {
    Some(
        claude_projects()?
            .join(slug(&std::env::current_dir().ok()?))
            .join(format!("{session}.jsonl")),
    )
}

fn claude_transcript(session: &str) -> Option<std::path::PathBuf> {
    let projects = claude_projects()?;
    let named = format!("{session}.jsonl");
    let derived = claude_transcript_path(session)?;
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

pub struct Leftover {
    pub agent: Agent,
    pub path: std::path::PathBuf,
    pub touched: Option<std::time::SystemTime>,
    thread: String,
}

impl Leftover {
    pub fn forget(&self) -> Result<(), String> {
        match self.agent {
            Agent::Claude => forget_claude(&self.path),
            Agent::Codex => forget_codex(&self.thread),
        }
    }
}

fn modified(path: &std::path::Path) -> Option<std::time::SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

// Codex's index is its own, undocumented; read-only through sqlite3, and only for what a file scan cannot see.
fn codex_stranded(ours: &dyn Fn(&str) -> bool) -> Result<Vec<Leftover>, String> {
    let Some(home) = codex_home() else {
        return Ok(Vec::new());
    };
    let indexes: Vec<std::path::PathBuf> = std::fs::read_dir(&home)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("state_") && name.ends_with(".sqlite"))
        })
        .collect();
    let mut found = Vec::new();
    for index in indexes {
        let out = Command::new("sqlite3")
            .args(["-readonly", "-json"])
            .arg(&index)
            .arg("select id, rollout_path, updated_at, first_user_message from threads where originator = 'codex_exec'")
            .stdin(Stdio::null())
            .output()
            .map_err(|e| format!("cannot run sqlite3 to read {}: {e}", index.display()))?;
        if !out.status.success() {
            return Err(format!(
                "cannot read {}: {}",
                index.display(),
                String::from_utf8_lossy(&out.stderr).trim()
            ));
        }
        let text = String::from_utf8_lossy(&out.stdout);
        let rows: Vec<serde_json::Value> = if text.trim().is_empty() {
            Vec::new()
        } else {
            serde_json::from_str(&text).map_err(|e| format!("{}: {e}", index.display()))?
        };
        for row in rows {
            let (Some(id), Some(rollout), Some(first)) = (
                row["id"].as_str(),
                row["rollout_path"].as_str(),
                row["first_user_message"].as_str(),
            ) else {
                continue;
            };
            let path = std::path::PathBuf::from(rollout);
            if path.exists() || !ours(first) {
                continue;
            }
            let touched = row["updated_at"]
                .as_u64()
                .map(|at| std::time::UNIX_EPOCH + Duration::from_secs(at));
            found.push(Leftover {
                agent: Agent::Codex,
                path,
                touched,
                thread: id.to_string(),
            });
        }
    }
    Ok(found)
}

// Transcripts run to tens of megabytes; the prompt opens them.
const OPENING_RECORDS: usize = 50;

fn opening_records(path: &std::path::Path) -> impl Iterator<Item = serde_json::Value> {
    use std::io::BufRead;
    std::fs::File::open(path)
        .ok()
        .map(std::io::BufReader::new)
        .into_iter()
        .flat_map(|reader| reader.lines().map_while(Result::ok))
        .take(OPENING_RECORDS)
        .filter_map(|line| serde_json::from_str(&line).ok())
}

// The prompt a headless run opened on; a session someone typed into is never this tool's.
fn claude_opening(path: &std::path::Path) -> Option<String> {
    let first = opening_records(path).find(|record| record["type"] == "user")?;
    if first["entrypoint"] != "sdk-cli" {
        return None;
    }
    first["message"]["content"].as_str().map(str::to_string)
}

// Codex opens a thread on context messages of its own.
fn codex_opened(path: &std::path::Path, ours: &dyn Fn(&str) -> bool) -> bool {
    let records: Vec<serde_json::Value> = opening_records(path).collect();
    let headless = records
        .iter()
        .find(|record| record["type"] == "session_meta")
        .is_some_and(|meta| meta["payload"]["originator"] == "codex_exec");
    headless
        && records
            .iter()
            .filter(|record| {
                record["type"] == "response_item"
                    && record["payload"]["type"] == "message"
                    && record["payload"]["role"] == "user"
            })
            .filter_map(|record| record["payload"]["content"].as_array())
            .flatten()
            .filter_map(|item| item["text"].as_str())
            .any(ours)
}

fn forget_claude(transcript: &std::path::Path) -> Result<(), String> {
    match std::fs::remove_file(transcript) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            return Err(format!("cannot delete {}: {e}", transcript.display()))
        }
        _ => {}
    }
    let Some(id) = transcript.file_stem().and_then(|stem| stem.to_str()) else {
        return Ok(());
    };
    let _ = std::fs::remove_dir_all(transcript.with_extension(""));
    let Some(home) = claude_home() else {
        return Ok(());
    };
    let _ = std::fs::remove_dir_all(home.join("session-env").join(id));
    let _ = std::fs::remove_dir_all(home.join("file-history").join(id));
    for todo in std::fs::read_dir(home.join("todos"))
        .into_iter()
        .flatten()
        .flatten()
    {
        if todo.file_name().to_string_lossy().starts_with(id) {
            let _ = std::fs::remove_file(todo.path());
        }
    }
    Ok(())
}

const DELETING: Duration = Duration::from_secs(30);

// Only codex's own delete: removing the rollout behind its back leaves the thread indexed with nothing to find it by.
fn forget_codex(thread: &str) -> Result<(), String> {
    let mut child = Command::new("codex")
        .args(["delete", "--force", thread])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run codex delete: {e}"))?;
    let (drained, err) = drain(child.stderr.take(), None);
    let by = Instant::now() + DELETING;
    while matches!(child.try_wait(), Ok(None)) {
        if Instant::now() > by {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "codex delete {thread} did not finish in {}s",
                DELETING.as_secs()
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let status = child
        .wait()
        .map_err(|e| format!("codex delete {thread}: {e}"))?;
    if status.success() {
        return Ok(());
    }
    settled(&drained, Instant::now() + SETTLING);
    let said = clipped(&text_of(&err));
    Err(format!("codex delete {thread} exited {status}: {said}"))
}

// Linux's cap on a single argument (MAX_ARG_STRLEN), which the brief rides in as one.
const ARGUMENT: usize = 128 * 1024;

fn codex(
    role: Role,
    system: &str,
    prompt: &str,
    session: &Session,
    terms: &Terms,
) -> Result<Answer, String> {
    // Codex's only file form, model_instructions_file, replaces its base prompt rather than adding to it.
    let instructions = format!("developer_instructions=\"{}\"", escaped(system));
    if instructions.len() >= ARGUMENT {
        return Err(format!(
            "this gate's brief is {} KiB, and codex takes it as one argument, which Linux caps at {} KiB.\nSplit its criteria across more gates, or review it under claude, which reads its brief from a file.",
            instructions.len() / 1024,
            ARGUMENT / 1024
        ));
    }
    let mut command = Command::new("codex");
    command.args(["exec", "--json", "-c"]).arg(&instructions);
    if let Some(model) = terms.model {
        command.args(["--model", model]);
    }
    // No smaller judge model is pinned: codex's lineup moves too fast. Never resumed, so never saved.
    if let Role::JudgeIntent = role {
        command.args(["-c", "model_reasoning_effort=\"low\"", "--ephemeral"]);
    }
    // exec refuses what would have prompted unless the host's config sets an auto-reviewer; without --read-only, that config picks the sandbox.
    if terms.read_only {
        command.args(["--sandbox", "read-only"]);
    }
    let tap = match session {
        Session::Fresh(handle) => {
            command.arg("-");
            Some(linking(handle, role)?)
        }
        Session::Resume(id) => {
            command.args(["resume", &codex_thread(id), "-"]);
            None
        }
    };
    let told = |detail: String| Agent::Codex.with_transcript(&detail, session.id());
    let ran = piped(
        Agent::Codex,
        role,
        command,
        prompt,
        terms.ceiling,
        session.id(),
        tap,
    )
    .and_then(|said| read_codex(&said))
    .map_err(told);
    // Only a fresh review that failed is taken up again through its link; a resume gets one attempt.
    let kept = ran.is_err() && matches!(session, Session::Fresh(_)) && matches!(role, Role::Review);
    if let (false, Ok(dir)) = (kept, threads()) {
        let _ = std::fs::remove_file(dir.join(session.id()));
    }
    let (text, thread) = ran?;
    // The event stream never names the model; the transcript does, per turn.
    let reviewer = codex_transcript(&thread)
        .and_then(|path| std::fs::read_to_string(path).ok())
        .and_then(|log| ran_on(&log))
        .unwrap_or_else(|| "codex".to_string());
    Ok(Answer {
        text,
        reviewer,
        session: thread,
        stop_reason: String::new(),
    })
}

// JSON's escapes, plus DEL, which JSON leaves raw and a TOML basic string forbids.
fn escaped(text: &str) -> String {
    let quoted = serde_json::Value::from(text).to_string();
    quoted[1..quoted.len() - 1].replace('\u{7f}', "\\u007F")
}

// The brief codex was started with: the first developer message, as -c developer_instructions put it there.
fn briefed(log: &str) -> Option<String> {
    log.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|record| {
            record["type"] == "response_item"
                && record["payload"]["type"] == "message"
                && record["payload"]["role"] == "developer"
        })?["payload"]["content"]
        .as_array()?
        .iter()
        .find(|item| item["type"] == "input_text")?["text"]
        .as_str()
        .map(str::to_string)
}

fn threads() -> Result<std::path::PathBuf, String> {
    git::git_path("AGENT_VERDICT_CODEX_THREADS")
}

// Past any round worth taking up: an audit's or an abandoned round's link is never read again.
const STALE: Duration = Duration::from_secs(7 * 24 * 60 * 60);

// Codex picks its own session id; linking the handle to it lets a round killed mid-review resume it.
fn linking(handle: &str, role: Role) -> Result<Tap, String> {
    let dir = threads()?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    for old in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
        let age = old.metadata().and_then(|m| m.modified()).ok();
        if age
            .and_then(|at| at.elapsed().ok())
            .is_some_and(|age| age > STALE)
        {
            let _ = std::fs::remove_file(old.path());
        }
    }
    let link = dir.join(handle);
    let handle = handle.to_string();
    let sessions = codex_home().map(|home| home.join("sessions"));
    Ok(Box::new(move |seen: &[u8]| {
        let Some(thread) = thread_started(&String::from_utf8_lossy(seen)) else {
            return false;
        };
        let _ = std::fs::write(&link, &thread);
        if let Role::Review = role {
            crate::report::thread_named(&handle, &thread);
            // The file, and so its day and time, does not exist yet.
            if let Some(sessions) = &sessions {
                let log = sessions.join(format!("*/*/*/rollout-*-{thread}.jsonl"));
                crate::report::progress_log(&log, Agent::Codex.latest_activity());
            }
        }
        true
    }))
}

fn codex_thread(session: &str) -> String {
    threads()
        .ok()
        .and_then(|dir| std::fs::read_to_string(dir.join(session)).ok())
        .map_or_else(|| session.to_string(), |thread| thread.trim().to_string())
}

fn thread_started(out: &str) -> Option<String> {
    out.lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|event| event["type"] == "thread.started")
        .and_then(|event| event["thread_id"].as_str().map(str::to_string))
}

fn codex_home() -> Option<std::path::PathBuf> {
    match std::env::var_os("CODEX_HOME").filter(|home| !home.is_empty()) {
        Some(home) => Some(home.into()),
        None => Some(std::path::Path::new(&std::env::var("HOME").ok()?).join(".codex")),
    }
}

// rollout-<YYYY-MM-DDTHH-MM-SS>-<thread>[_<rollout>].jsonl[.zst], as codex-rs/rollout names it; only codex reads a .zst back.
fn rollout_thread(name: &str) -> Option<String> {
    let core = name.strip_prefix("rollout-")?;
    let core = core.strip_suffix(".zst").unwrap_or(core);
    Some(
        core.strip_suffix(".jsonl")?
            .get(20..)?
            .split('_')
            .next()?
            .to_string(),
    )
}

fn rollout_of(name: &str, thread: &str) -> bool {
    rollout_thread(name).is_some_and(|named| named == thread)
}

// Filed under sessions/YYYY/MM/DD by a start day this side does not know.
fn codex_rollouts() -> Vec<std::path::PathBuf> {
    let Some(home) = codex_home() else {
        return Vec::new();
    };
    let mut found = vec![home.join("sessions")];
    for _ in 0..4 {
        found = found
            .iter()
            .filter_map(|dir| std::fs::read_dir(dir).ok())
            .flat_map(|entries| entries.flatten().map(|entry| entry.path()))
            .collect();
    }
    found
}

fn codex_transcript(session: &str) -> Option<std::path::PathBuf> {
    let thread = codex_thread(session);
    codex_rollouts().into_iter().find(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| rollout_of(name, &thread))
    })
}

fn ran_on(transcript: &str) -> Option<String> {
    transcript
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find(|record| record["type"] == "turn_context")
        .and_then(|record| record["payload"]["model"].as_str().map(str::to_string))
}

// An API refusal arrives as the provider's JSON error body, quoted whole as the message.
fn unwrapped(message: &str) -> String {
    serde_json::from_str::<serde_json::Value>(message)
        .ok()
        .and_then(|body| body["error"]["message"].as_str().map(str::to_string))
        .unwrap_or_else(|| message.to_string())
}

// The answer is the turn's last agent_message; only turn.completed says the turn ended.
fn read_codex(said: &Said) -> Result<(String, String), String> {
    let mut text = None;
    let mut ended = None;
    let mut last_error = None;
    for line in said.out.lines() {
        let Ok(event) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        match event["type"].as_str().unwrap_or_default() {
            "item.completed" if event["item"]["type"] == "agent_message" => {
                text = event["item"]["text"].as_str().map(str::to_string);
            }
            "turn.completed" => ended = Some(Ok(())),
            "turn.failed" => {
                let reported = event["error"]["message"]
                    .as_str()
                    .unwrap_or("no reason given");
                ended = Some(Err(unwrapped(reported)));
            }
            // Not fatal on its own: codex reports a retried stream this way and carries on.
            "error" => last_error = event["message"].as_str().map(unwrapped),
            _ => {}
        }
    }
    // Read before the exit status: codex exits 1 on a failed turn, and only stdout says why.
    match (ended, last_error) {
        (Some(Ok(())), _) => exited(said)?,
        (Some(Err(reported)), _) => {
            return Err(with_noise(
                &format!("the reviewer reported an error: {reported}"),
                &said.err,
            ))
        }
        (None, _) if !said.status.success() => exited(said)?,
        (None, Some(reported)) => {
            return Err(with_noise(
                &format!("the reviewer's turn never completed; its last error: {reported}"),
                &said.err,
            ))
        }
        (None, None) => return Err(with_noise("the reviewer's turn never completed", &said.err)),
    }
    let thread = thread_started(&said.out)
        .ok_or_else(|| with_noise("the reviewer's events have no thread.started", &said.err))?;
    let text = text.ok_or_else(|| {
        with_noise(
            "the reviewer finished its turn without a message",
            &said.err,
        )
    })?;
    Ok((text, thread))
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
    use super::{escaped, read_codex, rollout_of, slug, Said};

    // Raw wait statuses: the exit code sits in the second byte.
    fn said(out: &str, code: i32, err: &str) -> Said {
        Said {
            out: out.to_string(),
            err: err.to_string(),
            status: std::os::unix::process::ExitStatusExt::from_raw(code << 8),
        }
    }

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

    // Shaped as codex-rs/exec/src/exec_events.rs serializes them: the session first, commentary before the answer.
    #[test]
    fn a_codex_answer_is_its_last_message_in_the_session_it_started() {
        let out = r#"{"type":"thread.started","thread_id":"0199a213-81c0-7800-8aa1-bbab2a035a53"}
{"type":"turn.started"}
{"type":"item.completed","item":{"id":"item_0","type":"reasoning","text":"**Reading the diff**"}}
{"type":"item.completed","item":{"id":"item_1","type":"agent_message","text":"Reading the staged change."}}
{"type":"item.completed","item":{"id":"item_2","type":"command_execution","command":"bash -lc 'git diff --cached'","aggregated_output":"diff --git","exit_code":0,"status":"completed"}}
{"type":"item.completed","item":{"id":"item_3","type":"agent_message","text":"No findings.\nVERDICT: major=0 moderate=0 minor=0"}}
{"type":"turn.completed","usage":{"input_tokens":24763,"cached_input_tokens":24448,"output_tokens":122,"reasoning_output_tokens":0}}
"#;
        let (text, thread) = read_codex(&said(out, 0, "")).expect("an answer");
        assert_eq!(text, "No findings.\nVERDICT: major=0 moderate=0 minor=0");
        assert_eq!(thread, "0199a213-81c0-7800-8aa1-bbab2a035a53");
    }

    // As codex-cli 0.162.1 reported a bad --model: exit 1, tracing noise on stderr, the API's error body on stdout.
    #[test]
    fn a_failed_codex_turn_is_an_error_naming_its_reason() {
        let out = r#"{"type":"thread.started","thread_id":"0199a213-81c0-7800-8aa1-bbab2a035a53"}
{"type":"turn.started"}
{"type":"error","message":"Reconnecting... 1/5"}
{"type":"turn.failed","error":{"message":"{\"type\":\"error\",\"status\":400,\"error\":{\"type\":\"invalid_request_error\",\"message\":\"The 'gpt-nope' model is not supported when using Codex with a ChatGPT account.\",\"code\":\"model_not_found\"}}"}}
"#;
        let noise = "2026-10-28T10:00:00Z ERROR codex_api: websocket closed: 401";
        let failed = read_codex(&said(out, 1, noise)).expect_err("a failed turn");
        assert!(
            failed.contains(
                "reported an error: The 'gpt-nope' model is not supported when using Codex"
            ),
            "{failed}"
        );
    }

    // A TOML basic string forbids DEL, which JSON leaves raw.
    #[test]
    fn a_brief_is_escaped_for_a_toml_basic_string() {
        assert_eq!(escaped("a\u{7f}\"b\"\n"), "a\\u007F\\\"b\\\"\\n");
    }

    #[test]
    fn a_codex_transcript_is_named_by_its_thread() {
        let thread = "01a1330d-35ec-7522-9371-755e554c8d2c";
        assert!(rollout_of(
            "rollout-2026-10-12T19-17-46-01a1330d-35ec-7522-9371-755e554c8d2c.jsonl",
            thread
        ));
        assert!(rollout_of(
            "rollout-2026-10-12T19-17-46-01a1330d-35ec-7522-9371-755e554c8d2c_01a1330d-35ec-7522-9371-000000000000.jsonl",
            thread
        ));
        assert!(rollout_of(
            "rollout-2026-10-12T19-17-46-01a1330d-35ec-7522-9371-755e554c8d2c.jsonl.zst",
            thread
        ));
        assert!(!rollout_of(
            "rollout-2026-10-12T19-18-23-01a1330d-c617-7fa1-92a4-c26de74d9a1d.jsonl",
            thread
        ));
    }
}
