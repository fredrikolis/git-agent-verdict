// Concern: the reviewer block a gate hands out — what it says, and where its declaration is read from | Non-concern: whether a commit passes (tests/gate.rs) | IO: (temp repo, hook) -> stdout

mod common;

use common::{Repo, BIN, CLEAN};

#[test]
fn the_prompt_demands_an_intent_and_says_how_to_judge_it() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards("subject\n\nbody\n");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("INTENT:"), "{out}");
    assert!(
        out.contains("Judge that INTENT before anything else"),
        "{out}"
    );
    assert!(out.contains("Scope is not your question"), "{out}");
}

// One look, and only major= blocks: a reviewer briefed to expect a second pass holds findings back for it.
#[test]
fn the_default_ladder_blocks_on_major_alone_and_promises_no_second_pass() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let (code, out) = repo.standards("subject\n\nbody\n");
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("ONLY rung that blocks"), "{out}");
    assert!(out.contains("no re-review"), "{out}");
    assert!(out.contains("major=0 moderate=<n> minor=<n>"), "{out}");
}

#[test]
fn a_simple_gate_briefs_its_reviewer_that_nothing_it_finds_blocks() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let args = ["look", "--simple", "--doc", "rubric.md", "--path", "."];
    let (code, out) = repo.run("subject\n\nbody\n", &args);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("ADVISORY"), "{out}");
    assert!(!out.contains("ONLY rung that blocks"), "{out}");
    // No count is demanded, so the shape holds no zero to copy.
    assert!(out.contains("major=<n> moderate=<n> minor=<n>"), "{out}");
}

#[test]
fn an_override_prompt_replaces_the_built_in_block_verbatim() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    repo.write("brief.md", "MY OWN BRIEF for {{gate}} against:\n{{docs}}\n");
    let args = [
        "standards",
        "--override-prompt",
        "brief.md",
        "--doc",
        "rubric.md",
        "--path",
        ".",
    ];
    let (code, out) = repo.run("subject\n\nbody\n", &args);
    assert_eq!(code, 1, "{out}");
    assert!(out.contains("MY OWN BRIEF for standards"), "{out}");
    assert!(out.contains("rubric.md"), "{out}");
    // Its first line is content, not an annotation to strip, and nothing of the built-in survives.
    assert!(!out.contains("NEUTRAL REVIEW"), "{out}");
    assert!(!out.contains("THE LADDER"), "{out}");
}

// Exit 2, not a quiet fall back to the built-in: a repo that thinks it overrode the brief and did not is the one case this flag must never produce.
#[test]
fn an_override_prompt_that_does_not_exist_fails_loudly() {
    let repo = Repo::new();
    repo.stage(&["src.rs"]);
    let args = [
        "standards",
        "--override-prompt",
        "nope.md",
        "--doc",
        "rubric.md",
        "--path",
        ".",
    ];
    let (code, out) = repo.run(CLEAN, &args);
    assert_eq!(code, 2, "{out}");
    assert!(out.contains("--override-prompt"), "{out}");
}

#[test]
fn reviewer_prompt_reads_its_docs_from_the_hook() {
    let repo = Repo::new();
    repo.hook(&["standards --doc rubric.md --path ."]);

    let (code, text, err) = repo.reviewer_prompt("standards");
    assert_eq!(code, 0, "{err}");
    assert!(text.contains("NEUTRAL REVIEW — gate: standards"), "{text}");
    assert!(text.contains("INTENT:"), "{text}");
    assert!(text.contains("rubric.md"), "{text}");

    let (code, _, err) = repo.reviewer_prompt("nope");
    assert_eq!(code, 2, "{err}");
    assert!(err.contains("it declares: standards"), "{err}");
}

// The whole brief is read back, not just the docs: a block that describes a review its gate is not running briefs the reviewer against the wrong bar.
#[test]
fn reviewer_prompt_inherits_simple_and_the_override_from_the_hook() {
    let repo = Repo::new();
    repo.write("brief.md", "MY OWN BRIEF for {{gate}}\n");
    repo.hook(&[
        "look --simple --doc rubric.md --path .",
        "standards --override-prompt brief.md --doc rubric.md --path .",
    ]);

    let (code, text, err) = repo.reviewer_prompt("look");
    assert_eq!(code, 0, "{err}");
    assert!(text.contains("ADVISORY"), "{text}");
    assert!(!text.contains("ONLY rung that blocks"), "{text}");

    let (code, text, err) = repo.reviewer_prompt("standards");
    assert_eq!(code, 0, "{err}");
    assert!(text.starts_with("MY OWN BRIEF for standards"), "{text}");
    assert!(!text.contains("NEUTRAL REVIEW"), "{text}");
}

// An override that opts back into the ladder gets the one its gate declared, which is why substitution runs at all on a file the tool did not write.
#[test]
fn an_override_prompt_still_renders_the_ladder_its_gate_declared() {
    let repo = Repo::new();
    repo.write("brief.md", "BRIEF\n{{ladder}}\n");
    repo.hook(&["look --simple --override-prompt brief.md --doc rubric.md --path ."]);

    let (code, text, err) = repo.reviewer_prompt("look");
    assert_eq!(code, 0, "{err}");
    assert!(text.contains("THE LADDER"), "{text}");
    assert!(text.contains("ADVISORY"), "{text}");
}

#[test]
fn reviewer_prompt_refuses_the_gate_mode_flags() {
    let repo = Repo::new();
    for extra in [
        vec!["--path", "."],
        vec!["--per-file"],
        vec!["--doc", "rubric.md"],
        vec!["--simple"],
        vec!["--override-prompt", "rubric.md"],
        vec!["MSG"],
    ] {
        let mut args = vec!["--reviewer-prompt", "standards"];
        args.extend(extra.iter());
        let out = std::process::Command::new(BIN)
            .current_dir(&repo.dir)
            .args(&args)
            .output()
            .expect("binary runs");
        assert_eq!(out.status.code(), Some(2), "{args:?}");
    }
}
