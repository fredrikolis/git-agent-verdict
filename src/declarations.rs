// Concern: the hook's declaration line, written and read back | Non-concern: judging a declaration, or briefing anyone from it | IO: (hook) -> gates

use crate::cli::{Brief, Invocation};
use crate::git;

// Set while the hook is re-run to enumerate itself: every mode prints its declaration and exits instead of acting, so the list is read from the hook rather than retyped beside it.
const LIST_ENV: &str = "GIT_AGENT_VERDICT_LIST";

pub struct Declaration {
    pub gate: String,
    pub standards: Vec<String>,
    pub docs: Vec<String>,
    pub rules: Vec<String>,
    pub paths: Vec<String>,
    // Which model reviews this gate, as the repo asked for it: the intensity a gate is worth is the repo's call, not one this tool makes for it.
    pub model: Option<String>,
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
    if let Some(model) = &inv.model {
        fields.push(format!("model={model}"));
    }
    if let Some(path) = &inv.brief.prompt {
        fields.push(format!("prompt={path}"));
    }
    fields.extend(inv.standards.iter().map(|s| format!("standard={s}")));
    fields.extend(inv.docs.iter().map(|d| format!("doc={d}")));
    fields.extend(inv.rules.iter().map(|r| format!("rule={r}")));
    fields.extend(inv.paths.iter().map(|p| format!("path={p}")));
    println!("{}", fields.join("\t"));
}

fn read_gate(gate: &str, fields: std::str::Split<'_, char>) -> Option<Declaration> {
    let mut declaration = Declaration {
        gate: gate.to_string(),
        standards: Vec::new(),
        docs: Vec::new(),
        rules: Vec::new(),
        paths: Vec::new(),
        model: None,
        brief: Brief::default(),
    };
    for field in fields {
        if let Some(name) = field.strip_prefix("standard=") {
            declaration.standards.push(name.to_string());
        } else if let Some(doc) = field.strip_prefix("doc=") {
            declaration.docs.push(doc.to_string());
        } else if let Some(text) = field.strip_prefix("rule=") {
            declaration.rules.push(text.to_string());
        } else if let Some(path) = field.strip_prefix("path=") {
            declaration.paths.push(path.to_string());
        } else if let Some(name) = field.strip_prefix("model=") {
            declaration.model = Some(name.to_string());
        } else if let Some(path) = field.strip_prefix("prompt=") {
            declaration.brief.prompt = Some(path.to_string());
        } else if field == "simple" {
            declaration.brief.simple = true;
        }
    }
    // A line with no measure is not a gate: the hook's own version check and any command it runs beside them print nothing here.
    if declaration.docs.is_empty()
        && declaration.rules.is_empty()
        && declaration.standards.is_empty()
    {
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
    // What the hook said while failing is the whole diagnosis; without it the reader is told only that a hook they can see declares gates declares none.
    let said = String::from_utf8_lossy(&out.stderr).trim().to_string();
    if hook.gates.is_empty() {
        if said.is_empty() {
            return Err(format!("{} declared no gates", hook.path));
        }
        return Err(format!("{} declared no gates; it said: {said}", hook.path));
    }
    // Every declaration prints its line and exits 0 while enumerating, so anything on stderr is one that refused. Read past it and the gate it refused for is simply absent from the listing — a repo one gate lighter than the hook says, and nothing saying so.
    if !said.is_empty() {
        return Err(format!(
            "{}: a declaration in it was refused, so what it gates by cannot be read:\n{said}",
            hook.path
        ));
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
