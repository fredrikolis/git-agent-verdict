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

    // The preflight takes no message file, so it cannot go through run().
    fn guard(&self, args: &[&str]) -> (i32, String) {
        let out = Command::new(BIN)
            .current_dir(&self.dir)
            .args(args)
            .output()
            .expect("binary runs");
        let text = String::from_utf8_lossy(&out.stderr).into_owned();
        (out.status.code().expect("exited"), text)
    }

    // Written beside the worktree, not in it: such a path can never appear in the index.
    fn outside_doc(&self) -> PathBuf {
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
fn the_preflight_refuses_a_staged_rubric_and_names_it() {
    let repo = Repo::new();
    repo.write("later-rubric.md", "the other standard");
    repo.stage(&["src.rs", "later-rubric.md"]);
    let args = [
        "--rubric-guard",
        "--doc",
        "rubric.md",
        "--doc",
        "later-rubric.md",
    ];
    let (code, out) = repo.guard(&args);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("RUBRIC IS STAGED"), "{out}");
    assert!(out.contains("later-rubric.md"), "{out}");
    assert!(!out.contains("\n  rubric.md"), "{out}");
    assert!(out.contains("--no-verify"), "{out}");
}

#[test]
fn the_preflight_passes_silently_when_no_rubric_is_staged() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.guard(&["--rubric-guard", "--doc", "rubric.md"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.is_empty(), "{out}");
}

#[test]
fn the_preflight_is_a_no_op_for_a_doc_outside_the_worktree() {
    let repo = Repo::new();
    repo.stage(&["src.rs", "rubric.md"]);
    let outside = repo.outside_doc();
    let (code, out) = repo.guard(&["--rubric-guard", "--doc", outside.to_str().expect("utf-8")]);
    assert_eq!(code, 0, "{out}");
    assert!(out.is_empty(), "{out}");
}

#[test]
fn the_preflight_rejects_arguments_its_mode_cannot_use() {
    let repo = Repo::new();
    repo.stage(&["src.rs", "rubric.md"]);
    let bad: [&[&str]; 4] = [
        &["--rubric-guard"],
        &["--rubric-guard", "--doc", "rubric.md", "--path", "."],
        &["--rubric-guard", "--doc", "rubric.md", "--per-file"],
        &["--rubric-guard", "MSG", "standards", "--doc", "rubric.md"],
    ];
    for args in bad {
        let (code, out) = repo.guard(args);
        assert_eq!(code, 2, "{args:?}: {out}");
        assert!(out.contains("usage:"), "{args:?}: {out}");
    }
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

#[test]
fn an_agent_coauthor_line_is_dropped_but_a_human_one_is_kept() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let msg = "subject\n\nbody\n\n\
        Reviewed-standards: reviewer=opus major=0 moderate=0 minor=0\n\
        Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\n\
        Co-authored-by: Claude Bernard <claude@example.com>\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 0, "{out}");
    let rewritten = std::fs::read_to_string(repo.dir.join("MSG")).expect("read back");
    assert!(!rewritten.contains("anthropic.com"), "{rewritten}");
    assert!(rewritten.contains("claude@example.com"), "{rewritten}");
    assert!(rewritten.contains("Reviewed-standards:"), "{rewritten}");
}

#[test]
fn reviewer_prompt_reads_its_docs_from_the_hook() {
    let repo = Repo::new();
    // By absolute path, not by name: a name resolves from PATH, which passes on a box with the tool installed and fails in CI, where the only build is the one cargo just made.
    repo.write(
        "hook",
        &format!("#!/bin/sh\nexec {BIN} \"$1\" standards --doc rubric.md --path .\n"),
    );
    let hooks = repo.dir.join("hooks");
    std::fs::create_dir_all(&hooks).expect("hooks dir");
    std::fs::rename(repo.dir.join("hook"), hooks.join("commit-msg")).expect("place hook");
    git(&repo.dir, &["config", "core.hooksPath", "hooks"]);
    let mode = std::os::unix::fs::PermissionsExt::from_mode(0o755);
    std::fs::set_permissions(hooks.join("commit-msg"), mode).expect("chmod");

    let out = std::process::Command::new(BIN)
        .current_dir(&repo.dir)
        .args(["--reviewer-prompt", "standards"])
        .output()
        .expect("binary runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(0), "{err}");
    assert!(text.contains("NEUTRAL REVIEW — gate: standards"), "{text}");
    assert!(text.contains("INTENT:"), "{text}");
    assert!(text.contains("rubric.md"), "{text}");

    let out = std::process::Command::new(BIN)
        .current_dir(&repo.dir)
        .args(["--reviewer-prompt", "nope"])
        .output()
        .expect("binary runs");
    let err = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(2), "{err}");
    assert!(err.contains("it declares: standards"), "{err}");
}

#[test]
fn reviewer_prompt_refuses_the_gate_mode_flags() {
    let repo = Repo::new();
    for extra in [
        vec!["--path", "."],
        vec!["--per-file"],
        vec!["--doc", "rubric.md"],
        vec!["MSG"],
    ] {
        let mut args = vec!["--reviewer-prompt", "standards"];
        args.extend(extra.iter());
        let out = std::process::Command::new(BIN)
            .current_dir(&repo.dir)
            .args(&args)
            .output()
            .expect("binary runs");
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
}
