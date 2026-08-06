// Concern: a throwaway git repo, and the ways of running the binary against it | Non-concern: what any outcome should be — assertions live in the tests | IO: (files, argv) -> status, output

// Shared by two test binaries, each of which uses part of it: unused-in-this-crate is the normal state here, not a finding.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

pub const BIN: &str = env!("CARGO_BIN_EXE_git-agent-verdict");
static SEQ: AtomicU32 = AtomicU32::new(0);

pub const CLEAN: &str =
    "subject\n\nbody\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=2\n";

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

    pub fn run(&self, msg: &str, args: &[&str]) -> (i32, String) {
        self.write("MSG", msg);
        let out = Command::new(BIN)
            .current_dir(&self.dir)
            .arg("MSG")
            .args(args)
            .output()
            .expect("binary runs");
        let text = String::from_utf8_lossy(&out.stderr).into_owned();
        (out.status.code().expect("exited"), text)
    }

    pub fn standards(&self, msg: &str) -> (i32, String) {
        self.run(msg, &["standards", "--doc", "rubric.md", "--path", "."])
    }

    // The preflight takes no message file, so it cannot go through run().
    pub fn bare(&self, args: &[&str]) -> (i32, String) {
        let out = Command::new(BIN)
            .current_dir(&self.dir)
            .args(args)
            .output()
            .expect("binary runs");
        let text = String::from_utf8_lossy(&out.stderr).into_owned();
        (out.status.code().expect("exited"), text)
    }

    // One gate declaration per line, run through the binary by absolute path: a name resolves from PATH, which passes on a box with the tool installed and fails in CI.
    pub fn hook(&self, gates: &[&str]) {
        let body: String = gates
            .iter()
            .map(|g| format!("{BIN} \"$1\" {g}\n"))
            .collect();
        let hooks = self.dir.join("hooks");
        std::fs::create_dir_all(&hooks).expect("hooks dir");
        let path = hooks.join("commit-msg");
        std::fs::write(&path, format!("#!/bin/sh\n{body}")).expect("place hook");
        let mode = std::os::unix::fs::PermissionsExt::from_mode(0o755);
        std::fs::set_permissions(&path, mode).expect("chmod");
        git(&self.dir, &["config", "core.hooksPath", "hooks"]);
    }

    // The one mode that answers on stdout, so it is the one that reports all three.
    pub fn reviewer_prompt(&self, gate: &str) -> (i32, String, String) {
        let out = Command::new(BIN)
            .current_dir(&self.dir)
            .args(["--reviewer-prompt", gate])
            .output()
            .expect("binary runs");
        (
            out.status.code().expect("exited"),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )
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
