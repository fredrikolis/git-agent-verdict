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
    let run = repo.attest(AIM);
    assert_eq!(run.code, 0, "{}", run.err);
    // The logs are numbered as the gates ran, so the order is in their names.
    assert!(run.out.contains("1-standards.log"), "{}", run.out);
    assert!(run.out.contains("2-ann.log"), "{}", run.out);
    assert_eq!(repo.commit().code, 0);
    let message = repo.head_message();
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
    assert!(run.err.contains("no MAJOR severity"), "{}", run.err);
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
    assert!(run.err.contains("cannot be attested"), "{}", run.err);
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
    assert!(run.err.contains("which is fixed"), "{}", run.err);
}

// The limit bounds the change, not the prose: an aim needing more than a line is more than one commit.
#[test]
fn an_intent_naming_more_than_one_change_is_refused_with_the_remedy() {
    let repo = Repo::new();
    repo.declare(CLEAN, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let two_changes = "x".repeat(301);
    let root = repo.root();
    let run = repo.capture(&["attest", "--repo", &root, "--intent", &two_changes]);
    assert_eq!(run.code, 2, "{}", run.err);
    assert!(run.err.contains("commit them separately"), "{}", run.err);
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
    let run = repo.capture(&["audit", "--repo", &root]);
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
    assert!(run.out.contains("progress log:"), "{}", run.out);
    assert!(run.out.contains(".jsonl"), "{}", run.out);
    // The command, not a digest this tool renders: the transcript's shape is the agent's to change.
    assert!(run.out.contains("latest activity: jq"), "{}", run.out);
    assert!(run.out.contains("tail -5"), "{}", run.out);
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

// The flag has to reach the agent, not just the brief: instruction is not enforcement when the repo is being worked in.
#[test]
fn a_read_only_gate_runs_its_reviewer_in_a_mode_that_cannot_write() {
    let repo = Repo::new();
    let records_argv = concat!(
        r#"echo "$AGENT_VERDICT_MODE" > mode-seen; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0""#
    );
    repo.declare_runner(
        records_argv,
        &[r#""$1" ro --read-only --doc rubric.md --path ."#],
    );
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 0, "{}", run.err);
    assert_eq!(repo.read("mode-seen").trim(), "plan", "{}", run.err);
}

// A reviewer runs headless, so it must never be in a position to ask: whatever a prompt would have covered, it decides alone.
#[test]
fn a_reviewer_is_never_left_in_a_mode_that_could_ask() {
    let repo = Repo::new();
    let records = concat!(
        r#"echo "$AGENT_VERDICT_MODE" >> modes-seen; "#,
        r#"echo "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0""#
    );
    repo.declare_runner(records, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 0, "{}", run.err);
    let seen = repo.read("modes-seen");
    assert!(seen.contains("dontAsk"), "{seen}");
    assert!(
        !seen.lines().any(str::is_empty),
        "a round ran with no mode: {seen}"
    );
}

// Two gates and one run: a caller that has to drive the tool gate by gate is one paying a round trip for something the tool already knows.
#[test]
fn one_run_reviews_every_gate_the_commit_reaches() {
    let repo = Repo::new();
    let second_gate = r#""$1" ann --doc rubric.md --path ."#;
    repo.declare(CLEAN, &[STANDARDS, second_gate]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.err.contains("standards:"), "{}", run.err);
    assert!(run.err.contains("ann:"), "{}", run.err);
    assert_eq!(repo.commit().code, 0);
    let message = repo.head_message();
    assert!(message.contains("Reviewed-standards:"), "{message}");
    assert!(message.contains("Reviewed-ann:"), "{message}");
}

// The gates after a MAJOR are reviewing content the author is about to change, so the run stops rather than paying for verdicts on text nobody is keeping.
#[test]
fn a_major_stops_the_run_before_the_gates_after_it() {
    let repo = Repo::new();
    let counting = concat!(
        r#"echo "$AGENT_VERDICT_SYSTEM" >> asked; "#,
        r#"if grep -q "ann" "$AGENT_VERDICT_SYSTEM"; then "#,
        r#"echo "VERDICT: reviewer=fake session=s major=0 moderate=0 minor=0"; else "#,
        r#"echo "VERDICT: reviewer=fake session=s major=1 moderate=0 minor=0"; fi"#
    );
    let second_gate = r#""$1" ann --doc rubric.md --path ."#;
    repo.declare_runner(counting, &[STANDARDS, second_gate]);
    repo.stage(&["src.rs"]);
    let run = repo.attest(AIM);
    assert_eq!(run.code, 1, "{}", run.err);
    assert!(
        !repo.read("asked").contains("ann"),
        "{}",
        repo.read("asked")
    );
}
