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

// A passing verdict closes its gate for this HEAD, so the edit satisfying a MODERATE is never recounted: a bound the reviewer enforces can break under the fix and land in a trailer claiming review.
#[test]
fn content_moving_after_its_verdict_is_named_before_the_commit() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    repo.write("src.rs", "code, and the line the fix added");
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert!(repo.committed(), "{}", run.err);
    assert!(
        run.err.contains("CONTENT MOVED SINCE ITS VERDICT"),
        "{}",
        run.err
    );
    assert!(run.err.contains("standards"), "{}", run.err);
}

#[test]
fn content_left_alone_after_its_verdict_says_nothing() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest_until(AIM, 3);
    assert!(repo.committed(), "{}", run.err);
    assert!(!run.err.contains("CONTENT MOVED"), "{}", run.err);
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

// Enumerating the hook must not fire its guards: under `set -e` the refusal kills the hook, and every gate below it leaves the listing — the guard reported as a hook that declares nothing.
#[test]
fn a_staged_rubric_still_lets_attest_read_the_hook() {
    let repo = Repo::new();
    let guard = r#"--rubric-guard --doc rubric.md"#;
    repo.declare(CLEAN, &[guard, STANDARDS]);
    repo.stage(&["src.rs", "rubric.md"]);
    let run = repo.attest(AIM);
    assert!(!run.err.contains("declared no gates"), "{}", run.err);
    assert!(run.err.contains("RUBRIC IS STAGED"), "{}", run.err);
    assert!(!repo.committed(), "a staged rubric committed anyway");
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

// The two shapes meet in one message, and the hook the commit fires reads both back: a graded trailer and an advisory one are the same commit's evidence.
#[test]
fn a_graded_and_an_advisory_gate_land_their_own_shapes_in_one_commit() {
    let repo = Repo::new();
    let per_gate = r#"case "$(cat)" in *"gate: prose"*) printf 'VERDICT: reviewer=fake session=s-1 findings=3\n';; *) printf 'VERDICT: reviewer=fake session=s-2 major=0 moderate=1 minor=2\n';; esac"#;
    repo.declare_runner(per_gate, &[STANDARDS, PROSE]);
    repo.stage(&["src.rs"]);
    let message = repo.landed(AIM, 3);
    let graded = "Reviewed-standards: reviewer=fake major=0 moderate=1 minor=2";
    assert!(message.contains(graded), "{message}");
    assert!(
        message.contains("Reviewed-prose: reviewer=fake findings=3"),
        "{message}"
    );
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
