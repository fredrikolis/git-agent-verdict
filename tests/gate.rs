// Concern: the gate's decision against a real repo — what passes, what is refused, what exits 2 | Non-concern: the reviewer block it prints (tests/brief.rs) | IO: (temp repo, message) -> exit status

mod common;

use common::{Repo, BIN, CLEAN};

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
    let msg = "subject\n\nReviewed-standards: reviewer=opus major=1 moderate=0 minor=0\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DECLARED BLOCKER"), "{out}");
}

// The count records what the reviewer found. The MODERATEs were fixed without a second look, so what survives into the trailer is a record, not an outstanding defect.
#[test]
fn a_moderate_count_passes_and_is_reported() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let msg = "subject\n\nReviewed-standards: reviewer=opus major=0 moderate=2 minor=1\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("moderate=2"), "{out}");
}

#[test]
fn a_simple_gate_records_findings_without_blocking_on_them() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let args = ["look", "--simple", "--doc", "rubric.md", "--path", "."];
    let msg = "subject\n\nReviewed-look: reviewer=opus major=3 moderate=2 minor=1\n";
    let (code, out) = repo.run(msg, &args);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("major=3"), "{out}");

    // Advisory about the findings, not about the review: the trailer itself is still demanded.
    let (code, out) = repo.run("subject\n\nbody\n", &args);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("REVIEW GATE FAILED"), "{out}");
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
    let (code, out) = repo.bare(&args);
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
    let (code, out) = repo.bare(&["--rubric-guard", "--doc", "rubric.md"]);
    assert_eq!(code, 0, "{out}");
    assert!(out.is_empty(), "{out}");
}

#[test]
fn the_preflight_is_a_no_op_for_a_doc_outside_the_worktree() {
    let repo = Repo::new();
    repo.stage(&["src.rs", "rubric.md"]);
    let outside = repo.outside_doc();
    let (code, out) = repo.bare(&["--rubric-guard", "--doc", outside.to_str().expect("utf-8")]);
    assert_eq!(code, 0, "{out}");
    assert!(out.is_empty(), "{out}");
}

#[test]
fn the_preflight_rejects_arguments_its_mode_cannot_use() {
    let repo = Repo::new();
    repo.stage(&["src.rs", "rubric.md"]);
    let bad: [&[&str]; 6] = [
        &["--rubric-guard"],
        &["--rubric-guard", "--doc", "rubric.md", "--path", "."],
        &["--rubric-guard", "--doc", "rubric.md", "--per-file"],
        &["--rubric-guard", "--doc", "rubric.md", "--simple"],
        &[
            "--rubric-guard",
            "--doc",
            "rubric.md",
            "--override-prompt",
            "rubric.md",
        ],
        &["--rubric-guard", "MSG", "standards", "--doc", "rubric.md"],
    ];
    for args in bad {
        let (code, out) = repo.bare(args);
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

// A floor: the release above it passes, which is the whole difference from pinning an equality.
#[test]
fn the_version_floor_passes_at_or_below_the_installed_version() {
    let installed = env!("CARGO_PKG_VERSION");
    for want in [installed, "0.0.1", "0.1"] {
        let out = std::process::Command::new(BIN)
            .args(["--check-min-version", want])
            .output()
            .expect("binary runs");
        assert_eq!(out.status.code(), Some(0), "{want}: {out:?}");
    }
}

#[test]
fn a_floor_above_the_installed_version_fails_and_names_the_remedy() {
    let out = std::process::Command::new(BIN)
        .args(["--check-min-version", "99.0.0"])
        .output()
        .expect("binary runs");
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("cargo install git-agent-verdict"), "{text}");
}

#[test]
fn a_floor_that_is_not_a_version_is_a_usage_error() {
    for want in ["v0.3", "latest"] {
        let out = std::process::Command::new(BIN)
            .args(["--check-min-version", want])
            .output()
            .expect("binary runs");
        assert_eq!(out.status.code(), Some(2), "{want}: {out:?}");
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
