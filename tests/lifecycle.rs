// Concern: how a run ends and what it leaves behind — the ceiling, a kill, the claim, a round taken up again | Non-concern: what a review records (tests/attest.rs) | IO: (repo, hook) -> status, files

mod common;

use common::{Repo, STANDARDS};

const CLEAN: &str = "VERDICT: reviewer=fake session=s-01 major=0 moderate=1 minor=2";
const BLOCKER: &str = "VERDICT: reviewer=fake session=s-01 major=1 moderate=0 minor=0";
const AIM: &str = "raise the staged file's line count";

#[test]
fn a_reviewer_that_stops_answering_is_killed_at_the_ceiling() {
    let repo = Repo::new();
    repo.declare_agent("echo 'still thinking' >&2; sleep 60", &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest_within(AIM, "2s");
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("without answering"), "{}", run.err);
    assert!(run.err.contains("2s ceiling"), "{}", run.err);
    // Said before the spawn, so it is still there after the kill: which gate was in play, and which session to go and read.
    assert!(run.err.contains("standards: reviewing"), "{}", run.err);
    assert!(run.err.contains("session "), "{}", run.err);
    assert!(run.err.contains("still reviewing"), "{}", run.err);
    assert!(run.err.contains("--timeout"), "{}", run.err);
    assert!(run.err.contains("still thinking"), "{}", run.err);
}

// A process the agent spawned can hold its pipe open after the agent exits; waiting on that pipe would be unbounded.
#[test]
fn an_agent_that_leaves_its_pipe_held_open_does_not_hang_the_run() {
    let repo = Repo::new();
    // Backgrounded by the reviewer itself and left running as it exits, so the holder inherits the pipe and outlives the process that opened it.
    let leaves_a_holder = concat!(
        "sleep 120 &\n",
        r#"python3 -c 'import json; print(json.dumps({"is_error": False, "result": "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0", "session_id": "s-1", "modelUsage": {"fake-model": {}}}))'"#,
        "\nexit 0"
    );
    repo.declare_agent(leaves_a_holder, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let started = std::time::Instant::now();
    let run = repo.attest(AIM);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(60),
        "the run waited on a pipe its agent had already finished with: {:?}",
        started.elapsed()
    );
    assert_eq!(run.code, 0, "{}", run.err);
}

// An agent can crash and still exit 0; stderr is then the only account of the fault.
#[test]
fn a_crash_on_stderr_reaches_the_author_though_the_agent_exited_clean() {
    let repo = Repo::new();
    repo.declare_agent(
        "echo 'UnhandledPromiseRejection: boom' >&2; exit 0",
        &[STANDARDS],
    );
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("UnhandledPromiseRejection"), "{}", run.err);
}

#[test]
fn an_answer_with_no_verdict_names_why_the_reviewer_stopped() {
    let repo = Repo::new();
    let answer = r#"python3 -c 'import json; print(json.dumps({"is_error": False, "result": "I read the diff and then", "stop_reason": "max_tokens", "session_id": "s-cut", "modelUsage": {"fake-model": {}}}))'"#;
    repo.declare_agent(answer, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("stopped on max_tokens"), "{}", run.err);
}

#[test]
fn the_ceiling_is_read_as_minutes_and_says_so_when_it_cannot_be() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest_within(AIM, "soon");
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("45m"), "{}", run.err);
    let zero = repo.attest_within(AIM, "0");
    assert_eq!(zero.code, 2, "{}", zero.err);
    assert!(
        zero.err.contains("must be greater than zero"),
        "{}",
        zero.err
    );
}

#[test]
fn a_round_cut_short_is_resumed_where_it_stopped() {
    let repo = Repo::new();
    let hangs_once = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"cat >> prompts; echo "[$AGENT_VERDICT_PRIOR_SESSION]" >> handed; "#,
        r#"if [ "$n" = 1 ]; then sleep 60; fi; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0""#
    );
    repo.declare_runner(hangs_once, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let killed = repo.attest_within(AIM, "2s");
    assert_eq!(killed.code, 2, "{}", killed.err);
    // The reviewer got as far as writing its transcript, which is what makes the round worth taking up.
    let cut_short = repo.last_assigned();
    repo.transcript_for(&cut_short);

    let resumed = repo.again();
    assert_eq!(resumed.code, 0, "{}", resumed.err);
    assert!(resumed.err.contains("resuming session"), "{}", resumed.err);
    assert!(resumed.err.contains(&cut_short), "{}", resumed.err);
    assert!(resumed.err.contains("last wrote"), "{}", resumed.err);
    assert!(
        repo.read("handed").contains(&format!("[{cut_short}]")),
        "{}",
        repo.read("handed")
    );
    assert!(repo.prompts().contains("interrupted"), "{}", repo.prompts());
    assert!(
        !repo.prompts().contains("Fixes incorporated"),
        "{}",
        repo.prompts()
    );
}

// Codex names its own thread, so the round reaches it only through the link the tool keeps.
#[test]
fn a_codex_round_cut_short_is_resumed_through_its_link() {
    let repo = Repo::new();
    let hangs_once = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"cat >> prompts; echo "[$AGENT_VERDICT_PRIOR_SESSION]" >> handed; "#,
        r#"if [ "$n" = 1 ]; then sleep 60; fi; "#,
        r#"echo "VERDICT: major=0 moderate=0 minor=0""#
    );
    repo.declare_codex(hangs_once, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let killed = repo.attest_within(AIM, "2s");
    assert_eq!(killed.code, 2, "{}", killed.err);
    let links = repo.codex_links();
    assert_eq!(links.len(), 1, "{links:?}");
    let (handle, thread) = &links[0];
    assert!(killed.err.contains(thread.as_str()), "{}", killed.err);

    let resumed = repo.again();
    assert_eq!(resumed.code, 0, "{}", resumed.err);
    assert!(
        resumed.err.contains(&format!("resuming session {handle}")),
        "{}",
        resumed.err
    );
    assert_eq!(repo.read("handed"), format!("[]\n[{thread}]\n"));
    assert!(repo.prompts().contains("interrupted"), "{}", repo.prompts());
    assert_eq!(repo.codex_links(), Vec::new());
}

#[test]
fn a_codex_resume_that_fails_names_its_transcript_and_drops_its_link() {
    let repo = Repo::new();
    let hangs_then_fails = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"if [ "$n" = 1 ]; then sleep 60; fi; "#,
        r#"echo "the model went away"; exit 1"#
    );
    repo.declare_codex(hangs_then_fails, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest_within(AIM, "2s").code, 2);
    let (_, thread) = repo.codex_links().pop().expect("a link");

    let failed = repo.again();
    assert_eq!(failed.code, 2, "{}", failed.err);
    assert!(failed.err.contains("the model went away"), "{}", failed.err);
    assert!(
        failed.err.contains(&format!("{thread}.jsonl")),
        "the failed resume did not name its transcript: {}",
        failed.err
    );
    assert_eq!(repo.codex_links(), Vec::new());
}

// The resume that fails drops the link, so only what was resolved while it stood still names the thread.
#[test]
fn a_reset_deletes_a_codex_thread_whose_resume_failed() {
    let repo = Repo::new();
    let hangs_then_fails = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"if [ "$n" = 1 ]; then sleep 60; fi; "#,
        r#"echo "the model went away"; exit 1"#
    );
    repo.declare_codex(hangs_then_fails, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest_within(AIM, "2s").code, 2);
    let (_, thread) = repo.codex_links().pop().expect("a link");
    assert_eq!(repo.again().code, 2);
    let root = repo.root();
    let reset = repo.capture(&["reset", "--repo", &root, "the change changed shape"]);
    assert_eq!(reset.code, 0, "{}", reset.err);
    assert_eq!(repo.read("deleted"), format!("{thread}\n"));
}

// Codex keeps the brief its thread started with, so a changed brief cannot be resumed into it.
#[test]
fn a_codex_gate_resumes_its_thread_only_while_its_brief_is_unchanged() {
    let repo = Repo::new();
    let escalating = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"echo "[$AGENT_VERDICT_PRIOR_SESSION]" >> handed; m=0; if [ "$n" != 2 ]; then m=1; fi; "#,
        r#"echo "VERDICT: major=$m moderate=0 minor=0""#
    );
    repo.declare_codex(escalating, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 1);
    let first = repo
        .read("threads")
        .lines()
        .last()
        .expect("a thread")
        .to_string();
    repo.write("src.rs", "reworked");
    repo.stage(&["src.rs"]);
    assert_eq!(repo.again().code, 0);
    assert!(
        repo.read("handed").ends_with(&format!("[{first}]\n")),
        "{}",
        repo.read("handed")
    );
    assert_eq!(repo.codex_links(), Vec::new());

    repo.commit();
    repo.write("src.rs", "changed again");
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 1);
    repo.write("rubric.md", "a different standard");
    repo.write("src.rs", "reworked again");
    repo.stage(&["src.rs"]);
    repo.again();
    assert!(
        repo.read("handed").ends_with("[]\n[]\n"),
        "a thread briefed with the old rubric was resumed: {}",
        repo.read("handed")
    );
}

#[test]
fn a_round_that_left_no_transcript_opens_a_fresh_reviewer() {
    let repo = Repo::new();
    let hangs_once = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"cat >> prompts; "#,
        r#"if [ "$n" = 1 ]; then sleep 60; fi; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0""#
    );
    repo.declare_runner(hangs_once, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let killed = repo.attest_within(AIM, "2s");
    assert_eq!(killed.code, 2, "{}", killed.err);
    let cut_short = repo.last_assigned();

    let fresh = repo.again();
    assert_eq!(fresh.code, 0, "{}", fresh.err);
    assert!(!fresh.err.contains("died mid-review"), "{}", fresh.err);
    assert_ne!(
        repo.last_assigned(),
        cut_short,
        "the dead session was reused"
    );
    assert!(
        !repo.prompts().contains("interrupted"),
        "{}",
        repo.prompts()
    );
}

#[test]
fn a_recorded_round_leaves_no_marker_for_the_next_one_to_resume() {
    let repo = Repo::new();
    let escalating = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"cat >> prompts; m=0; if [ "$n" = 1 ]; then m=1; fi; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=$m moderate=0 minor=0""#
    );
    repo.declare_runner(escalating, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let blocked = repo.attest(AIM);
    assert_eq!(blocked.code, 1, "{}", blocked.err);
    repo.write("src.rs", "reworked");
    repo.stage(&["src.rs"]);
    let again = repo.again();
    assert_eq!(again.code, 0, "{}", again.err);
    assert!(!again.err.contains("died mid-review"), "{}", again.err);
    assert!(
        repo.prompts().contains("Fixes incorporated"),
        "{}",
        repo.prompts()
    );
    assert!(
        !repo.prompts().contains("interrupted"),
        "{}",
        repo.prompts()
    );
}

#[test]
fn a_resume_that_fails_is_not_resumed_again() {
    let repo = Repo::new();
    let dies_then_refuses = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"echo "[$AGENT_VERDICT_PRIOR_SESSION]" >> handed; "#,
        r#"if [ "$n" = 1 ]; then sleep 60; fi; "#,
        r#"echo "nothing this tool can record""#
    );
    repo.declare_runner(dies_then_refuses, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let killed = repo.attest_within(AIM, "2s");
    assert_eq!(killed.code, 2, "{}", killed.err);
    let cut_short = repo.last_assigned();
    repo.transcript_for(&cut_short);

    let resumed = repo.again();
    assert_eq!(resumed.code, 2, "{}", resumed.err);
    assert!(
        repo.read("handed").contains(&format!("[{cut_short}]")),
        "the killed round was not the one taken up: {}",
        repo.read("handed")
    );

    let fresh = repo.again();
    assert_eq!(fresh.code, 2, "{}", fresh.err);
    assert_ne!(
        repo.last_assigned(),
        cut_short,
        "a session that failed on resume was resumed a second time"
    );
}

// The kernel holds the claim, so a file left behind by a run that died holds nothing: there is no pid to test, no start time to compare, and nothing to reap.
#[test]
fn a_claim_file_nobody_holds_is_simply_taken() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.write(".git/agent-verdict.lock", "0");
    let run = repo.attest(AIM);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.out.contains("standards:"), "{}", run.out);
}

#[test]
fn attest_returns_while_its_round_is_still_reviewing() {
    let repo = Repo::new();
    repo.declare_agent("echo . > reviewing\nsleep 30", &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let running = common::pipe(&repo.dir.join("reviewing"));
    let root = repo.root();
    let started = repo.capture(&["attest", "--repo", &root, "--intent", AIM]);
    assert_eq!(started.code, 0, "{}", started.err);
    common::arrived_at(&running).expect("the reviewer never started");
    let at = repo.last_round().expect("the round named where it writes");
    assert!(
        !at.join("status").exists(),
        "the round had already finished"
    );
    assert_eq!(repo.aborted().code, 0);
}

#[test]
fn await_carries_the_rounds_own_verdict() {
    let repo = Repo::new();
    repo.declare(BLOCKER, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.capture_attest(AIM).code, 0);
    let waited = repo.awaited();
    assert_eq!(waited.code, 1, "{}", waited.err);
    assert!(waited.err.contains("BLOCKED"), "{}", waited.err);
}

#[test]
fn abort_ends_the_round_and_keeps_what_was_earned() {
    let repo = Repo::new();
    let slow_second = concat!(
        r#"if grep -q "ann" "$AGENT_VERDICT_SYSTEM" && [ -f slow ]; then "#,
        r#"rm -f slow; echo . > reviewing; sleep 30; fi; "#,
        r#"echo "VERDICT: reviewer=fake session=s major=0 moderate=0 minor=0""#
    );
    let second_gate = r#""$1" ann --doc rubric.md --path ."#;
    repo.declare_runner(slow_second, &[STANDARDS, second_gate]);
    repo.write("slow", "");
    repo.stage(&["src.rs"]);
    let running = common::pipe(&repo.dir.join("reviewing"));
    let root = repo.root();
    assert_eq!(
        repo.capture(&["attest", "--repo", &root, "--intent", AIM])
            .code,
        0
    );
    common::arrived_at(&running).expect("the second gate never started");
    let ended = repo.aborted();
    assert_eq!(ended.code, 0, "{}", ended.err);
    assert!(ended.err.contains("standards"), "{}", ended.err);
    let again = repo.again();
    assert!(again.out.contains("2-ann.log"), "{}", again.out);
    assert!(!again.out.contains("2-standards.log"), "{}", again.out);
}

#[test]
fn reset_during_a_round_names_abort() {
    let repo = Repo::new();
    repo.declare_agent("echo . > reviewing\nsleep 30", &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let running = common::pipe(&repo.dir.join("reviewing"));
    let root = repo.root();
    assert_eq!(
        repo.capture(&["attest", "--repo", &root, "--intent", AIM])
            .code,
        0
    );
    common::arrived_at(&running).expect("the reviewer never started");
    let refused = repo.capture(&["reset", "--repo", &root, "the change changed shape"]);
    assert_eq!(refused.code, 2, "{}", refused.err);
    assert!(refused.err.contains("abort"), "{}", refused.err);
    assert_eq!(repo.aborted().code, 0);
}

#[test]
fn a_landed_commit_takes_its_reviewers_claude_sessions_with_it() {
    let repo = Repo::new();
    repo.declare(BLOCKER, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 1);
    repo.declare(CLEAN, &[STANDARDS]);
    repo.write("src.rs", "reworked");
    repo.stage(&["src.rs"]);
    assert_eq!(repo.again().code, 0);
    assert_eq!(
        repo.saved_sessions().len(),
        1,
        "the review left no session to resume"
    );
    let landed = repo.commit();
    assert_eq!(landed.code, 0, "{}", landed.err);
    assert_eq!(repo.saved_sessions(), Vec::<std::path::PathBuf>::new());
}

#[test]
fn a_landed_commit_deletes_its_codex_threads_through_codex() {
    let repo = Repo::new();
    repo.declare_codex(&format!("echo '{CLEAN}'"), &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 0);
    // The judge's thread was never saved: only the review's rollout is on disk.
    assert_eq!(repo.saved_sessions().len(), 1);
    let review = repo
        .read("threads")
        .lines()
        .last()
        .expect("a thread")
        .to_string();
    assert_eq!(repo.commit().code, 0);
    assert_eq!(repo.read("deleted"), format!("{review}\n"));
    assert_eq!(repo.saved_sessions(), Vec::<std::path::PathBuf>::new());
}

#[test]
fn an_intent_judged_alone_leaves_no_session() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.judge("VERDICT: refused — gives a reason");
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 2);
    assert_eq!(repo.saved_sessions(), Vec::<std::path::PathBuf>::new());
}

#[test]
fn a_reset_discards_the_sessions_it_clears() {
    let repo = Repo::new();
    repo.declare(BLOCKER, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 1);
    assert_eq!(repo.saved_sessions().len(), 1);
    let root = repo.root();
    let reset = repo.capture(&["reset", "--repo", &root, "the change changed shape"]);
    assert_eq!(reset.code, 0, "{}", reset.err);
    assert_eq!(repo.saved_sessions(), Vec::<std::path::PathBuf>::new());
}

#[test]
fn an_answered_audit_leaves_no_session() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let audit = repo.audit();
    assert_eq!(audit.code, 0, "{}", audit.err);
    assert_eq!(repo.saved_sessions(), Vec::<std::path::PathBuf>::new());
}

#[test]
fn a_codex_thread_that_would_not_delete_is_kept_and_retried_at_the_next_commit() {
    let repo = Repo::new();
    repo.declare_codex(&format!("echo '{CLEAN}'"), &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 0);
    let first = repo
        .read("threads")
        .lines()
        .last()
        .expect("a thread")
        .to_string();
    repo.write("delete-fails", "");
    let landed = repo.commit();
    assert_eq!(landed.code, 0, "{}", landed.err);
    assert!(landed.err.contains("database is locked"), "{}", landed.err);
    assert_eq!(
        repo.saved_sessions().len(),
        1,
        "the rollout went without codex"
    );

    std::fs::remove_file(repo.dir.join("delete-fails")).expect("unblock");
    repo.write("src.rs", "changed again");
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 0);
    assert_eq!(repo.commit().code, 0);
    assert!(
        repo.read("deleted").lines().any(|t| t == first),
        "{}",
        repo.read("deleted")
    );
    assert_eq!(repo.saved_sessions(), Vec::<std::path::PathBuf>::new());
}

#[test]
fn a_codex_thread_codex_keeps_refusing_is_reported_once_per_try_then_left_to_prune() {
    let repo = Repo::new();
    repo.declare_codex(&format!("echo '{CLEAN}'"), &[STANDARDS]);
    repo.write("delete-fails", "");
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest(AIM).code, 0);
    let first = repo
        .read("threads")
        .lines()
        .last()
        .expect("a thread")
        .to_string();
    let landed = repo.commit();
    assert!(
        landed
            .err
            .contains("1 reviewer session(s) could not be deleted"),
        "{}",
        landed.err
    );
    let root = repo.root();
    let mut said = String::new();
    for round in 0..2 {
        repo.write("src.rs", &format!("change {round}"));
        repo.stage(&["src.rs"]);
        assert_eq!(repo.attest(AIM).code, 0);
        said = repo
            .capture(&["reset", "--repo", &root, "the change changed shape"])
            .err;
    }
    assert!(said.contains("no longer retried"), "{said}");
    assert!(said.contains(&first), "{said}");
    let undeleted = repo.dir.join(".git/agent-verdict/undeleted");
    let left = std::fs::read_to_string(undeleted).unwrap_or_default();
    assert!(!left.contains(&first), "{left}");
}

// A cut-short round keeps its link, so the ledger names one thread twice: by handle and by thread.
#[test]
fn a_thread_named_by_handle_and_by_thread_is_tried_once() {
    let repo = Repo::new();
    repo.declare_codex("sleep 60", &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.attest_within(AIM, "2s").code, 2);
    assert_eq!(repo.codex_links().len(), 1);
    repo.write("delete-fails", "");
    let root = repo.root();
    let reset = repo.capture(&["reset", "--repo", &root, "the change changed shape"]);
    assert!(
        reset
            .err
            .contains("1 reviewer session(s) could not be deleted"),
        "{}",
        reset.err
    );
}
