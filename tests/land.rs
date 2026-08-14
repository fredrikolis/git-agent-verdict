// Concern: the commit attest makes — its message, the trailers it carries, and what it says landed unread | Non-concern: the reviews that earned them (tests/attest.rs) | IO: (temp repo, hook) -> commit

mod common;

use common::{Repo, PROSE, STANDARDS};

const CLEAN: &str = "VERDICT: reviewer=fake session=s-01 major=0 moderate=1 minor=2";
const AIM: &str = "raise the staged file's line count";

// The subject is the brief, verbatim: the one line both the reviewer and the record need.
#[test]
fn the_intent_becomes_the_subject() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    assert_eq!(message.lines().next(), Some(AIM), "{message}");
}

// A staged path no gate reads is the maintainer's declaration, not a hole the committer can act on — and a mechanical pre-commit gate this tool cannot see may cover it more strictly than a review would. The board says which gates ran; nothing warns about the ones that did not.
#[test]
fn a_path_no_gate_reaches_lands_without_comment() {
    let repo = Repo::new();
    repo.write("notes.txt", "loose");
    let scoped = r#""$1" standards --doc rubric.md --path "*.rs""#;
    repo.declare(CLEAN, &[scoped]);
    repo.stage(&["src.rs", "notes.txt"]);
    let run = repo.attest_until(AIM, 3);
    assert!(repo.committed(), "{}", run.err);
    assert!(!run.err.contains("unreviewed"), "{}", run.err);
}

// stdout is the channel an agent parses, and git's own output is the only other thing on it: a landed commit it has to infer is one it may make twice.
#[test]
fn the_landed_commit_is_announced_on_stdout() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest_until(AIM, 3);
    assert!(repo.committed(), "{}", run.err);
    assert!(run.out.contains("committed "), "{}", run.out);
    assert!(run.out.contains("nothing left to run"), "{}", run.out);
}

// The diary is keyed on HEAD, which the commit moved, so a second run finds no step and nothing staged — which is not the hook failing to declare a gate.
#[test]
fn attest_after_the_commit_landed_names_the_empty_index() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest_until(AIM, 3);
    assert!(repo.committed());
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("nothing staged"), "{}", run.err);
}

// A graded gate and an advisory one in the same hook: both land a trailer, and the hook the commit fires reads both back.
#[test]
fn a_graded_and_an_advisory_gate_both_land_a_trailer() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS, PROSE]);
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    assert!(message.contains("Reviewed-standards:"), "{message}");
    assert!(message.contains("Reviewed-prose:"), "{message}");
}

// The counts say how much; only the report says what. An author told to address a finding it cannot read has been told nothing.
#[test]
fn what_the_reviewer_said_reaches_the_author() {
    let repo = Repo::new();
    let spoken = format!("MODERATE - the lede repeats the heading\\n{CLEAN}");
    repo.declare(&spoken, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    let path = run
        .err
        .lines()
        .find_map(|l| l.strip_prefix("see the full report: "))
        .expect("a path")
        .trim()
        .to_string();
    let body = std::fs::read_to_string(&path).expect("the report");
    assert!(body.contains("the lede repeats the heading"), "{body}");
    let _ = std::fs::remove_file(&path);
}

#[test]
fn a_reset_is_counted_and_keeps_its_reason() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    let root = repo.root();
    let run = repo.capture(&[
        "reset", "--repo", &root, "the", "brief", "named", "the", "wrong", "aim",
    ]);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.err.contains("reset 1"), "{}", run.err);

    let message = repo.landed("a different aim entirely", 3);
    assert!(message.contains("resets=1"), "{message}");
    assert!(message.contains("Reset: the brief named"), "{message}");
}

#[test]
fn a_reset_without_a_reason_is_refused() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    let root = repo.root();
    let run = repo.capture(&["reset", "--repo", &root]);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("needs a reason"), "{}", run.err);
}

// A full review runs to hundreds of lines; an author reading the tail of the output would miss the findings above it, so the whole report is on disk and named.
#[test]
fn a_long_report_is_written_where_it_can_be_read_whole() {
    let repo = Repo::new();
    let long: String = (1..=60)
        .map(|n| format!("MINOR - finding {n}\\n"))
        .collect();
    repo.declare(&format!("{long}{CLEAN}"), &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert!(run.out.contains("standards: major=0"), "{}", run.out);
    assert!(!run.err.contains("finding 42"), "{}", run.err);
    let path = run
        .err
        .lines()
        .find_map(|l| l.strip_prefix("see the full report: "))
        .expect("a path")
        .trim()
        .to_string();
    let body = std::fs::read_to_string(&path).expect("the report");
    assert!(body.contains("finding 42"), "{body}");
    assert!(body.contains("finding 60"), "{body}");
    let _ = std::fs::remove_file(&path);
}
