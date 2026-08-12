// Concern: the gate's decision against a real repo — what passes, what is refused, what exits 2 | Non-concern: the review that earns a trailer (tests/attest.rs) | IO: (temp repo, message) -> exit status

mod common;

use common::{Repo, BIN, DUMMY, STANDARDS};

#[test]
fn an_unattested_commit_fails_and_names_the_remedy() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards("subject\n\nbody\n");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("REVIEW GATE FAILED"), "{out}");
    assert!(out.contains("attest --intent"), "{out}");
}

// A hand-written trailer is well-formed and names nothing: the counts in a message are worth only as much as the review they can be traced to.
#[test]
fn a_trailer_whose_token_names_no_review_is_refused() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards(DUMMY);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("UNKNOWN TOKEN"), "{out}");
}

#[test]
fn an_edited_count_is_caught_against_the_recorded_review() {
    let repo = Repo::new();
    repo.declare(
        "VERDICT: reviewer=fake session=s-09 major=1 moderate=0 minor=0",
        &[STANDARDS],
    );
    repo.stage(&["src.rs"]);
    repo.attest("raise the staged file's line count");
    let token = repo.issued_token();

    let clean = format!(
        "subject\n\nReviewed-standards: reviewer=fake major=0 moderate=0 minor=0 token={token}\n"
    );
    let (code, out) = repo.standards(&clean);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("CONTRADICTS"), "{out}");
    assert!(out.contains("major=1"), "{out}");
}

#[test]
fn a_declared_blocker_fails_even_when_it_is_traceable() {
    let repo = Repo::new();
    repo.declare(
        "VERDICT: reviewer=fake session=s-09 major=1 moderate=0 minor=0",
        &[STANDARDS],
    );
    repo.stage(&["src.rs"]);
    repo.attest("raise the staged file's line count");
    let token = repo.issued_token();

    let honest = format!(
        "subject\n\nReviewed-standards: reviewer=fake major=1 moderate=0 minor=0 token={token}\n"
    );
    let (code, out) = repo.standards(&honest);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("DECLARED BLOCKER"), "{out}");
}

#[test]
fn a_trailer_above_the_body_is_named_not_reported_missing() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let msg = "subject\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=0 token=ab\n\nbody\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("trailing paragraph"), "{out}");
}

#[test]
fn a_trailer_with_no_token_is_malformed() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let msg = "subject\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=0\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("MALFORMED"), "{out}");
}

#[test]
fn a_gate_with_no_matching_staged_file_is_skipped_and_says_so() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(
        DUMMY,
        &["standards", "--doc", "rubric.md", "--path", "*.py"],
    );
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("skipped"), "{out}");
}

#[test]
fn staging_a_rubric_refuses_the_commit() {
    let repo = Repo::new();
    repo.stage(&["src.rs", "rubric.md"]);
    let (code, out) = repo.standards(DUMMY);
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
    let bad: [&[&str]; 5] = [
        &["--rubric-guard"],
        &["--rubric-guard", "--doc", "rubric.md", "--path", "."],
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
    let (code, out) = repo.run(DUMMY, &["standards", "--doc", "rubric.md", "."]);
    assert_eq!(code, 2, "{out}");
}

#[test]
fn a_doc_that_does_not_exist_fails_loudly() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(DUMMY, &["standards", "--doc", "nope.md", "--path", "."]);
    assert_eq!(code, 2, "{out}");
}

#[test]
fn a_literal_path_naming_nothing_tracked_is_a_typo() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let args = ["standards", "--doc", "rubric.md", "--path", "NOPE.md"];
    let (code, out) = repo.run(DUMMY, &args);
    assert_eq!(code, 2, "{out}");
}

#[test]
fn version_and_help_are_info_flags_that_exit_clean() {
    let version = format!("git-agent-verdict {}", env!("CARGO_PKG_VERSION"));
    for (flag, needle) in [("--version", version.as_str()), ("--help", "usage:")] {
        let out = std::process::Command::new(BIN)
            .arg(flag)
            .output()
            .expect("binary runs");
        let text = String::from_utf8_lossy(&out.stdout).into_owned();
        assert_eq!(out.status.code(), Some(0), "{flag}: {text}");
        assert!(text.contains(needle), "{flag}: {text}");
    }
}

// A pin, not a floor: 0.4 is its own compatibility line, and a hook that declares its gates against one release must not be answered by another.
#[test]
fn the_installed_line_satisfies_a_pin_on_that_line() {
    let installed = env!("CARGO_PKG_VERSION");
    let (major, minor) = installed.split_once('.').expect("a dotted version");
    for want in [
        installed.to_string(),
        format!("{major}.{minor}"),
        format!("{major}.{minor}.0"),
    ] {
        let out = std::process::Command::new(BIN)
            .args(["--require-version", &want])
            .output()
            .expect("binary runs");
        assert_eq!(out.status.code(), Some(0), "{want}: {out:?}");
    }
}

// The failure the old floor could not see: a release that took a flag away passed a hook pinned below it, and the hook found out when a commit died.
#[test]
fn a_pin_on_another_line_is_refused_in_both_directions() {
    for want in ["0.3.0", "0.1", "2.0.0", "99.0.0"] {
        let out = std::process::Command::new(BIN)
            .args(["--require-version", want])
            .output()
            .expect("binary runs");
        let text = String::from_utf8_lossy(&out.stderr).into_owned();
        assert_eq!(out.status.code(), Some(1), "{want}: {text}");
        assert!(
            text.contains("cargo install git-agent-verdict"),
            "{want}: {text}"
        );
    }
}

#[test]
fn a_later_patch_on_the_same_line_is_too_old_not_incompatible() {
    let installed = env!("CARGO_PKG_VERSION");
    let (major, rest) = installed.split_once('.').expect("a dotted version");
    let minor = rest.split('.').next().expect("a minor field");
    let out = std::process::Command::new(BIN)
        .args(["--require-version", &format!("{major}.{minor}.99")])
        .output()
        .expect("binary runs");
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    assert_eq!(out.status.code(), Some(1), "{text}");
    assert!(text.contains("is older than"), "{text}");
}

#[test]
fn a_pin_that_is_not_a_version_is_a_usage_error() {
    for want in ["v0.3", "latest"] {
        let out = std::process::Command::new(BIN)
            .args(["--require-version", want])
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
        Reviewed-standards: reviewer=opus major=0 moderate=0 minor=0 token=ab\n\
        Co-Authored-By: Claude Opus 5 (1M context) <noreply@anthropic.com>\n\
        Co-authored-by: Claude Bernard <claude@example.com>\n";
    repo.standards(msg);
    let rewritten = std::fs::read_to_string(repo.dir.join("MSG")).expect("read back");
    assert!(!rewritten.contains("anthropic.com"), "{rewritten}");
    assert!(rewritten.contains("claude@example.com"), "{rewritten}");
    assert!(rewritten.contains("Reviewed-standards:"), "{rewritten}");
}
