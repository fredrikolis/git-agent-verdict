// Concern: the gate's observable contract — exit status and decision against a real repo | Non-concern: the trailer grammar, unit-tested in src/trailer.rs | IO: (temp repo, message) -> exit status

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};

const BIN: &str = env!("CARGO_BIN_EXE_git-agent-verdict");
static SEQ: AtomicU32 = AtomicU32::new(0);

struct Repo {
    dir: PathBuf,
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git runs");
    assert!(out.status.success(), "git {args:?}: {out:?}");
}

impl Repo {
    fn new() -> Self {
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

    fn write(&self, name: &str, body: &str) {
        std::fs::write(self.dir.join(name), body).expect("write");
    }

    fn stage(&self, paths: &[&str]) {
        for p in paths {
            git(&self.dir, &["add", p]);
        }
    }

    fn run(&self, msg: &str, args: &[&str]) -> (i32, String) {
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

    fn standards(&self, msg: &str) -> (i32, String) {
        self.run(msg, &["standards", "--doc", "rubric.md", "--path", "."])
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

const CLEAN: &str =
    "subject\n\nbody\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=2\n";

#[test]
fn an_attested_commit_passes() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards(CLEAN);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("attested"), "{out}");
}

#[test]
fn an_unattested_commit_fails_and_prints_the_prompt() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards("subject\n\nbody\n");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("REVIEW GATE FAILED"), "{out}");
    assert!(out.contains("NEUTRAL REVIEW"), "{out}");
}

#[test]
fn a_declared_blocker_fails() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let msg = "subject\n\nReviewed-standards: reviewer=opus major=0 moderate=1 minor=0\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DECLARED BLOCKER"), "{out}");
}

#[test]
fn a_repeated_count_cannot_bury_a_blocker() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let msg = "subject\n\nReviewed-standards: reviewer=opus major=1 major=0 moderate=0 minor=0\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("more than once"), "{out}");
}

#[test]
fn a_trailer_above_the_body_is_named_not_reported_missing() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let msg = "subject\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=0\n\nbody\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("trailing paragraph"), "{out}");
}

#[test]
fn a_gate_with_no_matching_staged_file_is_skipped_and_says_so() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(
        CLEAN,
        &["standards", "--doc", "rubric.md", "--path", "*.py"],
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("skipped"), "{out}");
}

#[test]
fn staging_a_rubric_refuses_the_commit() {
    let repo = Repo::new();
    repo.stage(&["src.rs", "rubric.md"]);
    let (code, out) = repo.standards(CLEAN);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("RUBRIC IS STAGED"), "{out}");
}

#[test]
fn per_file_demands_a_trailer_for_every_staged_file() {
    let repo = Repo::new();
    repo.write("other.rs", "more");
    repo.stage(&["src.rs", "other.rs"]);
    let msg = "subject\n\nReviewed-ann: reviewer=opus major=0 moderate=0 minor=0 file=src.rs\n";
    let args = ["ann", "--per-file", "--doc", "rubric.md", "--path", "."];
    let (code, out) = repo.run(msg, &args);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("other.rs"), "{out}");

    let both =
        format!("{msg}Reviewed-ann: reviewer=opus major=0 moderate=0 minor=0 file=other.rs\n");
    let (code, out) = repo.run(&both, &args);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn an_auto_generated_subject_carries_no_review() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards("fixup! subject\n");
    assert_eq!(code, 0, "{out}");
}

#[test]
fn a_forged_merge_subject_is_not_exempt() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards("Merge branch 'nope'\n");
    assert_eq!(code, 1, "{out}");
}

#[test]
fn a_misplaced_pathspec_cannot_be_absorbed_into_docs() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(CLEAN, &["standards", "--doc", "rubric.md", "."]);
    assert_eq!(code, 2, "{out}");
}

#[test]
fn a_doc_that_does_not_exist_fails_loudly() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(CLEAN, &["standards", "--doc", "nope.md", "--path", "."]);
    assert_eq!(code, 2, "{out}");
}

#[test]
fn a_literal_path_naming_nothing_tracked_is_a_typo() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(
        CLEAN,
        &["standards", "--doc", "rubric.md", "--path", "NOPE.md"],
    );
    assert_eq!(code, 2, "{out}");
}

#[test]
fn the_prompt_demands_an_intent_and_says_how_to_judge_it() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards("subject\n\nbody\n");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("INTENT:"), "{out}");
    assert!(
        out.contains("Judge that INTENT before anything else"),
        "{out}"
    );
    assert!(out.contains("Scope is not your question"), "{out}");
}

#[test]
fn version_and_help_are_info_flags_that_exit_clean() {
    for (flag, needle) in [("--version", "git-agent-verdict 0."), ("--help", "usage:")] {
        let out = std::process::Command::new(BIN)
            .arg(flag)
            .output()
            .expect("binary runs");
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        assert_eq!(out.status.code(), Some(0), "{flag}: {text}");
        assert!(text.contains(needle), "{flag}: {text}");
    }
}
