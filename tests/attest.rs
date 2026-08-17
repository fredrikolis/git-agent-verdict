// Concern: the review attest runs — the gate it takes, what it records, the commit it lands | Non-concern: what the gate refuses (tests/gate.rs) | IO: (temp repo, hook) -> status, commit

mod common;

use common::{Repo, PROSE, STANDARDS};

const CLEAN: &str = "VERDICT: reviewer=fake session=s-01 major=0 moderate=1 minor=2";
const BLOCKER: &str = "VERDICT: reviewer=fake session=s-02 major=1 moderate=0 minor=0";
const AIM: &str = "raise the staged file's line count";

// Scope is the gate's own pathspec, handed over as the command that applies it: an unscoped diff would show the reviewer files another gate owns.
#[test]
fn the_brief_scopes_the_diff_to_the_gates_pathspec() {
    let repo = Repo::new();
    let scoped = r#""$1" standards --doc rubric.md --path "*.rs""#;
    repo.declare(CLEAN, &[scoped]);
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["--reviewer-prompt", "standards"]);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(
        run.out.contains("git diff --cached -- '*.rs'"),
        "{}",
        run.out
    );
}

// A doc is read in, not pointed at: a path is something a reviewer may skim or skip, and it is the same bytes every round.
#[test]
fn the_criteria_carry_the_documents_themselves() {
    let repo = Repo::new();
    repo.write("rubric.md", "the measure: every line earns its place");
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["--reviewer-prompt", "standards"]);
    assert!(
        run.out.contains("<document title=\"rubric.md\">"),
        "{}",
        run.out
    );
    assert!(
        run.out.contains("every line earns its place"),
        "{}",
        run.out
    );
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
    repo.attest(AIM);
    for _ in 0..2 {
        let run = repo.again();
        assert!(run.out.contains("standards:"), "{}", run.out);
        assert!(!run.out.contains("prose:"), "{}", run.out);
    }
}

#[test]
fn gates_are_reviewed_in_the_order_the_hook_declares_them() {
    let repo = Repo::new();
    let second_gate = r#""$1" ann --doc rubric.md --path ."#;
    repo.declare(CLEAN, &[STANDARDS, second_gate]);
    repo.stage(&["src.rs"]);
    let first = repo.attest(AIM);
    assert!(first.out.contains("standards:"), "{}", first.out);
    let second = repo.again();
    assert!(second.out.contains("ann:"), "{}", second.out);
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

// Acting on a MODERATE or a MINOR moves content, and a round keyed on that resamples advice for ever: every review of a taste-adjacent rubric returns something.
#[test]
fn a_fix_after_a_passing_verdict_does_not_review_again() {
    let repo = Repo::new();
    let counting = r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; echo "VERDICT: reviewer=fake session=s-1 major=0 moderate=$n minor=0""#;
    repo.declare_runner(counting, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    repo.write("src.rs", "the fix the review asked for");
    repo.stage(&["src.rs"]);
    let message = repo.landed_again(3);
    assert_eq!(repo.read("rounds").trim(), "1", "{message}");
    assert!(message.contains("moderate=1"), "{message}");
}

// MAJOR is the rung that re-opens a gate, and the round after it is the one that wants the reviewer's own context.
#[test]
fn a_blocked_gate_reviews_again_and_is_handed_its_session() {
    let repo = Repo::new();
    let escalating = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"echo "[$AGENT_VERDICT_PRIOR_SESSION]" >> handed; m=0; if [ "$n" = 1 ]; then m=1; fi; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=$m moderate=0 minor=0""#
    );
    repo.declare_runner(escalating, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let blocked = repo.attest(AIM);
    assert_eq!(blocked.code, 1, "{}", blocked.err);
    repo.write("src.rs", "reworked");
    repo.stage(&["src.rs"]);
    let message = repo.landed_again(3);
    assert_eq!(repo.read("rounds").trim(), "2", "{message}");
    assert_eq!(
        repo.read("handed"),
        "[]\n[s-1]\n",
        "{}",
        repo.read("handed")
    );
    assert!(message.contains("major=0"), "{message}");
}

// A reviewer whose session carried over holds the aim, the documents and the ladder already. Sending them again invites the whole sweep a second time instead of a look at what moved.
#[test]
fn a_reviewer_that_resumed_is_briefed_only_on_what_changed() {
    let repo = Repo::new();
    let recording = concat!(
        r#"n=$(cat n 2>/dev/null || echo 0); cat > prompt-$n; "#,
        r#"cp "$AGENT_VERDICT_SYSTEM" system-$n; echo $((n+1)) > n; "#,
        r#"m=0; if [ "$n" = 0 ]; then m=1; fi; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=$m moderate=0 minor=0""#
    );
    repo.declare_runner(recording, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    repo.write("src.rs", "reworked");
    repo.stage(&["src.rs"]);
    repo.again();
    let (first, second) = (repo.read("prompt-0"), repo.read("prompt-1"));
    assert!(
        first.contains(&format!("<diff-intent>{AIM}</diff-intent>")),
        "{first}"
    );
    assert!(second.starts_with("Fixes incorporated"), "{second}");
    // Handed over once and unchanged, which is the whole of what a cache needs.
    let standing = repo.read("system-0");
    assert_eq!(standing, repo.read("system-1"));
    assert!(standing.contains("<grading-criteria>"), "{standing}");
    assert!(!standing.contains(AIM), "the aim is not in the cached half");
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
    assert!(
        from_sub.out.contains("<document title="),
        "{}",
        from_sub.out
    );
}

// The verdict claims the staged content was reviewed, and a reviewer opens files to read them in context. Where the two disagree it reviewed what the commit will not carry — right repo, wrong tree state.
#[test]
fn an_unstaged_edit_to_a_reviewed_file_refuses_before_reviewing() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.write("src.rs", "edited after staging, never added");
    let run = repo.attest(AIM);
    assert_eq!(run.code, 1, "{}", run.err);
    assert!(
        run.err.contains("index and the working tree disagree"),
        "{}",
        run.err
    );
    // Before the cheapest call the run makes, so nothing was spent on it.
    assert!(!run.err.contains("judging the intent"), "{}", run.err);
    assert!(!repo.committed(), "{}", run.err);
}

// Staging one change and carrying on with another is ordinary git. Only the files a gate actually reviews have to agree with the index.
#[test]
fn an_unstaged_edit_no_gate_reviews_is_left_alone() {
    let repo = Repo::new();
    let scoped = r#""$1" standards --doc rubric.md --path "*.rs""#;
    repo.declare(CLEAN, &[scoped]);
    repo.write("notes.txt", "tracked");
    repo.stage(&["notes.txt", "src.rs", "rubric.md"]);
    common::git(&repo.dir, &["commit", "--no-verify", "-q", "-m", "base"]);
    repo.write("notes.txt", "edited, and no gate reads it");
    repo.write("src.rs", "the change under review");
    repo.stage(&["src.rs"]);
    let run = repo.attest("raise the staged file's line count");
    assert!(
        !run.err.contains("index and the working tree disagree"),
        "{}",
        run.err
    );
    let message = repo.landed_again(3);
    assert!(message.contains("Reviewed-standards:"), "{message}");
}

// How hard a gate is worth reviewing is the repo's call: an annotation check and a correctness review are not worth the same model, and the tool has no business choosing for either.
#[test]
fn a_gate_declaring_a_model_hands_it_to_the_agent() {
    let repo = Repo::new();
    let heavy = r#""$1" standards --model opus --doc rubric.md --path .;"#;
    let light = r#""$1" prose --simple --model haiku --doc rubric.md --path .;"#;
    repo.declare(CLEAN, &[heavy, light]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    repo.again();
    assert_eq!(repo.read("asked-model"), "[opus]\n[haiku]\n");
}

// Nothing here keeps a list of which models exist; that list would go stale, and the agent already answers for an unknown one in its own words.
#[test]
fn an_unknown_model_is_the_hooks_fault_and_says_so() {
    let repo = Repo::new();
    let gate = r#""$1" standards --model no-such-model --doc rubric.md --path .;"#;
    repo.declare(CLEAN, &[gate]);
    repo.write("refuse-model", "");
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(
        run.err.contains("declares --model no-such-model"),
        "{}",
        run.err
    );
    assert!(run.err.contains("may not have access to it"), "{}", run.err);
    assert!(run.err.contains("--no-verify"), "{}", run.err);
    assert!(!repo.committed(), "{}", run.err);
}

// Two runs at once review the same gate, pay for it twice, and the second to finish writes the diary the first already wrote. A caller that guards against this by hand writes the wait loop that never ends; the tool holds the claim so nobody has to.
#[test]
fn a_second_attest_is_refused_while_one_is_running() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let exe = std::env::current_exe().expect("exe");
    let held = format!(
        "{}\t0\t{}",
        std::process::id(),
        exe.file_name().expect("name").to_string_lossy()
    );
    repo.write(".git/agent-verdict.lock", &held);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(
        run.err.contains("another attest is already running"),
        "{}",
        run.err
    );
    assert!(run.err.contains("Wait for it, or kill it"), "{}", run.err);
    assert!(!run.err.contains("judging the intent"), "{}", run.err);
}

// A run that was killed leaves its claim behind, and a repo no command can enter again is worse than the race the claim prevents.
#[test]
fn a_claim_left_by_a_dead_run_is_taken_over() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.write(
        ".git/agent-verdict.lock",
        "999999999\t0\t/nowhere/git-agent-verdict",
    );
    let run = repo.attest(AIM);
    assert!(run.out.contains("standards:"), "{}", run.err);
    // Cleared with a word, not in silence: the claim is the only record this tool holds that a run ended some other way than by finishing.
    assert!(run.err.contains("stale lock file"), "{}", run.err);
    assert!(run.err.contains("pid 999999999"), "{}", run.err);
    // The file itself, named: "the claim" is this tool's word for it and tells a reader nothing they can go and look at.
    assert!(run.err.contains("agent-verdict.lock"), "{}", run.err);
    // How long it ran is recorded nowhere, so it is not claimed here.
    assert!(!run.err.contains("when it stopped."), "{}", run.err);
}

// Nothing is holding it once the run is over, or the next attest meets its own leftovers.
#[test]
fn the_claim_is_released_when_the_run_ends() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    repo.attest(AIM);
    assert!(
        !repo.dir.join(".git/agent-verdict.lock").exists(),
        "the claim outlived the run"
    );
}

// The pin is the one line enumeration honours. Read past it, attest would review against a declaration nobody has established this release can parse — and pay for it before git ever runs the hook that refuses.
#[test]
fn a_hook_pinned_to_another_line_refuses_before_a_review_is_paid_for() {
    let repo = Repo::new();
    repo.declare(CLEAN, &["--require-version 99.0", STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert!(run.err.contains("--version '^99.0'"), "{}", run.err);
    assert!(!run.err.contains("judging the intent"), "{}", run.err);
    assert!(!repo.committed(), "{}", run.err);
}

// Enumerating the hook must not act on it: under `set -e` a mode that refuses during the listing kills the hook, and every gate below it leaves the listing.
#[test]
fn a_staged_rubric_still_lets_attest_read_the_hook() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs", "rubric.md"]);
    let run = repo.attest(AIM);
    assert!(!run.err.contains("declared no gates"), "{}", run.err);
    assert!(run.err.contains("can never be attested"), "{}", run.err);
    assert!(!repo.committed(), "a rubric commit landed anyway");
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
    assert!(run.err.contains("does not move"), "{}", run.err);
}

// The limit bounds the change, not the prose: an aim needing more than a line is more than one commit.
#[test]
fn an_intent_naming_more_than_one_change_is_refused_with_the_remedy() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let two_changes = "x".repeat(301);
    let root = repo.root();
    let run = repo.capture(&[
        "attest",
        "--repo",
        &root,
        "--intent",
        &two_changes,
        "--confirm-running-in-background-shell-with-long-timeout",
    ]);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("commit them separately"), "{}", run.err);
}

// Undocumented, so the refusal is the only place an agent meets it: a review runs for many minutes, and a foreground shell kills it partway.
#[test]
fn attest_refuses_a_run_that_has_not_acknowledged_the_background_shell() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["attest", "--intent", AIM]);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("BACKGROUND shell"), "{}", run.err);
    assert!(!repo.committed(), "a foreground attest reviewed anyway");
}

// Named in no usage line and no guide: an agent that has read either still meets it at the first review, which is when it is worth reading.
#[test]
fn the_background_flag_is_documented_nowhere() {
    let repo = Repo::new();
    let help = repo.capture(&["--help"]);
    let guide = repo.capture(&["--repo-setup-guide"]);
    assert!(!help.out.contains("background"), "{}", help.out);
    assert!(!guide.out.contains("background"), "{}", guide.out);
}

// The guard the cap cannot enforce: an aim well under the limit can still argue, and the judge is what catches it — before a review is paid for.
#[test]
fn an_intent_the_judge_refuses_costs_no_review() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.judge("VERDICT: refused — gives a reason");
    repo.stage(&["src.rs"]);
    let run = repo.attest("add the cache because the old path was far too slow");
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("intent was refused"), "{}", run.err);
    assert!(run.err.contains("gives a reason"), "{}", run.err);
    assert!(!repo.committed(), "a refused intent committed anyway");
}

// The aim is judged before any gate is chosen, so an advisory-only hook is held to it exactly as a blocking one is.
#[test]
fn an_advisory_only_hook_still_judges_the_intent() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[PROSE]);
    repo.judge("VERDICT: refused — defends the approach");
    repo.stage(&["src.rs"]);
    let run = repo.attest("add the cache because the old path was far too slow");
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("intent was refused"), "{}", run.err);
}

// The counts are the whole of what the line carries now, so one missing is the broken contract. Who reviewed and on what session come from the agent.
#[test]
fn a_verdict_line_missing_a_count_is_refused() {
    for missing in [
        "VERDICT: moderate=0 minor=0",
        "VERDICT: major=0 minor=0",
        "VERDICT: major=0 moderate=0",
    ] {
        let repo = Repo::new();
        repo.declare(missing, &[STANDARDS]);
        repo.stage(&["src.rs"]);
        let run = repo.attest(AIM);
        assert_eq!(run.code, 2, "{missing}: {}", run.err);
        assert!(run.err.contains("needs major="), "{missing}: {}", run.err);
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

// The id is chosen here and handed over, not read back out of the answer: read back it arrives only in an answer that a crashed, hung or killed run never produced, which is every case with something to diagnose. Assigned first, it names the transcript before anything can go wrong.
#[test]
fn the_reviewers_session_is_assigned_before_it_runs_and_resumed_by_name_after() {
    let repo = Repo::new();
    let escalating = concat!(
        r#"n=$(cat rounds 2>/dev/null || echo 0); n=$((n+1)); echo "$n" > rounds; "#,
        r#"m=0; if [ "$n" = 1 ]; then m=1; fi; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=$m moderate=0 minor=0""#
    );
    repo.declare_runner(escalating, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let blocked = repo.attest(AIM);
    assert_eq!(blocked.code, 1, "{}", blocked.err);
    let assigned = repo.read("assigned-sessions");
    let opening: Vec<&str> = assigned.lines().filter(|l| !l.trim().is_empty()).collect();
    // The judge and the first review each open one; neither had a session to resume.
    assert_eq!(opening.len(), 2, "{assigned}");
    for id in &opening {
        assert_eq!(id.len(), 36, "{id} is not a uuid");
        assert_eq!(id.split('-').count(), 5, "{id} is not a uuid");
        assert_eq!(&id[14..15], "4", "{id} is not a version 4 uuid");
    }
    assert_ne!(opening[0], opening[1], "two rounds took one id: {assigned}");
    // The second round resumes the reviewer it already briefed, so it assigns nothing.
    repo.write("src.rs", "reworked");
    repo.stage(&["src.rs"]);
    let message = repo.landed_again(3);
    let after = repo.read("assigned-sessions");
    let opened: Vec<&str> = after.lines().filter(|l| !l.trim().is_empty()).collect();
    assert_eq!(
        opened.len(),
        2,
        "a resumed round opened a new session: {after}"
    );
    assert!(message.contains("Reviewed-standards:"), "{message}");
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

// The verb exists because a rubric that changed condemns code no commit is touching, and no diff will ever show it.
#[test]
fn audit_reviews_every_tracked_file_a_gate_reaches() {
    let repo = Repo::new();
    let recording = concat!(
        r#"cat > prompts; cp "$AGENT_VERDICT_SYSTEM" system-seen; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=0 moderate=2 minor=1""#
    );
    repo.declare_runner(recording, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.audit();
    assert_eq!(run.code, 0, "{}", run.err);
    // The tree, not the diff: what the reviewer is told to run says which.
    let brief = repo.read("system-seen");
    assert!(brief.contains("git ls-files -- '.'"), "{brief}");
    assert!(!brief.contains("git diff --cached"), "{brief}");
    assert!(brief.contains("every file it lists"), "{brief}");
    // No aim, because there is no commit to state one for.
    assert!(
        repo.prompts().contains("no commit and no diff"),
        "{}",
        repo.prompts()
    );
    assert!(
        run.out.contains("standards: major=0 moderate=2 minor=1"),
        "{}",
        run.out
    );
}

// An audit lands nothing: a trailer attests one commit, and there is no commit here.
#[test]
fn audit_records_nothing_and_commits_nothing() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    assert_eq!(repo.audit().code, 0);
    assert!(!repo.committed(), "audit committed");
    // The diary is untouched, so the next attest still reviews the gate itself.
    let attested = repo.attest(AIM);
    assert!(attested.out.contains("standards:"), "{}", attested.out);
}

// MAJOR is still MAJOR when nothing is being committed: the tree does not meet the rubric, and the run says so in its status.
#[test]
fn a_major_found_by_audit_is_reported_in_the_exit_status() {
    let repo = Repo::new();
    repo.declare(BLOCKER, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.audit();
    assert_eq!(run.code, 1, "{}", run.err);
    assert!(run.err.contains("including MAJOR"), "{}", run.err);
}

// The flag is the whole guard: an agent that reached for this verb because attest refused it gets told the difference, not the flag name.
#[test]
fn audit_without_the_whole_repo_confirmation_says_what_it_would_have_done() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let root = repo.root();
    let run = repo.capture(&["audit", "--repo", &root, common::BACKGROUND]);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("not the staged diff"), "{}", run.err);
    assert!(run.err.contains("attest"), "{}", run.err);
    assert!(!repo.read("rounds").contains('1'), "it reviewed anyway");
}

// Undocumented on purpose, like the background one: the refusal is where it is met.
#[test]
fn the_whole_repo_flag_is_in_no_usage_line() {
    let repo = Repo::new();
    let help = repo.capture(&["--help"]);
    assert!(!help.out.contains("--confirm-reviewing"), "{}", help.out);
    let guide = repo.capture(&["--repo-setup-guide"]);
    assert!(!guide.out.contains("--confirm-reviewing"), "{}", guide.out);
}

// A twenty-minute review that says nothing until it ends leaves a caller unable to tell a live one from a dead one. Naming the transcript and the command that reads it answers that for one line of output, and for nothing at all while nobody asks.
#[test]
fn a_review_names_its_transcript_and_how_to_read_it() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(
        run.out.contains("progress log being appended here:"),
        "{}",
        run.out
    );
    assert!(run.out.contains(".jsonl"), "{}", run.out);
    // The command, not a digest this tool renders: the transcript's shape is the agent's to change.
    assert!(run.out.contains("latest activity: jq"), "{}", run.out);
    assert!(run.out.contains("tail -5"), "{}", run.out);
}

// The guard printed one hardcoded attest example whichever verb was run, so an audit's caller was told to run attest, which is a different operation and commits.
#[test]
fn audits_foreground_guard_teaches_audit_not_attest() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let root = repo.root();
    let bare = repo.capture(&["audit", "--repo", &root]);
    assert_eq!(bare.code, 2, "{}", bare.err);
    // With nothing given, the first thing read is what audit does, not how to run attest.
    assert!(bare.err.contains("not the staged diff"), "{}", bare.err);
    // The usage block below carries every verb's flags; what must not appear is a remedy telling this caller to run attest.
    assert!(
        !bare.err.contains("git agent-verdict attest --repo"),
        "{}",
        bare.err
    );
    let shell = repo.capture(&["audit", "--repo", &root, common::WHOLE]);
    assert_eq!(shell.code, 2, "{}", shell.err);
    assert!(
        shell.err.contains("git agent-verdict audit"),
        "{}",
        shell.err
    );
    assert!(
        !shell.err.contains("git agent-verdict attest --repo"),
        "{}",
        shell.err
    );
    assert!(!shell.err.contains("--intent \""), "{}", shell.err);
    assert!(
        shell.err.contains("every file each gate reaches"),
        "{}",
        shell.err
    );
}

// A survey that stops at the first gate whose reviewer failed throws away every gate that answered, and the run had already paid for them.
#[test]
fn audit_sweeps_every_gate_even_when_one_reviewer_fails() {
    let repo = Repo::new();
    let second = r#""$1" ann --doc rubric.md --path ."#;
    let per_gate = concat!(
        r#"if grep -q "the standards gate" "$AGENT_VERDICT_SYSTEM" 2>/dev/null; then "#,
        r#"echo "nothing this tool can record"; else "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=0 moderate=1 minor=0"; fi"#
    );
    repo.declare_runner(per_gate, &[STANDARDS, second]);
    repo.stage(&["src.rs"]);
    let run = repo.audit();
    assert_eq!(run.code, 2, "{}", run.err);
    // The gate that answered still reported, and the one that did not is named.
    assert!(run.out.contains("ann: major=0 moderate=1"), "{}", run.out);
    assert!(run.err.contains("no verdict from"), "{}", run.err);
}
