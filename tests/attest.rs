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
fn an_advisory_gate_grades_on_the_same_ladder_and_never_blocks() {
    let repo = Repo::new();
    repo.declare(
        "VERDICT: reviewer=fake session=s-03 major=0 moderate=1 minor=3",
        &[PROSE],
    );
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    assert!(message.contains("Reviewed-prose:"), "{message}");
    assert!(message.contains("major=0 moderate=1 minor=3"), "{message}");
}

// major= is the count that reaches zero, and an advisory gate has no MAJOR rung: a reviewer reporting one answered a brief it was not given.
#[test]
fn an_advisory_reviewer_reporting_a_major_is_refused() {
    let repo = Repo::new();
    repo.declare(BLOCKER, &[PROSE]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("no MAJOR rung"), "{}", run.err);
    assert!(!repo.committed(), "an advisory major committed anyway");
}

// A verdict describes the content its reviewer saw. Fixing what it named moves that content — that is the work — so the gate opens again rather than landing a trailer about text that no longer exists.
#[test]
fn content_moving_after_its_verdict_reopens_the_gate() {
    let repo = Repo::new();
    let counting = r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; echo "VERDICT: reviewer=fake session=s-$n major=0 moderate=$n minor=0""#;
    repo.declare_runner(counting, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    repo.write("src.rs", "the line the fix added");
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    assert_eq!(repo.read("rounds").trim(), "2", "{message}");
    assert!(message.contains("moderate=2"), "{message}");
    assert!(!message.contains("moderate=1"), "{message}");
}

#[test]
fn a_verdict_stands_while_its_content_does_not_move() {
    let repo = Repo::new();
    let counting = r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; echo "VERDICT: reviewer=fake session=s-$n major=0 moderate=1 minor=0""#;
    repo.declare_runner(counting, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    assert_eq!(repo.read("rounds").trim(), "1", "{message}");
}

// A fresh reviewer resamples a rubric it has never seen, which is how counts wander between rounds that fixed nothing. The runner decides whether to resume; the tool only hands it the session.
#[test]
fn a_re_review_is_handed_the_session_of_the_last_one() {
    let repo = Repo::new();
    let recording = r#"echo "[$AGENT_VERDICT_PRIOR_SESSION]" >> handed; echo "VERDICT: reviewer=fake session=s-99 major=0 moderate=0 minor=0""#;
    repo.declare_runner(recording, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    repo.write("src.rs", "moved");
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    assert_eq!(
        repo.read("handed"),
        "[]\n[s-99]\n",
        "{}",
        repo.read("handed")
    );
}

// A pathspec is written against the root because that is where git runs a hook, and one resolved from a subdirectory passes a gate on a fraction of the change without saying so.
#[test]
fn a_gate_reviews_the_same_files_from_any_directory() {
    let repo = Repo::new();
    std::fs::create_dir_all(repo.dir.join("sub")).expect("subdir");
    repo.write("sub/deep.rs", "deep");
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs", "sub/deep.rs"]);
    let from_root = repo.capture(&["--reviewer-prompt", "standards"]);
    let from_sub = repo.capture_in("sub", &["--reviewer-prompt", "standards"]);
    assert_eq!(from_sub.code, 0, "{}", from_sub.err);
    assert_eq!(from_root.out, from_sub.out, "{}", from_sub.out);
    assert!(from_sub.out.contains("src.rs"), "{}", from_sub.out);
}

// Enumerating the hook must not act on it: under `set -e` a mode that refuses during the listing kills the hook, and every gate below it leaves the listing.
#[test]
fn a_staged_rubric_still_lets_attest_read_the_hook() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs", "rubric.md"]);
    let run = repo.attest(AIM);
    assert!(!run.err.contains("declared no gates"), "{}", run.err);
    assert!(run.err.contains("RUBRIC IS STAGED"), "{}", run.err);
    assert!(!repo.committed(), "a mixed rubric commit landed anyway");
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

// Recorded, the second line shares the first's token and lands as a trailer of its own, which the gate reads as contradicting the review it names: a commit the tool makes and its own hook refuses.
#[test]
fn a_reviewer_closing_with_two_verdict_lines_is_refused() {
    let repo = Repo::new();
    let doubled = "VERDICT: reviewer=a session=s-1 major=0 moderate=1 minor=0\\nVERDICT: reviewer=b session=s-2 major=0 moderate=2 minor=0";
    repo.declare(doubled, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("asks for one"), "{}", run.err);
    assert!(!repo.committed(), "a doubled verdict committed anyway");
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
