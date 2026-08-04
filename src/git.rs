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

fn nul_separated(bytes: Vec<u8>) -> Vec<String> {
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
    Ok(nul_separated(run(&args)?))
}

pub fn staged_existing(paths: &[String]) -> Result<Vec<String>, String> {
    let base = ["diff", "--cached", "--name-only", "-z", "--diff-filter=d"];
    let args = with_pathspec(&base, paths);
    Ok(nul_separated(run(&args)?))
}

// A literal pathspec matching nothing in the index is a typo; a glob is allowed to match nothing.
pub fn unmatched_literals(paths: &[String]) -> Result<Vec<String>, String> {
    let mut bad = Vec::new();
    for spec in paths {
        if spec.contains(['*', '?', '[', ':']) {
            continue;
        }
        let args = with_pathspec(&["ls-files", "-z"], std::slice::from_ref(spec));
        if nul_separated(run(&args)?).is_empty() {
            bad.push(spec.clone());
        }
    }
    Ok(bad)
}

// Repo-relative form of a doc, or None when it lives outside the worktree and can never be staged.
pub fn relative_to_root(doc: &str) -> Option<String> {
    let root = run(&["rev-parse", "--show-toplevel"]).ok()?;
    let root = std::path::Path::new(String::from_utf8_lossy(&root).trim())
        .canonicalize()
        .ok()?;
    let rest = std::path::Path::new(doc).strip_prefix(root).ok()?;
    Some(rest.to_string_lossy().into_owned())
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
