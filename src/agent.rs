// Concern: how a named agent is invoked — its argv, its session, its ceiling, and where its answer and transcript are read back | Non-concern: what it is asked | IO: (system, prompt) -> answer

use crate::git;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// What a run gives back. The model and the session are read from the agent rather than asked of it: one it would have to guess at, the other it cannot know. Why it stopped comes back too, unused until an answer turns out to carry no verdict — which is the moment it says whether the reviewer was cut off or simply ignored its brief.
pub struct Answer {
    pub text: String,
    pub reviewer: String,
    pub session: String,
    pub stop_reason: String,
}

// The reviewer a round is handed to: a session nothing has used yet, or one an earlier round left behind. Named before the spawn either way, so the caller can write it down while there is still something to write it down about.
pub enum Session {
    Fresh(String),
    Resume(String),
}

impl Session {
    pub fn opened() -> Session {
        Session::Fresh(assigned())
    }

    pub fn resumed(id: &str) -> Session {
        Session::Resume(id.to_string())
    }

    pub fn id(&self) -> &str {
        let (Session::Fresh(id) | Session::Resume(id)) = self;
        id
    }
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
        session: &Session,
        model: Option<&str>,
        ceiling: Duration,
    ) -> Result<Answer, String> {
        claude(role, system, prompt, session, model, ceiling)
    }
}

// Through a file, because argv and a pipe both have a ceiling the standing instructions do not: they carry every rubric inlined.
fn system_file(text: &str) -> Result<std::path::PathBuf, String> {
    let path = git::git_path("AGENT_VERDICT_SYSTEM")?;
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(path)
}

// A review's model is the repo's call and passes through untouched — never checked against a list this build would have to keep current. Judging one line of text is not worth the model a review is worth, and which model is small enough for it is claude's own business.
fn claude_model(role: Role, asked: Option<&str>) -> Option<&str> {
    match role {
        Role::Review => asked,
        Role::JudgeIntent => Some("haiku"),
    }
}

// Judging one line of text has nothing to think about for half an hour, and the ceiling the author set is for the review they are paying for. Which question deserves which patience is knowledge about this agent, so it lives here beside the model.
const JUDGE_CEILING: Duration = Duration::from_secs(5 * 60);

fn claude_ceiling(role: Role, asked: Duration) -> Duration {
    match role {
        Role::Review => asked,
        Role::JudgeIntent => JUDGE_CEILING,
    }
}

// Chosen here and handed to the agent, rather than read back out of its answer. The two are the same identifier and a world apart in when they are known: read back, it arrives in the final answer, which is the one thing a run that crashed, hung or was killed never produced — so the id would be available in exactly the cases with nothing to use it for. Assigned first, it is known before anything can go wrong, and the transcript it names can be pointed at when something does.
fn assigned() -> String {
    let mut bytes = [0u8; 16];
    // Exactly sixteen bytes, taken by hand: the device never reaches an end, and anything that reads it to one reads for ever.
    let taken =
        std::fs::File::open("/dev/urandom").and_then(|mut urandom| urandom.read_exact(&mut bytes));
    match taken {
        Ok(()) => {}
        // A box without /dev/urandom still needs an id no live session already holds; the clock and the pid give one without pretending to be random.
        Err(_) => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            bytes[..8].copy_from_slice(&(now as u64).to_le_bytes());
            bytes[8..12].copy_from_slice(&std::process::id().to_le_bytes());
        }
    }
    // Version 4 and the variant bits, because the flag takes a uuid and refuses anything that is merely uuid-shaped.
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
    model: Option<&str>,
    ceiling: Duration,
) -> Result<Answer, String> {
    let file = system_file(system)?;
    let mut command = Command::new("claude");
    command.args(["-p", "--output-format", "json"]);
    command.arg("--append-system-prompt-file").arg(&file);
    // The same identifier under either flag: one opens the session, the other takes up the one already holding everything this reviewer had read.
    match session {
        Session::Fresh(id) => command.args(["--session-id", id]),
        Session::Resume(id) => command.args(["--resume", id]),
    };
    if let Some(model) = claude_model(role, model) {
        command.args(["--model", model]);
    }
    let told = |detail: String| with_transcript(&detail, session.id());
    let said = piped(
        command,
        prompt,
        claude_ceiling(role, ceiling),
        matches!(role, Role::Review),
    )
    .map_err(told)?;
    let _ = std::fs::remove_file(&file);
    read_claude(&said).map_err(told)
}

// Both halves of what the agent said. Its stderr is kept whatever the exit status, because the two do not agree: an agent can crash on stderr and still exit 0, and then the only account of what went wrong is the half a caller that trusts the status throws away.
struct Said {
    out: String,
    err: String,
}

// Long enough that the wait costs nothing against a ceiling counted in minutes, short enough that the kill lands while the shell that asked for it is still there to read the reason.
const POLL: Duration = Duration::from_millis(200);

// Far enough apart that a long review is a handful of lines, close enough that a killed one is placed to the minute. Against a ceiling short enough that a minute would pass in silence, it is a quarter of the ceiling instead: the point is that the wait is accounted for, not that it is accounted for every sixty seconds.
const HEARTBEAT: Duration = Duration::from_secs(60);

fn heartbeat(ceiling: Duration) -> Duration {
    HEARTBEAT.min(ceiling / 4)
}

// The whole of it, then a limit: an agent's crash is often one line and its answer is a page, and a diagnosis cut off mid-sentence sends the author after the wrong fault.
const KEPT: usize = 2000;

fn clipped(text: &str) -> String {
    let text = text.trim();
    match text.char_indices().nth(KEPT) {
        Some((cut, _)) => format!("{}…", &text[..cut]),
        None => text.to_string(),
    }
}

type Seen = std::sync::Arc<std::sync::Mutex<Vec<u8>>>;

// Drained from their own threads, because both pipes are bounded: an agent that fills either one blocks there until a read this side cannot reach while it waits for the process to exit. Into a buffer shared with this side rather than returned at the end, so what has arrived can be read without waiting for the end to come — a killed agent's pipe is held open by whatever it spawned, and a timeout that waits for that bounds nothing.
fn drain(pipe: Option<impl Read + Send + 'static>) -> (std::thread::JoinHandle<()>, Seen) {
    let seen: Seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let filling = std::sync::Arc::clone(&seen);
    let reader = std::thread::spawn(move || {
        let Some(mut pipe) = pipe else { return };
        let mut chunk = [0u8; 8192];
        // Chunked, and the lock taken only to append: held across the read it would be a lock on the agent's silence.
        while let Ok(n) = pipe.read(&mut chunk) {
            if n == 0 {
                return;
            }
            hold(&filling).extend_from_slice(&chunk[..n]);
        }
    });
    (reader, seen)
}

// A reader thread that panicked mid-append leaves what it had; nothing here is worth losing a diagnosis over.
fn hold(seen: &Seen) -> std::sync::MutexGuard<'_, Vec<u8>> {
    seen.lock().unwrap_or_else(|held| held.into_inner())
}

// Long enough that draining an answer already at EOF finishes inside it many times over, short enough that a pipe nothing will ever close is not mistaken for one still filling.
const SETTLING: Duration = Duration::from_secs(5);

fn settling(readers: &[std::thread::JoinHandle<()>]) {
    let until = Instant::now() + SETTLING;
    while Instant::now() < until && readers.iter().any(|r| !r.is_finished()) {
        std::thread::sleep(POLL);
    }
}

fn text_of(seen: &Seen) -> String {
    String::from_utf8_lossy(&hold(seen)).into_owned()
}

// Polled rather than waited on, because a wait has no deadline: the ceiling is the whole point, and an agent that has stopped answering must be killed and reported by the tool that started it, or it is left for whatever shell is holding the run to kill without a word.
fn wait_by(
    child: &mut Child,
    ceiling: Duration,
    narrate: bool,
) -> Result<std::process::ExitStatus, String> {
    let started = Instant::now();
    let mut said_at = Duration::ZERO;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {}
            Err(e) => return Err(format!("the reviewer did not finish: {e}")),
        }
        // The judge says nothing: it is one question, and a heartbeat under it would be noise standing where a finding should be.
        if narrate && started.elapsed() >= said_at + heartbeat(ceiling) {
            said_at = started.elapsed();
            crate::report::still_reviewing(said_at.as_secs(), ceiling.as_secs());
        }
        if started.elapsed() >= ceiling {
            // Killed before it is reported, so the message is not written about a process still running: the claim on the repo is released as this run exits, and a reviewer outliving it would be spending against a commit nobody is waiting for any more.
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!(
                "the reviewer ran {}s without answering and was killed at the {}s ceiling.\nRaise it with --timeout <minutes> if a review here is genuinely this long; otherwise this is an agent that has stopped rather than one that is thinking.",
                started.elapsed().as_secs(),
                ceiling.as_secs()
            ));
        }
        std::thread::sleep(POLL);
    }
}

fn piped(
    mut command: Command,
    prompt: &str,
    ceiling: Duration,
    narrate: bool,
) -> Result<Said, String> {
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("cannot run the reviewer: {e}"))?;
    let mut stdin = child.stdin.take().ok_or("the reviewer took no stdin")?;
    let text = prompt.to_string();
    let writer = std::thread::spawn(move || stdin.write_all(text.as_bytes()));
    let (reading_out, out) = drain(child.stdout.take());
    let (reading_err, err) = drain(child.stderr.take());
    let status = match wait_by(&mut child, ceiling, narrate) {
        Ok(status) => status,
        // Nothing is joined on this path: an agent killed at the ceiling leaves threads blocked on a prompt it never read and on pipes whatever it spawned still holds open, and this run has a refusal to deliver now. What it had managed to say is read out of the shared buffer instead, which is worth more than the timeout alone.
        Err(timeout) => return Err(with_noise(&timeout, &text_of(&err))),
    };
    // Waited for, not joined. An exited agent normally leaves both pipes at EOF and this returns at once with the tail of the answer — but the write end outlives it wherever the agent left something running that inherited it, and a join there waits for that instead, silently past the ceiling this exists to impose. Whatever has arrived by the end of the grace is the answer; what a still-open pipe would add is not coming.
    settling(&[reading_out, reading_err]);
    let said = Said {
        out: text_of(&out),
        err: text_of(&err),
    };
    // An agent that answers without reading closes the pipe first; that is its business, and the answer it prints is still the answer.
    match writer.join() {
        Err(_) => return Err("the prompt was never written to the reviewer".to_string()),
        Ok(Err(e)) if e.kind() != std::io::ErrorKind::BrokenPipe => {
            return Err(format!("cannot brief the reviewer: {e}"));
        }
        Ok(_) => {}
    }
    // Carried out rather than left on the terminal: a refusal it makes before answering — an unknown model is the one that matters — is said only here, and a caller that reports an exit status alone has thrown away the whole diagnosis.
    if !status.success() {
        let noise = clipped(&said.err);
        if noise.is_empty() {
            return Err(format!("the reviewer exited {status}"));
        }
        return Err(noise);
    }
    Ok(said)
}

// The agent keys a transcript on the directory it ran in, with everything that is not a letter or a digit written as a hyphen. Derived rather than asked for — there is nothing to ask — and therefore never trusted: what this returns is checked against the filesystem before it is named.
fn slug(dir: &std::path::Path) -> String {
    dir.to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

// Where the reviewer's own account of the round is: every file it read, every tool that answered it, and whatever it was in the middle of when it stopped. This tool reports what the reviewer said; the transcript is what it did, and after a failure that is the difference between a diagnosis and a shrug.
pub fn transcript(session: &str) -> Option<std::path::PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let projects = std::path::Path::new(&home).join(".claude").join("projects");
    let named = format!("{session}.jsonl");
    let derived = projects
        .join(slug(&std::env::current_dir().ok()?))
        .join(&named);
    if derived.is_file() {
        return Some(derived);
    }
    // A session is unique across every project the agent has ever run in, so a miss on the derived name is answered by looking rather than by guessing at the rule a second time. The layout is the agent's, and it is free to move it.
    std::fs::read_dir(&projects)
        .ok()?
        .flatten()
        .map(|project| project.path().join(&named))
        .find(|path| path.is_file())
}

// How long ago the reviewer last wrote to its own transcript, which is the closest thing to a time of death this side holds: a run killed by something else writes nothing on its way out, so the last line the agent managed is what dates the end. None where no transcript was ever written.
pub fn last_wrote(session: &str) -> Option<u64> {
    let written = transcript(session)?.metadata().ok()?.modified().ok()?;
    Some(written.elapsed().ok()?.as_secs())
}

// Named only where it exists: a path invented for a message sends the author to an empty prompt, which is worse than saying nothing.
fn with_transcript(detail: &str, session: &str) -> String {
    match transcript(session) {
        Some(path) => format!(
            "{detail}\n\nwhat the reviewer actually did is in its transcript:\n  {}",
            path.display()
        ),
        None => detail.to_string(),
    }
}

// What the agent muttered while failing, kept beside the failure. An agent that exits 0 having crashed leaves its whole account here, and a message built from the exit status alone reports the symptom this side saw rather than the fault that side had.
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
        .ok_or_else(|| with_noise("the reviewer's answer carries no result", &said.err))?;
    let session = json["session_id"]
        .as_str()
        .ok_or_else(|| with_noise("the reviewer's answer carries no session_id", &said.err))?;
    // Which model actually answered, rather than which one was asked for: a fallback would otherwise reach the trailer under the name of the model that never ran.
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
    use super::{assigned, slug};

    // Checked against a directory the agent has really keyed a transcript on: /home/me/src/my_test.dir v2 becomes -home-me-src-my-test-dir-v2, so a dot, an underscore and a space are hyphens exactly as a separator is.
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

    // The flag takes a uuid and refuses what is merely uuid-shaped, and two rounds must never be handed one id.
    #[test]
    fn an_assigned_session_is_a_version_four_uuid_and_is_not_reused() {
        let id = assigned();
        let fields: Vec<&str> = id.split('-').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(
            fields.iter().map(|f| f.len()).collect::<Vec<_>>(),
            vec![8, 4, 4, 4, 12],
            "{id}"
        );
        assert!(
            id.chars().all(|c| c.is_ascii_hexdigit() || c == '-'),
            "{id}"
        );
        assert!(fields[2].starts_with('4'), "{id} is not version 4");
        assert!(matches!(&fields[3][..1], "8" | "9" | "a" | "b"), "{id}");
        assert_ne!(id, assigned());
    }
}
