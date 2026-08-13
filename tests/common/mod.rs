// Concern: a throwaway git repo, a declared hook, and ways to run the binary | Non-concern: what any outcome should be | IO: (files, argv) -> status, output

// Shared by two test binaries, each of which uses part of it: unused-in-this-crate is the normal state here, not a finding.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

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
        repo.write("rubric.md", "the standard");
        repo.write("src.rs", "code");
        repo
    }

    pub fn write(&self, name: &str, body: &str) {
        std::fs::write(self.dir.join(name), body).expect("write");
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

    // Where the caller stands is not where a hook's paths are written from: an agent runs attest from wherever it happens to be.
    pub fn capture_in(&self, subdir: &str, args: &[&str]) -> Run {
        let out = Command::new(BIN)
            .current_dir(self.dir.join(subdir))
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .args(args)
            .output()
            .expect("binary runs");
        Run {
            code: out.status.code().expect("exited"),
            out: String::from_utf8_lossy(&out.stdout).into_owned(),
            err: String::from_utf8_lossy(&out.stderr).into_owned(),
        }
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

    // The preflight takes no message file, so it cannot go through run().
    pub fn bare(&self, args: &[&str]) -> (i32, String) {
        let run = self.capture(args);
        (run.code, run.err)
    }

    pub fn attest(&self, intent: &str) -> Run {
        self.capture(&["attest", "--intent", intent])
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

    // The one mode that answers on stdout, so it is the one that reports all three.
    pub fn reviewer_prompt(&self, gate: &str) -> (i32, String, String) {
        let run = self.capture(&["--reviewer-prompt", gate]);
        (run.code, run.out, run.err)
    }

    // The reviewer is host configuration, set per clone here: a repo that declared one would pick an agent for every maintainer.
    pub fn declare(&self, verdict: &str, gates: &[&str]) {
        let cmd = format!("printf '{verdict}\\n'");
        self.declare_runner(&cmd, gates);
    }

    // The brief names its gate, so a runner that reads stdin can answer each one in its own shape.
    pub fn declare_runner(&self, cmd: &str, gates: &[&str]) {
        self.hook(gates);
        git(&self.dir, &["config", "agent-verdict.runner", cmd]);
    }

    // Run until it stops complaining, which is the whole protocol: the last run has no gate left and commits.
    pub fn attest_until(&self, intent: &str, rounds: usize) -> Run {
        let mut last = self.attest(intent);
        for _ in 1..rounds {
            if self.committed() {
                break;
            }
            last = self.attest(intent);
        }
        last
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

    // The whole protocol in one call: review every gate, then commit, and answer with the message that landed.
    pub fn landed(&self, intent: &str, rounds: usize) -> String {
        let run = self.attest_until(intent, rounds);
        assert!(self.committed(), "no commit landed: {}", run.err);
        self.head_message()
    }

    // Read the way an author can read it. Coupled to the diary's layout on purpose: pasting a blocked review's own token is the forgery the gate has to survive.
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

    // Written beside the worktree, not in it: such a path can never appear in the index.
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
    }
}
