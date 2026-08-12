// Concern: the review attest runs — the gate it takes, what it records, the commit it lands | Non-concern: what the gate refuses (tests/gate.rs) | IO: (temp repo, hook) -> status, commit

mod common;

use common::{Repo, PROSE, STANDARDS};

const CLEAN: &str = "VERDICT: reviewer=fake session=s-01 major=0 moderate=1 minor=2";
const BLOCKER: &str = "VERDICT: reviewer=fake session=s-02 major=1 moderate=0 minor=0";
const AIM: &str = "raise the staged file's line count";

// Scoping is the pathspec plus the index, and the brief says so: the reviewer is told which files are in and that there are no others.
#[test]
fn the_brief_names_the_staged_files_in_scope() {
    let repo = Repo::new();
    repo.write("other.rs", "more");
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["--reviewer-prompt", "standards"]);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.out.contains("files under review"), "{}", run.out);
    assert!(run.out.contains("src.rs"), "{}", run.out);
    assert!(!run.out.contains("other.rs"), "{}", run.out);
}

#[test]
fn the_trailer_carries_the_counts_the_reviewer_reported() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    assert!(message.contains("Reviewed-standards:"), "{message}");
    assert!(message.contains("major=0 moderate=1 minor=2"), "{message}");
    assert!(message.contains("reviewer=fake"), "{message}");
}

// The subject is the brief, verbatim: the one line both the reviewer and the record need.
#[test]
fn the_intent_becomes_the_subject() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    assert_eq!(message.lines().next(), Some(AIM), "{message}");
}

#[test]
fn a_major_holds_the_gate_and_commits_nothing() {
    let repo = Repo::new();
    repo.declare(BLOCKER, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 1, "{}", run.err);
    assert!(run.err.contains("MAJOR"), "{}", run.err);
    assert!(!repo.committed(), "a blocked gate committed anyway");
}

// The same gate again, not the next one: a blocked gate is the only one that is ever reviewed twice.
#[test]
fn a_blocked_gate_is_the_one_that_runs_again() {
    let repo = Repo::new();
    repo.declare(BLOCKER, &[STANDARDS, PROSE]);
    repo.stage(&["src.rs"]);
    for _ in 0..2 {
        let run = repo.attest(AIM);
        assert!(run.err.contains("standards:"), "{}", run.err);
        assert!(!run.err.contains("prose:"), "{}", run.err);
    }
}

#[test]
fn gates_are_reviewed_in_the_order_the_hook_declares_them() {
    let repo = Repo::new();
    let second_gate = r#""$1" ann --doc rubric.md --path ."#;
    repo.declare(CLEAN, &[STANDARDS, second_gate]);
    repo.stage(&["src.rs"]);
    let first = repo.attest(AIM);
    assert!(first.err.contains("standards:"), "{}", first.err);
    let second = repo.attest(AIM);
    assert!(second.err.contains("ann:"), "{}", second.err);
    let message = repo.landed(AIM, 2);
    assert!(message.contains("Reviewed-standards:"), "{message}");
    assert!(message.contains("Reviewed-ann:"), "{message}");
}

#[test]
fn an_advisory_gate_counts_findings_and_never_grades() {
    let repo = Repo::new();
    repo.declare("VERDICT: reviewer=fake session=s-03 findings=4", &[PROSE]);
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    assert!(message.contains("Reviewed-prose:"), "{message}");
    assert!(message.contains("findings=4"), "{message}");
    assert!(!message.contains("major="), "{message}");
}

// The brief is the one input the author still writes, so it may not drift between the gates of one commit.
#[test]
fn the_intent_cannot_change_between_gates() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS, PROSE]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    let run = repo.attest("something else entirely");
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("only change after a MAJOR"), "{}", run.err);
}

// The limit bounds the change, not the prose: an aim needing more than a line is more than one commit.
#[test]
fn an_intent_naming_more_than_one_change_is_refused_with_the_remedy() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let two_changes = "x".repeat(301);
    let run = repo.capture(&["attest", "--intent", &two_changes]);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("commit them separately"), "{}", run.err);
}

// The guard the cap cannot enforce: a brief well under the limit can still argue, and the reviewer is what catches it.
#[test]
fn a_reviewer_that_refuses_the_brief_records_nothing() {
    let repo = Repo::new();
    repo.declare("VERDICT: refused", &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest("add the cache because the old path was far too slow");
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("refused the brief"), "{}", run.err);
    assert!(!repo.committed(), "a refused brief committed anyway");
}

// The hole the old guard had: an advisory gate could not block on a refusal, so a contaminated brief passed unnoticed.
#[test]
fn an_advisory_gate_refuses_a_contaminated_brief_too() {
    let repo = Repo::new();
    repo.declare("VERDICT: refused", &[PROSE]);
    repo.stage(&["src.rs"]);
    let run = repo.attest("add the cache because the old path was far too slow");
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("refused the brief"), "{}", run.err);
}

// The brief states the line it will be read by, so a field it omits is a broken contract, not a value to invent.
#[test]
fn a_verdict_line_missing_a_required_field_is_refused() {
    for missing in [
        "VERDICT: session=s-1 major=0 moderate=0 minor=0",
        "VERDICT: reviewer=fake major=0 moderate=0 minor=0",
    ] {
        let repo = Repo::new();
        repo.declare(missing, &[STANDARDS]);
        repo.stage(&["src.rs"]);
        let run = repo.attest(AIM);
        assert_eq!(run.code, 2, "{missing}: {}", run.err);
        assert!(run.err.contains("carries no"), "{missing}: {}", run.err);
    }
}

#[test]
fn a_reviewer_that_reports_no_verdict_line_is_an_error_not_a_pass() {
    let repo = Repo::new();
    repo.declare("no verdict here", &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("VERDICT"), "{}", run.err);
}

#[test]
fn a_host_with_no_reviewer_configured_says_so() {
    let repo = Repo::new();
    repo.hook(&[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("agent-verdict.runner"), "{}", run.err);
}

#[test]
fn a_reset_is_counted_and_keeps_its_reason() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    let run = repo.capture(&["reset", "the", "brief", "named", "the", "wrong", "aim"]);
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
    let run = repo.capture(&["reset"]);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("needs a reason"), "{}", run.err);
}
