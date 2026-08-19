// Concern: how a run ends and what it leaves behind — the ceiling, a kill, the claim, a round taken up again | Non-concern: what a review records (tests/attest.rs) | IO: (repo, hook) -> status, files

mod common;

use common::{Repo, STANDARDS};

const CLEAN: &str = "VERDICT: reviewer=fake session=s-01 major=0 moderate=1 minor=2";
const AIM: &str = "raise the staged file's line count";

// The tool's own ceiling, because without one the only thing that ends a hung reviewer is whatever shell is holding the run — which kills it with no elapsed time, no signal and nothing said.
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
    // The wait is accounted for as it happens — the difference between a run killed at thirteen seconds and one killed at ten minutes, which is invisible in the kill itself.
    assert!(run.err.contains("still reviewing"), "{}", run.err);
    assert!(run.err.contains("--timeout"), "{}", run.err);
    // What it had said before it stopped, which is the half of the diagnosis a bare timeout throws away.
    assert!(run.err.contains("still thinking"), "{}", run.err);
}

// An agent can exit while something it spawned still holds the pipe it was writing to. Waiting for that pipe to close is waiting on a process this tool never started and cannot bound — the ceiling defeated by the one path that was meant not to need it.
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

// An agent can crash and still exit 0, and then its stderr is the only account of the fault. A message built from the exit status alone reports what this side saw rather than what that side did.
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

// Cut off at a limit and answering at length while ignoring the brief are the same silence here, and they are not the same fault: one is re-run, the other is a brief to fix.
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

// Minutes is the unit a review is discussed in, so a bare number is one. The refusal states the whole grammar rather than the one form it just rejected.
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
    assert!(zero.err.contains("above zero"), "{}", zero.err);
}

// The reviewer that was cut off holds everything it had read; the round is taken up rather than paid for from the top. Nothing changed while it was gone, so it is told that and not that fixes were made.
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
    assert!(resumed.err.contains("died mid-review"), "{}", resumed.err);
    assert!(resumed.err.contains(&cut_short), "{}", resumed.err);
    // The one measured fact about the end. Everything else after a kill is inferred from markers written before it.
    assert!(
        resumed.err.contains("transcript was last written"),
        "{}",
        resumed.err
    );
    // Handed back to the same reviewer, and told it was interrupted rather than re-reviewed.
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

// A round that died before its reviewer wrote anything has nothing to take up, and a resume of it would be a refusal in place of a review.
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

// The marker says a round is open, so recording its verdict must close it: left behind, it would resume a reviewer that had already answered and brief it as though it never had.
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

// Taking up a dead round is worth one attempt. A session that cannot be finished would otherwise be resumed by every run from here, each paying to learn the same thing.
#[test]
fn a_resume_that_fails_is_not_resumed_again() {
    let repo = Repo::new();
    // The first round is killed mid-review, which is the only thing that leaves a marker in earnest. Every later round refuses.
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

// One review at a time, with nothing else in the way: no signal, no inheritance, no round taken up. The claim is the whole subject, so when this fails it is the claim that broke.
#[test]
fn a_second_attest_is_refused_while_one_is_running() {
    let repo = Repo::new();
    repo.declare_agent("echo $$ > reviewing\nsleep 30", &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let root = repo.root();
    let reviewing = common::pipe(&repo.dir.join("reviewing"));
    let mut running = repo.running(&[
        "attest",
        "--repo",
        &root,
        "--intent",
        AIM,
        common::BACKGROUND,
    ]);
    let reviewer =
        common::arrived_at(&reviewing).expect("the first run never reached its reviewer");
    let beside = repo.attest(AIM);
    assert_eq!(beside.code, 2, "{}", beside.err);
    assert!(
        beside.err.contains("another review is already running"),
        "{}",
        beside.err
    );
    assert!(
        beside.err.contains(&format!("kill {reviewer}")),
        "the refusal did not name the reviewer {reviewer}: {}",
        beside.err
    );
    let _ = running.kill();
    let _ = running.wait();
}

// A run reaped by whatever shell holds it wrote nothing on its way out, so the kill read exactly like a reviewer that stopped on its own. The handler is the only chance this side gets to say otherwise.
#[test]
fn a_run_killed_by_a_signal_says_so_and_leaves_its_reviewer_holding_the_claim() {
    let repo = Repo::new();
    repo.declare_agent("echo $$ > reviewing\nsleep 30", &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let root = repo.root();
    // Opened by the reviewer once it is running, not by the round marker that precedes it: the round is recorded before the reviewer is spawned, so a signal fired on the marker can land while nothing yet holds the claim, which is the one thing this test is about.
    let running_now = common::pipe(&repo.dir.join("reviewing"));
    let running = repo.running(&[
        "attest",
        "--repo",
        &root,
        "--intent",
        AIM,
        common::BACKGROUND,
    ]);
    let reviewer =
        common::arrived_at(&running_now).expect("the reviewer never started, so nothing held");
    let killed = std::process::Command::new("kill")
        .args(["-TERM", &running.id().to_string()])
        .status()
        .expect("kill runs");
    assert!(killed.success(), "could not signal the run");
    let out = running.wait_with_output().expect("it exits");
    let said = String::from_utf8_lossy(&out.stderr);
    // One line: the round it was in and what that means for it are one sentence, not a sentence and a fragment under it.
    assert!(
        said.contains("killed by SIGTERM while reviewing standards, session"),
        "{said}"
    );
    // Where it carried on, not merely that it did: the reviewer survived the run, and a reader told only that a round is resumable has no way to see what it is doing or to end it.
    assert!(
        said.contains(&format!(
            ". The review is still running, detached: pid {reviewer}, holding this repo until it answers."
        )),
        "{said}"
    );
    assert!(said.contains(" It writes to "), "{said}");
    assert!(
        said.contains(" The round is resumable, once it is done or ended.\n"),
        "{said}"
    );
    // Died of the signal rather than exiting on a status that stands for it: a supervisor reads the two apart, and this run was killed.
    assert_eq!(
        std::os::unix::process::ExitStatusExt::signal(&out.status),
        Some(libc::SIGTERM),
        "{said}"
    );
    // The reviewer inherited the claim, so it is still held after the run that took it is gone: a second run must find the repo occupied rather than open a review beside a live one.
    let beside = repo.attest(AIM);
    assert_eq!(beside.code, 2, "{}", beside.err);
    assert!(
        beside.err.contains("another review is already running"),
        "{}",
        beside.err
    );
    // The reviewer by its own pid, not merely something to kill: the claim is inherited and the holder is nobody this repo wrote down, so a refusal that named the refusing run instead would send the reader after a process that is already gone.
    assert!(
        beside.err.contains(&format!("kill {reviewer}")),
        "the refusal did not name the reviewer {reviewer}: {}",
        beside.err
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

// A reviewer that leaves something running behind it has handed the repo's claim to a process no later run can see or ask about. Not covered here: a reviewer that puts itself in a new session, which no group signal reaches — the claim would then be held until it exits on its own. The review is over when the reviewer answers, so what it started ends with it.
#[test]
fn a_helper_the_reviewer_leaves_behind_does_not_hold_the_repo() {
    let repo = Repo::new();
    // Answers cleanly, having started something that outlives it and inherited the claim.
    let leaves_a_helper = concat!(
        "sleep 120 &\n",
        r#"python3 -c 'import json; print(json.dumps({"is_error": False, "result": "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0", "session_id": "s-1", "modelUsage": {"fake-model": {}}}))'"#
    );
    repo.declare_agent(leaves_a_helper, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let first = repo.attest(AIM);
    assert_eq!(first.code, 0, "{}", first.err);

    // The claim is free the moment the review is, not whenever the helper happens to exit.
    let again = repo.again();
    assert!(
        !again.err.contains("another review is already running"),
        "a helper outlived the review and kept the repo: {}",
        again.err
    );
    assert_eq!(again.code, 0, "{}", again.err);
}

// The sentence is armed for the round and dropped when it ends. Killed before any round opens, a run has nothing to say about one, and saying it anyway would name a review that never started.
#[test]
fn a_run_killed_before_a_round_opens_names_no_review() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    // The judge holds, so the signal lands while the run is still deciding the aim and before any round is opened.
    repo.write("slow-judge", "");
    let judging = common::pipe(&repo.dir.join("judging"));
    let running = repo.running(&[
        "attest",
        "--repo",
        &repo.root(),
        "--intent",
        AIM,
        common::BACKGROUND,
    ]);
    common::arrived_at(&judging).expect("the run never reached the judge");
    let _ = std::process::Command::new("kill")
        .args(["-TERM", &running.id().to_string()])
        .status();
    let out = running.wait_with_output().expect("it exits");
    let said = String::from_utf8_lossy(&out.stderr);
    assert!(said.contains("killed by SIGTERM"), "{said}");
    assert!(
        !said.contains("while reviewing"),
        "a run killed before any round opened named one: {said}"
    );
}
