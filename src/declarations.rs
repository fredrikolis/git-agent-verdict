// Concern: the hook's declaration line, written and read back | Non-concern: judging a declaration, or briefing anyone from it | IO: (hook) -> gates

use crate::cli::{Brief, Invocation};
use crate::git;

// Set while the hook is re-run to enumerate itself: every mode prints its declaration and exits instead of acting, so the list is read from the hook rather than retyped beside it.
const LIST_ENV: &str = "GIT_AGENT_VERDICT_LIST";

pub struct Declaration {
    pub gate: String,
    pub docs: Vec<String>,
    pub paths: Vec<String>,
    pub brief: Brief,
}

pub fn listing_requested() -> bool {
    std::env::var_os(LIST_ENV).is_some()
}

// Named fields, so a gate that declares no override still lists its docs unambiguously.
pub fn emit_gate(inv: &Invocation) {
    let mut fields = vec![inv.gate.clone()];
    if inv.brief.simple {
        fields.push("simple".to_string());
    }
    if let Some(path) = &inv.brief.prompt {
        fields.push(format!("prompt={path}"));
    }
    fields.extend(inv.docs.iter().map(|d| format!("doc={d}")));
    fields.extend(inv.paths.iter().map(|p| format!("path={p}")));
    println!("{}", fields.join("\t"));
}

fn read_gate(gate: &str, fields: std::str::Split<'_, char>) -> Option<Declaration> {
    let mut declaration = Declaration {
        gate: gate.to_string(),
        docs: Vec::new(),
        paths: Vec::new(),
        brief: Brief::default(),
    };
    for field in fields {
        if let Some(doc) = field.strip_prefix("doc=") {
            declaration.docs.push(doc.to_string());
        } else if let Some(path) = field.strip_prefix("path=") {
            declaration.paths.push(path.to_string());
        } else if let Some(path) = field.strip_prefix("prompt=") {
            declaration.brief.prompt = Some(path.to_string());
        } else if field == "simple" {
            declaration.brief.simple = true;
        }
    }
    // A line with no doc is not a gate: the hook's own version check and any command it runs beside them print nothing here.
    if declaration.docs.is_empty() {
        return None;
    }
    Some(declaration)
}

pub struct Hook {
    pub path: String,
    pub gates: Vec<Declaration>,
}

// Re-running the hook is what resolves `$KB/foo.md` and friends: the shell expands them, where reading the hook as text could not.
pub fn read() -> Result<Hook, String> {
    let path = git::hook_path()?;
    let out = std::process::Command::new(&path)
        .arg("/dev/null")
        .env(LIST_ENV, "1")
        .output()
        .map_err(|e| format!("cannot run {path}: {e}"))?;
    let listing = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut hook = Hook {
        path,
        gates: Vec::new(),
    };
    for line in listing.lines() {
        let mut fields = line.split('\t');
        let Some(head) = fields.next() else { continue };
        if let Some(gate) = read_gate(head, fields) {
            hook.gates.push(gate);
        }
    }
    if hook.gates.is_empty() {
        return Err(format!("{} declared no gates", hook.path));
    }
    Ok(hook)
}

pub fn find<'a>(hook: &'a Hook, want: &str) -> Result<&'a Declaration, String> {
    hook.gates.iter().find(|d| d.gate == want).ok_or_else(|| {
        let declared: Vec<&str> = hook.gates.iter().map(|d| d.gate.as_str()).collect();
        format!(
            "no gate '{want}' in {}; it declares: {}",
            hook.path,
            declared.join(", ")
        )
    })
}
