// Concern: what --prune-sessions finds of earlier runs, previews, and deletes | Non-concern: sessions a commit's own diary discards (tests/lifecycle.rs) | IO: (agent homes) -> paths, deletions

mod common;

use common::{Repo, STANDARDS};
use std::path::{Path, PathBuf};

const DAY_AND_A_HALF: std::time::Duration = std::time::Duration::from_secs(36 * 60 * 60);

fn left(path: &Path, lines: &[&str], aged: bool) -> PathBuf {
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("dir");
    std::fs::write(path, lines.join("\n") + "\n").expect("write");
    if aged {
        let then = std::time::SystemTime::now() - DAY_AND_A_HALF;
        let file = std::fs::File::options()
            .write(true)
            .open(path)
            .expect("open");
        file.set_modified(then).expect("age");
    }
    path.to_path_buf()
}

fn claude(repo: &Repo, id: &str, entrypoint: &str, prompt: &str, aged: bool) -> PathBuf {
    let record = serde_json::json!({"type": "user", "entrypoint": entrypoint, "message": {"role": "user", "content": prompt}});
    let path = repo
        .home()
        .join(".claude/projects/-elsewhere")
        .join(format!("{id}.jsonl"));
    left(
        &path,
        &[r#"{"type":"queue-operation"}"#, &record.to_string()],
        aged,
    )
}

fn codex(repo: &Repo, thread: &str, originator: &str, prompt: &str, aged: bool) -> PathBuf {
    let meta = serde_json::json!({"type": "session_meta", "payload": {"id": thread, "originator": originator}});
    let context = serde_json::json!({"type": "response_item", "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": "<environment_context>here</environment_context>"}]}});
    let asked = serde_json::json!({"type": "response_item", "payload": {"type": "message", "role": "user", "content": [{"type": "input_text", "text": prompt}]}});
    let path = repo
        .home()
        .join(".codex/sessions/2026/01/02")
        .join(format!("rollout-2026-01-02T03-04-05-{thread}.jsonl"));
    left(
        &path,
        &[&meta.to_string(), &context.to_string(), &asked.to_string()],
        aged,
    )
}

const OURS: &str =
    "<diff-intent>add the cache</diff-intent>\n\nExecute the review per the instructions above.\n";
const THREAD: &str = "01a1330d-35ec-7522-9371-755e554c8d2c";

struct Machine {
    repo: Repo,
    ours: Vec<PathBuf>,
    theirs: Vec<PathBuf>,
}

fn machine() -> Machine {
    let repo = Repo::new();
    // A stub on PATH, so a delete goes through codex and is seen to.
    repo.declare_codex("true", &[STANDARDS]);
    let ours = vec![
        claude(&repo, "a-review", "sdk-cli", OURS, true),
        codex(&repo, THREAD, "codex_exec", OURS, true),
    ];
    let theirs = vec![
        claude(&repo, "typed-by-hand", "cli", OURS, true),
        claude(
            &repo,
            "another-tool",
            "sdk-cli",
            "summarize this file",
            true,
        ),
        claude(&repo, "still-in-use", "sdk-cli", OURS, false),
        codex(
            &repo,
            "01a1330d-c617-7fa1-92a4-c26de74d9a1d",
            "codex-tui",
            OURS,
            true,
        ),
        codex(
            &repo,
            "01a1330d-c617-7fa1-92a4-000000000001",
            "codex_exec",
            "summarize this file",
            true,
        ),
        codex(
            &repo,
            "01a1330d-c617-7fa1-92a4-000000000002",
            "codex_exec",
            OURS,
            false,
        ),
    ];
    Machine { repo, ours, theirs }
}

fn listed(out: &str) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = out
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(_, path)| PathBuf::from(path))
        .collect();
    paths.sort();
    paths
}

#[test]
fn a_preview_lists_only_this_tools_settled_sessions_and_deletes_nothing() {
    let m = machine();
    let run = m.repo.capture(&["--prune-sessions"]);
    assert_eq!(run.code, 0, "{}", run.err);
    let mut ours = m.ours.clone();
    ours.sort();
    assert_eq!(listed(&run.out), ours, "{}", run.out);
    assert!(
        run.err.contains("--prune-sessions --confirm"),
        "{}",
        run.err
    );
    assert!(m.ours.iter().chain(&m.theirs).all(|path| path.exists()));
}

#[test]
fn confirming_deletes_this_tools_sessions_and_nothing_else() {
    let m = machine();
    let run = m.repo.capture(&["--prune-sessions", "--confirm"]);
    assert_eq!(run.code, 0, "{}", run.err);
    assert!(m.ours.iter().all(|path| !path.exists()), "{}", run.out);
    assert!(m.theirs.iter().all(|path| path.exists()));
    assert_eq!(m.repo.read("deleted"), format!("{THREAD}\n"));
}

const STRANDED: &str = "01a184cd-3444-74f3-8bf0-0ede781d3c9c";

// Codex's index as sqlite3 -json prints it: a stranded thread of ours, someone else's, and one whose rollout a file scan already finds.
fn indexed(m: &Machine) {
    let codex_home = m.repo.home().join(".codex");
    std::fs::write(codex_home.join("state_5.sqlite"), "").expect("index");
    let rows = serde_json::json!([
        {"id": STRANDED, "rollout_path": codex_home.join("sessions/gone.jsonl"), "updated_at": 1_700_000_000u64, "first_user_message": OURS},
        {"id": "01a184cd-0000-74f3-8bf0-000000000001", "rollout_path": codex_home.join("sessions/gone-too.jsonl"), "updated_at": 1_700_000_000u64, "first_user_message": "summarize this file"},
        {"id": THREAD, "rollout_path": &m.ours[1], "updated_at": 1_700_000_000u64, "first_user_message": OURS},
    ]);
    let stub = format!("#!/bin/sh\ncat <<'ROWS'\n{rows}\nROWS\n");
    let path = m.repo.dir.join("bin/sqlite3");
    std::fs::write(&path, stub).expect("stub");
    let mode = std::os::unix::fs::PermissionsExt::from_mode(0o755);
    std::fs::set_permissions(&path, mode).expect("chmod");
}

#[test]
fn a_thread_codex_still_indexes_without_its_rollout_is_found_and_deleted() {
    let m = machine();
    indexed(&m);
    let preview = m.repo.capture(&["--prune-sessions"]);
    assert_eq!(listed(&preview.out).len(), 3, "{}", preview.out);
    assert!(preview.out.contains("gone.jsonl"), "{}", preview.out);
    let run = m.repo.capture(&["--prune-sessions", "--confirm"]);
    assert_eq!(run.code, 0, "{}", run.err);
    let deleted = m.repo.read("deleted");
    assert!(deleted.lines().any(|t| t == STRANDED), "{deleted}");
    assert_eq!(deleted.lines().count(), 2, "{deleted}");
}

#[test]
fn a_delete_codex_refuses_is_reported_and_its_rollout_kept() {
    let m = machine();
    m.repo.write("delete-fails", "");
    let run = m.repo.capture(&["--prune-sessions", "--confirm"]);
    assert_eq!(run.code, 1, "{}", run.err);
    assert!(run.err.contains("database is locked"), "{}", run.err);
    assert!(m.ours[1].exists(), "the rollout went without codex");
}
