// Concern: a throwaway git repo, a declared hook, and ways to run the binary | Non-concern: what any outcome should be | IO: (files, argv) -> status, output

// Shared by two test binaries, each of which uses part of it: unused-in-this-crate is the normal state here, not a finding.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

pub const WHOLE: &str = "--confirm-reviewing-the-whole-repo-not-a-commit";
pub const STAGED_ONLY: &str = "--confirm-attesting-the-staged-version-not-the-working-tree";
pub const BIN: &str = env!("CARGO_BIN_EXE_git-agent-verdict");
static SEQ: AtomicU32 = AtomicU32::new(0);

// Well-formed and traceable to nothing: every refusal that fires before a token is looked up can be reached with it.
pub const DUMMY: &str =
    "subject\n\nbody\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=2 token=deadbeef\n";

pub const STANDARDS: &str = r#""$1" standards --doc rubric.md --path ."#;
pub const PROSE: &str = r#""$1" prose --simple --doc rubric.md --path ."#;

pub struct Repo {
    pub dir: PathBuf,
}

pub fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

pub struct Run {
    pub code: i32,
    pub out: String,
    pub err: String,
}

impl Repo {
    pub fn new() -> Self {
        let n = SEQ.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("gav-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        let dir = dir.canonicalize().expect("canonical temp dir");
        git(&dir, &["init", "-q"]);
        git(&dir, &["config", "user.email", "t@t"]);
        git(&dir, &["config", "user.name", "t"]);
        let repo = Repo { dir };
        std::fs::create_dir_all(repo.home()).expect("home dir");
        repo.write("rubric.md", "the standard");
        repo.write("src.rs", "code");
        repo
    }

    pub fn write(&self, name: &str, body: &str) {
        std::fs::write(self.dir.join(name), body).expect("write");
    }

    pub fn home(&self) -> PathBuf {
        self.dir.with_extension("home")
    }

    // Mirrors `slug` in src/agent.rs, duplicated since this crate ships no library: an edit to either wants the other.
    pub fn transcript_for(&self, session: &str) {
        let slug: String = self
            .dir
            .to_string_lossy()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect();
        let dir = self.home().join(".claude").join("projects").join(slug);
        std::fs::create_dir_all(&dir).expect("projects dir");
        std::fs::write(dir.join(format!("{session}.jsonl")), "{}\n").expect("transcript");
    }

    pub fn prompts(&self) -> String {
        self.read("prompts")
    }

    pub fn last_assigned(&self) -> String {
        self.read("assigned-sessions")
            .lines()
            .rfind(|l| !l.trim().is_empty())
            .expect("an assigned session")
            .to_string()
    }

    // A rubric the repo can never stage, which is how the setup guide tells a repo to keep one: `$KB/standards.md`, expanded by the hook's own shell.
    pub fn write_outside(&self, name: &str, body: &str) -> String {
        let path = self.dir.with_extension("outside");
        std::fs::create_dir_all(&path).expect("outside dir");
        let file = path.join(name);
        std::fs::write(&file, body).expect("write outside");
        file.to_string_lossy().into_owned()
    }

    pub fn stage(&self, paths: &[&str]) {
        for p in paths {
            git(&self.dir, &["add", p]);
        }
    }

    // Sealed off from the host's own git config: a test that inherited `agent-verdict.runner` would call the real reviewer, cost real money, and pass for the wrong reason.
    pub fn capture(&self, args: &[&str]) -> Run {
        self.capture_in(".", args)
    }

    pub fn capture_in(&self, subdir: &str, args: &[&str]) -> Run {
        let cwd = self.dir.join(subdir);
        self.capture_at(&cwd, args)
    }

    // The one place every run is isolated from the host's git config and the real agent; a test that built its own command instead would skip it.
    fn sealed(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(BIN);
        command
            .current_dir(cwd)
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            // Its own HOME: the tool writes transcripts and reports under it, and the real one would leak between tests.
            .env("HOME", self.home())
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.dir.join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .args(args);
        command
    }

    pub fn capture_at(&self, cwd: &Path, args: &[&str]) -> Run {
        let out = self.sealed(cwd, args).output().expect("binary runs");
        Run {
            code: out.status.code().expect("exited"),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
    }

    pub fn running(&self, args: &[&str]) -> std::process::Child {
        self.sealed(&self.dir, args)
            .stderr(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("binary runs")
    }

    pub fn run(&self, msg: &str, args: &[&str]) -> (i32, String) {
        self.write("MSG", msg);
        let mut argv = vec!["MSG"];
        argv.extend_from_slice(args);
        let run = self.capture(&argv);
        (run.code, run.err)
    }

    pub fn standards(&self, msg: &str) -> (i32, String) {
        self.run(msg, &["standards", "--doc", "rubric.md", "--path", "."])
    }

    pub fn bare(&self, args: &[&str]) -> (i32, String) {
        let run = self.capture(args);
        (run.code, run.err)
    }

    pub fn root(&self) -> String {
        self.dir.to_string_lossy().into_owned()
    }

    // The round's log is one file, folded into both streams: which one a line was headed for is no longer knowable.
    fn round(&self, args: &[&str]) -> Run {
        let started = self.capture(args);
        if started.code != 0 {
            return started;
        }
        let waited = self.awaited();
        let said = self.round_logs();
        Run {
            code: waited.code,
            out: format!("{}{}{}", started.out, waited.out, said),
            err: format!("{}{}{}", started.err, waited.err, said),
        }
    }

    pub fn awaited(&self) -> Run {
        let root = self.root();
        self.capture(&["await", "--repo", &root])
    }

    pub fn capture_attest(&self, intent: &str) -> Run {
        let root = self.root();
        self.capture(&["attest", "--repo", &root, "--intent", intent])
    }

    pub fn commit(&self) -> Run {
        let root = self.root();
        self.capture(&["commit", "--repo", &root])
    }

    pub fn aborted(&self) -> Run {
        let root = self.root();
        self.capture(&["abort", "--repo", &root])
    }

    pub fn round_logs(&self) -> String {
        let Some(at) = self.last_round() else {
            return String::new();
        };
        let Ok(entries) = std::fs::read_dir(at) else {
            return String::new();
        };
        let mut said: Vec<String> = entries
            .flatten()
            .filter_map(|e| std::fs::read_to_string(e.path()).ok())
            .collect();
        said.sort();
        said.join("")
    }

    pub fn last_round(&self) -> Option<PathBuf> {
        let text = std::fs::read_to_string(self.dir.join(".git/agent-verdict.last")).ok()?;
        let at = text.trim().to_string();
        (!at.is_empty()).then(|| PathBuf::from(at))
    }

    pub fn attest(&self, intent: &str) -> Run {
        let root = self.root();
        self.round(&["attest", "--repo", &root, "--intent", intent])
    }

    pub fn audit(&self) -> Run {
        let root = self.root();
        self.round(&["audit", "--repo", &root, WHOLE])
    }

    pub fn again(&self) -> Run {
        let root = self.root();
        self.round(&["attest", "--repo", &root])
    }

    pub fn attest_staged_only(&self, intent: &str) -> Run {
        let root = self.root();
        self.round(&["attest", "--repo", &root, "--intent", intent, STAGED_ONLY])
    }

    // A ceiling stated in seconds: proving a hung reviewer is killed must not cost the half hour the default allows.
    pub fn attest_within(&self, intent: &str, ceiling: &str) -> Run {
        let root = self.root();
        self.round(&[
            "attest",
            "--repo",
            &root,
            "--intent",
            intent,
            "--timeout",
            ceiling,
        ])
    }

    // One declaration per line, run through the binary by absolute path: a name resolves from PATH, which passes on a box with the tool installed and fails in CI.
    pub fn hook(&self, lines: &[&str]) {
        let body: String = lines.iter().map(|l| format!("{BIN} {l}\n")).collect();
        let hooks = self.dir.join("hooks");
        std::fs::create_dir_all(&hooks).expect("hooks dir");
        let path = hooks.join("commit-msg");
        // `set -e`, as the setup guide writes it: without it a refusing line lets the rest of the hook run, and no test sees what a real hook does.
        std::fs::write(&path, format!("#!/bin/sh\nset -e\n{body}")).expect("place hook");
        let mode = std::os::unix::fs::PermissionsExt::from_mode(0o755);
        std::fs::set_permissions(&path, mode).expect("chmod");
        git(&self.dir, &["config", "core.hooksPath", "hooks"]);
    }

    pub fn reviewer_prompt(&self, gate: &str) -> (i32, String, String) {
        let run = self.capture(&["--reviewer-prompt", gate]);
        (run.code, run.out, run.err)
    }

    pub fn declare(&self, verdict: &str, gates: &[&str]) {
        self.declare_runner(&format!("printf '{verdict}\\n'"), gates);
    }

    // The stub wraps the reviewer's text as the JSON the real `claude` CLI returns.
    pub fn declare_runner(&self, body: &str, gates: &[&str]) {
        self.hook(gates);
        let bin = self.dir.join("bin");
        std::fs::create_dir_all(&bin).expect("bin dir");
        let stub = format!(
            r#"#!/bin/sh
system=""; resume=""; assigned=""; mode=""
while [ $# -gt 0 ]; do
  case "$1" in
    --append-system-prompt-file) system="$2"; shift 2 ;;
    --resume) resume="$2"; shift 2 ;;
    --session-id) assigned="$2"; shift 2 ;;
    --permission-mode) mode="$2"; shift 2 ;;
    --model) model="$2"; shift 2 ;;
    *) shift ;;
  esac
done
echo "$assigned" >> assigned-sessions
export AGENT_VERDICT_SYSTEM="$system" AGENT_VERDICT_PRIOR_SESSION="$resume" AGENT_VERDICT_MODE="$mode"
if grep -q "You judge one line of text" "$system" 2>/dev/null; then
  [ -f slow-judge ] && echo $$ > judging && sleep 30
  text=$(cat judge-answer 2>/dev/null || echo "VERDICT: accepted")
else
  echo "[$model]" >> asked-model
  if [ -f refuse-model ]; then
    M="$model" python3 -c 'import json, os; print(json.dumps({{"is_error": True, "result": "There is an issue with the selected model (" + os.environ["M"] + "). It may not exist or you may not have access to it."}}))'
    exit 0
  fi
  text=$({body})
fi
export SID=$(cat session 2>/dev/null || echo s-1) TEXT="$text"
python3 -c 'import json, os; print(json.dumps({{"is_error": False, "result": os.environ["TEXT"], "session_id": os.environ["SID"], "modelUsage": {{"fake-model": {{}}}}}}))'
"#
        );
        let path = bin.join("claude");
        std::fs::write(&path, stub).expect("write stub");
        let mode = std::os::unix::fs::PermissionsExt::from_mode(0o755);
        std::fs::set_permissions(&path, mode).expect("chmod");
        git(&self.dir, &["config", "agent-verdict.runner", "claude"]);
    }

    // Unlike declare_runner, this writes the reviewer process itself: a crash has no well-formed answer to wrap. The judge still answers normally.
    pub fn declare_agent(&self, reviewer: &str, gates: &[&str]) {
        self.hook(gates);
        let bin = self.dir.join("bin");
        std::fs::create_dir_all(&bin).expect("bin dir");
        let stub = format!(
            r#"#!/bin/sh
system=""
while [ $# -gt 0 ]; do
  case "$1" in
    --append-system-prompt-file) system="$2"; shift 2 ;;
    *) shift ;;
  esac
done
if grep -q "You judge one line of text" "$system" 2>/dev/null; then
  python3 -c 'import json; print(json.dumps({{"is_error": False, "result": "VERDICT: accepted", "session_id": "s-judge", "modelUsage": {{"fake-model": {{}}}}}}))'
  exit 0
fi
{reviewer}
"#
        );
        let path = bin.join("claude");
        std::fs::write(&path, stub).expect("write stub");
        let mode = std::os::unix::fs::PermissionsExt::from_mode(0o755);
        std::fs::set_permissions(&path, mode).expect("chmod");
        git(&self.dir, &["config", "agent-verdict.runner", "claude"]);
    }

    pub fn judge(&self, answer: &str) {
        self.write("judge-answer", answer);
    }

    pub fn read(&self, name: &str) -> String {
        std::fs::read_to_string(self.dir.join(name)).unwrap_or_default()
    }

    pub fn attest_until(&self, intent: &str, rounds: usize) -> Run {
        let mut last = self.attest(intent);
        for _ in 1..rounds {
            if last.code != 0 {
                break;
            }
            last = self.again();
        }
        self.commit()
    }

    pub fn committed(&self) -> bool {
        Command::new("git")
            .current_dir(&self.dir)
            .args(["rev-parse", "--verify", "HEAD"])
            .output()
            .expect("git runs")
            .status
            .success()
    }

    pub fn head_message(&self) -> String {
        let out = Command::new("git")
            .current_dir(&self.dir)
            .args(["log", "-1", "--format=%B"])
            .output()
            .expect("git runs");
        String::from_utf8_lossy(&out.stdout).into_owned()
    }

    pub fn landed_again(&self, rounds: usize) -> String {
        let mut last = self.again();
        for _ in 1..rounds {
            if last.code != 0 {
                break;
            }
            last = self.again();
        }
        let last = self.commit();
        assert_eq!(last.code, 0, "no commit landed: {}", last.err);
        self.head_message()
    }

    pub fn landed(&self, intent: &str, rounds: usize) -> String {
        let run = self.attest_until(intent, rounds);
        assert!(self.committed(), "no commit landed: {}", run.err);
        self.head_message()
    }

    pub fn issued_token(&self) -> String {
        let dir = self.dir.join(".git/agent-verdict");
        let head = std::fs::read_dir(&dir)
            .expect("diary")
            .flatten()
            .map(|e| e.path())
            .find(|p| p.is_dir())
            .expect("a review recorded");
        let progress = std::fs::read_to_string(head.join("progress")).expect("progress");
        let last = progress.lines().next_back().expect("a step");
        last.split('\t').nth(1).expect("a token").to_string()
    }

    pub fn outside_doc(&self) -> PathBuf {
        let path = self.dir.with_extension("outside.md");
        std::fs::write(&path, "the standard").expect("write");
        path
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
        let _ = std::fs::remove_file(self.dir.with_extension("outside.md"));
        // Everything this repo was given, not only the tree: the home holds transcripts and review logs the tool wrote under it, and a suite that leaves one behind per test fills /tmp with them.
        let _ = std::fs::remove_dir_all(self.home());
        let _ = std::fs::remove_dir_all(self.dir.with_extension("outside"));
    }
}

// A pipe a stub opens when it reaches the moment a test is about to act on. Opening it blocks on the other side, so the two meet rather than one of them guessing when the other arrived.
pub fn pipe(at: &std::path::Path) -> std::path::PathBuf {
    let _ = std::fs::remove_file(at);
    let made = std::process::Command::new("mkfifo")
        .arg(at)
        .status()
        .expect("mkfifo runs");
    assert!(made.success(), "could not make {}", at.display());
    at.to_path_buf()
}

// The timeout is on the channel, not the read: a run that dies before opening the pipe must fail, not hang.
pub fn arrived_at(pipe: &std::path::Path) -> Option<String> {
    let (told, arrival) = std::sync::mpsc::channel();
    let path = pipe.to_path_buf();
    std::thread::spawn(move || {
        let _ = told.send(std::fs::read_to_string(path).ok());
    });
    arrival
        .recv_timeout(std::time::Duration::from_secs(30))
        .ok()
        .flatten()
        .map(|said| said.trim().to_string())
}
