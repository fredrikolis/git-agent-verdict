// Concern: the facts taken from git — staged paths, index membership, parsed trailers | Non-concern: judging them | IO: (pathspec, message file) -> paths, trailer block

use std::process::Command;

fn run(args: &[&str]) -> Result<Vec<u8>, String> {
    let out = Command::new("git")
        .args(["-c", "core.quotePath=false"])
        .args(args)
        .output()
        .map_err(|e| format!("cannot run git: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr).trim().to_string();
        return Err(format!("git {}: {err}", args.join(" ")));
    }
    Ok(out.stdout)
}

fn nul_separated(bytes: &[u8]) -> Vec<String> {
    bytes
        .split(|b| *b == 0)
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).into_owned())
        .collect()
}

fn with_pathspec<'a>(base: &[&'a str], paths: &'a [String]) -> Vec<&'a str> {
    let mut args: Vec<&str> = base.to_vec();
    args.push("--");
    args.extend(paths.iter().map(String::as_str));
    args
}

pub fn staged(paths: &[String]) -> Result<Vec<String>, String> {
    let args = with_pathspec(&["diff", "--cached", "--name-only", "-z"], paths);
    Ok(nul_separated(&run(&args)?))
}

// The worktree against the index, where staged() asks the index against HEAD. A file that differs here is one the reviewer opens and the commit will not carry.
pub fn unstaged(paths: &[String]) -> Result<Vec<String>, String> {
    let args = with_pathspec(&["diff", "--name-only", "-z"], paths);
    Ok(nul_separated(&run(&args)?))
}

// Every file a gate reaches, committed rather than changed: what `audit` reviews, where `staged` is what `attest` reviews.
pub fn tracked(paths: &[String]) -> Result<Vec<String>, String> {
    let args = with_pathspec(&["ls-files", "-z"], paths);
    Ok(nul_separated(&run(&args)?))
}

// A literal pathspec matching nothing in the index is a typo; a glob is allowed to match nothing.
pub fn unmatched_literals(paths: &[String]) -> Result<Vec<String>, String> {
    let mut bad = Vec::new();
    for spec in paths {
        if spec.contains(['*', '?', '[', ':']) {
            continue;
        }
        let args = with_pathspec(&["ls-files", "-z"], std::slice::from_ref(spec));
        if nul_separated(&run(&args)?).is_empty() {
            bad.push(spec.clone());
        }
    }
    Ok(bad)
}

pub fn toplevel() -> Result<String, String> {
    let out = run(&["rev-parse", "--show-toplevel"])?;
    Ok(String::from_utf8_lossy(&out).trim().to_string())
}

// Repo-relative form of a doc, or None when it lives outside the worktree and can never be staged.
pub fn relative_to_root(doc: &str) -> Option<String> {
    let root = std::path::Path::new(&toplevel().ok()?)
        .canonicalize()
        .ok()?;
    let rest = std::path::Path::new(doc).strip_prefix(root).ok()?;
    Some(rest.to_string_lossy().into_owned())
}

// Resolved through git so `core.hooksPath` is honoured; a repo that relocates its hooks still works.
pub fn hook_path() -> Result<String, String> {
    let out = run(&["rev-parse", "--git-path", "hooks/commit-msg"])?;
    let path = String::from_utf8_lossy(&out).trim().to_string();
    if std::path::Path::new(&path).exists() {
        return Ok(path);
    }
    Err(format!("no commit-msg hook at {path}"))
}

// Read through git so the whole precedence ladder applies: --global is the machine's default and --local overrides it per clone, neither of which a repo can commit for its maintainers.
pub fn config(key: &str) -> Option<String> {
    let out = run(&["config", "--get", key]).ok()?;
    let value = String::from_utf8_lossy(&out).trim().to_string();
    Some(value).filter(|v| !v.is_empty())
}

// Resolved through git so a worktree or a submodule lands in its own git dir rather than the superproject's.
pub fn git_path(relative: &str) -> Result<std::path::PathBuf, String> {
    let out = run(&["rev-parse", "--git-path", relative])?;
    let path = String::from_utf8_lossy(&out).trim().to_string();
    std::path::Path::new(&path)
        .canonicalize()
        .or_else(|_| std::env::current_dir().map(|cwd| cwd.join(&path)))
        .map_err(|e| format!("cannot resolve {path}: {e}"))
}

// The commit under review does not exist yet, so state is keyed on what it will be committed onto. An unborn HEAD has no sha and gets a name no sha can collide with.
pub fn head_sha() -> String {
    match run(&["rev-parse", "HEAD"]) {
        Ok(out) => String::from_utf8_lossy(&out).trim().to_string(),
        Err(_) => "unborn".to_string(),
    }
}

// Written through a file rather than -m so the message reaches git byte for byte, and committed with hooks live: the gate then verifies this message like any other.
pub fn commit(message: &str) -> Result<String, String> {
    let path = git_path("AGENT_VERDICT_MSG")?;
    std::fs::write(&path, message).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    let file = path.to_string_lossy().into_owned();
    let out = run(&["commit", "-F", &file]);
    let _ = std::fs::remove_file(&path);
    Ok(String::from_utf8_lossy(&out?).into_owned())
}

// Confirmed from git's own state, so a hand-typed subject cannot forge the exemption.
pub fn in_progress(marker: &str) -> bool {
    let Ok(out) = run(&["rev-parse", "--git-path", marker]) else {
        return false;
    };
    std::path::Path::new(String::from_utf8_lossy(&out).trim()).exists()
}

pub fn trailers(msg_file: &str) -> Result<String, String> {
    let out = run(&["interpret-trailers", "--parse", msg_file])?;
    Ok(String::from_utf8_lossy(&out).into_owned())
}
