// Concern: the gate's decision against a real repo — what passes, what is refused, what exits 2 | Non-concern: the review that earns a trailer (tests/attest.rs) | IO: (temp repo, message) -> exit status

mod common;

use common::{Repo, BIN, DUMMY, STANDARDS};

#[test]
fn an_unattested_commit_fails_and_names_the_remedy() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards("subject\n\nbody\n");
    assert_eq!(code, 1, "{out}");
    assert!(
        out.contains("error: standards: no reviewable trailer"),
        "{out}"
    );
    assert!(out.contains("attest --repo "), "{out}");
    assert!(
        out.contains(&repo.root()),
        "the remedy names this repo: {out}"
    );
}

#[test]
fn a_trailer_whose_token_names_no_review_is_refused() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards(DUMMY);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error: standards: unknown token"), "{out}");
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
    assert!(out.contains("contradicts the review"), "{out}");
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
    assert!(out.contains("error: standards: declared blocker"), "{out}");
}

#[test]
fn a_trailer_with_no_token_is_malformed() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let msg = "subject\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=0\n";
    let (code, out) = repo.standards(msg);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("error: standards: malformed trailer"), "{out}");
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
fn staging_a_rubric_is_refused_as_maintenance() {
    let repo = Repo::new();
    repo.declare(DUMMY, &[STANDARDS]);
    repo.stage(&["src.rs", "rubric.md"]);
    let (code, out) = repo.standards(DUMMY);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("cannot be attested"), "{out}");
    assert!(out.contains("--no-verify"), "{out}");
}

#[test]
fn a_rubric_alone_is_refused_too() {
    let repo = Repo::new();
    repo.declare(DUMMY, &[STANDARDS]);
    repo.stage(&["rubric.md"]);
    let (code, out) = repo.standards(DUMMY);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("cannot be attested"), "{out}");
}

#[test]
fn another_gates_rubric_is_refused_as_well() {
    let repo = Repo::new();
    repo.write("style.md", "the other measure");
    let other = r#""$1" prose --simple --doc style.md --path "*.md""#;
    repo.declare(DUMMY, &[STANDARDS, other]);
    repo.stage(&["src.rs", "style.md"]);
    let (code, out) = repo.standards(DUMMY);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("style.md"), "{out}");
}

// git goes fatal on a pathspec it cannot place; an outside-repo rubric used to fail every commit for that reason.
#[test]
fn a_doc_outside_the_repo_does_not_reach_git() {
    let repo = Repo::new();
    let outside = repo.write_outside("standards.md", "the measure, kept elsewhere");
    let gate = format!(r#""$1" standards --doc {outside} --path "*.rs""#);
    repo.declare(DUMMY, &[&gate]);
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(DUMMY, &["standards", "--doc", &outside, "--path", "*.rs"]);
    assert!(!out.contains("fatal"), "{out}");
    assert!(!out.contains("outside repository"), "{out}");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("unknown token"), "{out}");
}

#[test]
fn a_gate_that_could_only_ever_meet_its_own_criteria_is_refused() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(
        DUMMY,
        &["meta", "--doc", "rubric.md", "--path", "rubric.md"],
    );
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("its own criteria"), "{out}");
    assert!(out.contains("Extend --path"), "{out}");
}

#[test]
fn a_gate_whose_pathspec_merely_includes_its_rubric_stands() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.run(DUMMY, &["meta", "--doc", "rubric.md", "--path", "*.md"]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn the_retired_rubric_guard_is_an_unknown_flag() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.bare(&["--rubric-guard", "--doc", "rubric.md"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("unknown flag '--rubric-guard'"), "{out}");
    assert!(out.contains("core.hooksPath"), "{out}");
}

#[test]
fn an_info_flag_among_a_gates_arguments_does_not_pass_the_gate() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    for stray in ["--version", "-V", "--help", "-h"] {
        let (code, out) = repo.run(
            DUMMY,
            &["standards", "--doc", "rubric.md", "--path", ".", stray],
        );
        assert_eq!(code, 2, "{stray}: {out}");
        assert!(out.contains("unknown flag"), "{stray}: {out}");
    }
}

// git parses a trailer key as one word; a gate named otherwise could never read its own trailer back.
#[test]
fn a_gate_name_that_cannot_form_a_trailer_key_is_refused() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    for name in ["my gate", "gate:two", ""] {
        let (code, out) = repo.run(DUMMY, &[name, "--doc", "rubric.md", "--path", "."]);
        assert_eq!(code, 2, "{name:?}: {out}");
        assert!(out.contains("a gate name is letters"), "{name:?}: {out}");
    }
}

#[test]
fn a_stale_declaration_prints_the_whole_setup_guide() {
    let repo = Repo::new();
    let (code, out) = repo.bare(&["MSG", "standards", "--doc", "rubric.md", "--nope"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("usage:"), "{out}");
    assert!(out.contains("core.hooksPath"), "{out}");
    assert!(out.contains("agent-verdict.runner"), "{out}");
}

const PASSES: &str = "VERDICT: reviewer=fake session=s-01 major=0 moderate=0 minor=0";

#[test]
fn attest_acts_on_the_named_repo_from_anywhere() {
    let repo = Repo::new();
    let elsewhere = Repo::new();
    repo.declare(PASSES, &[STANDARDS]);
    repo.stage(&["src.rs"]);
    let root = repo.root();
    let run = repo.capture_at(
        &elsewhere.dir,
        &[
            "attest",
            "--repo",
            &root,
            "--intent",
            "raise the staged file's line count",
        ],
    );
    assert_eq!(run.code, 0, "{}", run.err);
    // Awaited from where the caller stood, since a round is named by --repo and not by the shell.
    let waited = elsewhere.capture_at(&elsewhere.dir, &["await", "--repo", &root]);
    assert_eq!(waited.code, 0, "{}", waited.err);
    assert!(repo.round_logs().contains("standards:"), "{}", waited.err);
    assert!(!elsewhere.committed(), "it acted on the shell's repo");
}

#[test]
fn a_missing_repo_offers_no_value_to_paste() {
    let repo = Repo::new();
    repo.declare(PASSES, &[STANDARDS]);
    let (code, out) = repo.bare(&["attest", "--intent", "an aim"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("shell's directory is not consulted"), "{out}");
    assert!(!out.contains(&repo.root()), "it suggested a path: {out}");
}

#[test]
fn a_relative_repo_is_refused() {
    let repo = Repo::new();
    repo.declare(PASSES, &[STANDARDS]);
    let (code, out) = repo.bare(&["attest", "--repo", ".", "--intent", "an aim"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("must be absolute"), "{out}");
}

#[test]
fn a_repo_that_is_not_the_root_is_refused() {
    let repo = Repo::new();
    repo.declare(PASSES, &[STANDARDS]);
    let inside = format!("{}/sub", repo.root());
    std::fs::create_dir_all(&inside).expect("subdir");
    let (code, out) = repo.bare(&["attest", "--repo", &inside, "--intent", "an aim"]);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("is not a repo root"), "{out}");
    assert!(out.contains(&repo.root()), "{out}");
}

#[test]
fn a_mistyped_agent_verb_gets_the_usage_and_not_the_guide() {
    let repo = Repo::new();
    for args in [
        vec!["attest", "--intent", "an aim", "--simple"],
        vec!["attest", "--doc", "rubric.md"],
        vec!["reset"],
    ] {
        let (code, out) = repo.bare(&args);
        assert_eq!(code, 2, "{args:?}: {out}");
        assert!(out.contains("usage:"), "{args:?}: {out}");
        assert!(!out.contains("core.hooksPath"), "{args:?}: {out}");
    }
}

#[test]
fn the_setup_guide_answers_outside_a_repo() {
    let out = std::process::Command::new(BIN)
        .current_dir(std::env::temp_dir())
        .arg("--repo-setup-guide")
        .output()
        .expect("binary runs");
    let text = String::from_utf8_lossy(&out.stdout).into_owned();
    assert_eq!(out.status.code(), Some(0), "{text}");
    assert!(text.contains("core.hooksPath .githooks"), "{text}");
    assert!(text.contains("agent-verdict.runner"), "{text}");
    assert!(text.contains("attest --repo"), "{text}");
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

#[test]
fn a_pin_on_another_line_is_refused_in_both_directions() {
    for want in ["0.3.0", "0.1", "1.15.0", "99.0.0"] {
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

#[test]
fn a_shipped_standard_reaches_the_brief_without_a_file_in_the_repo() {
    let repo = Repo::new();
    repo.declare(
        "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0",
        &[r#""$1" std --standard programming --path ."#],
    );
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["--reviewer-prompt", "std"]);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(
        run.out.contains("<document title=\"programming\">"),
        "{}",
        run.out
    );
    // Against the file itself, not a copied fragment, so it can never drift from the real prose.
    let shipped = include_str!("../standards/programming.md");
    assert!(run.out.contains(shipped), "{}", run.out);
}

#[test]
fn an_unknown_standard_is_refused_with_the_list_of_shipped_ones() {
    let repo = Repo::new();
    let (code, said) = repo.run("subject", &["std", "--standard", "nonesuch", "--path", "."]);
    assert_eq!(code, 2, "{said}");
    assert!(said.contains("this build ships"), "{said}");
    assert!(said.contains("programming"), "{said}");
}

#[test]
fn a_standard_alone_is_enough_to_declare_a_gate() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, said) = repo.run("subject", &["std", "--standard", "testing", "--path", "."]);
    assert_ne!(code, 2, "{said}");
    assert!(!said.contains("at least one"), "{said}");
}

// "Concern:" comes from each standard's own first-line annotation, not a second description that could go stale.
#[test]
fn the_shipped_standards_can_be_listed_and_read() {
    let repo = Repo::new();
    let listed = repo.capture(&["--standards"]);
    assert_eq!(listed.code, 0, "{}", listed.err);
    assert!(listed.out.contains("programming"), "{}", listed.out);
    assert!(listed.out.contains("human-communication"), "{}", listed.out);
    assert!(listed.out.contains("Concern:"), "{}", listed.out);

    let read = repo.capture(&["--standards", "testing"]);
    assert_eq!(read.code, 0, "{}", read.err);
    assert!(read.out.len() > 2000, "{}", read.out.len());

    let unknown = repo.capture(&["--standards", "nonesuch"]);
    assert_eq!(unknown.code, 2, "{}", unknown.err);
    assert!(unknown.err.contains("this build ships"), "{}", unknown.err);
}

#[test]
fn a_read_only_gate_tells_its_reviewer_it_cannot_write() {
    let repo = Repo::new();
    repo.declare(
        "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0",
        &[r#""$1" ro --read-only --doc rubric.md --path ."#],
    );
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["--reviewer-prompt", "ro"]);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(run.out.contains("cannot write anywhere"), "{}", run.out);
    assert!(!run.out.contains("copy the repo to a temp"), "{}", run.out);
}

#[test]
fn a_normal_gate_still_offers_its_reviewer_a_sandbox() {
    let repo = Repo::new();
    repo.declare(
        "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0",
        &[r#""$1" rw --doc rubric.md --path ."#],
    );
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["--reviewer-prompt", "rw"]);
    assert!(run.out.contains("copy the repo to a temp"), "{}", run.out);
    assert!(!run.out.contains("cannot write anywhere"), "{}", run.out);
}

#[test]
fn a_rule_carries_the_multiline_output_of_a_command() {
    let repo = Repo::new();
    let generated = concat!(
        r#""$1" gen --rule "$(printf 'first line\nsecond\tline\nthird')" --path .; "#,
        r#"true"#
    );
    repo.declare(
        "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0",
        &[generated],
    );
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["--reviewer-prompt", "gen"]);
    assert_eq!(run.code, 0, "{}", run.err);
    // Whole, and still one gate: a raw newline would have split the declaration and lost everything after it.
    assert!(
        run.out.contains("first line\nsecond\tline\nthird"),
        "{}",
        run.out
    );
    assert!(run.out.contains("<inline-rule-1>"), "{}", run.out);
}

#[test]
fn a_rule_reads_stdin_so_a_commands_output_has_no_size_limit() {
    let repo = Repo::new();
    // Over the 128 KiB per-argument cap: as an argument this would fail the exec outright.
    let big = "x".repeat(200_000);
    let heredoc = format!("\"$1\" gen --rule - --path . <<'RULE'\nfirst\nsecond\n{big}\nRULE");
    repo.declare(
        "VERDICT: reviewer=fake session=s-1 major=0 moderate=0 minor=0",
        &[&heredoc],
    );
    repo.stage(&["src.rs"]);
    let run = repo.capture(&["--reviewer-prompt", "gen"]);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(
        run.out.contains("<inline-rule-1>first\nsecond\n"),
        "{}",
        &run.out[..run.out.len().min(400)]
    );
    assert!(
        run.out.len() > 200_000,
        "the rule was truncated: {}",
        run.out.len()
    );
}
