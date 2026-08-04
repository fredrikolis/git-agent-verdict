// Concern: the verdict trailer's grammar — its key, its fields, what makes one blocking | Non-concern: obtaining the trailer block, or reporting a rejection | IO: (gate, block) -> verdicts

pub struct Verdict {
    pub major: u32,
    pub moderate: u32,
    pub minor: u32,
    pub file: Option<String>,
}

impl Verdict {
    pub fn blocks(&self) -> bool {
        self.major > 0 || self.moderate > 0
    }
}

pub fn key_for(gate: &str) -> String {
    format!("Reviewed-{gate}")
}

// `file=` is terminal so a path may contain spaces; every other field is a whitespace-free token.
fn parse_value(value: &str) -> Result<Verdict, String> {
    let (fields, file) = match value.split_once("file=") {
        Some((head, path)) => (head, Some(path.trim().to_string())),
        None => (value, None),
    };
    if file.as_deref().is_some_and(|p| p.contains("major=")) {
        return Err("the counts must come before file=, which runs to end of line".to_string());
    }
    let mut reviewer = None;
    let mut counts: [Option<u32>; 3] = [None; 3];
    for field in fields.split_whitespace() {
        let (name, raw) = field
            .split_once('=')
            .ok_or_else(|| format!("field '{field}' is not name=value"))?;
        let slot = match name {
            "reviewer" => 3,
            "major" => 0,
            "moderate" => 1,
            "minor" => 2,
            _ => return Err(format!("unknown field '{name}'")),
        };
        // Last-wins would let `major=1 major=0` bury a declared blocker.
        if slot == 3 && reviewer.is_some() || slot < 3 && counts[slot].is_some() {
            return Err(format!("{name}= is given more than once"));
        }
        if slot == 3 {
            reviewer = Some(raw.to_string());
            continue;
        }
        counts[slot] = Some(
            raw.parse()
                .map_err(|_| format!("{name}={raw} is not a number"))?,
        );
    }
    if reviewer.is_none_or(|r| r.is_empty()) {
        return Err("no reviewer= named".to_string());
    }
    match counts {
        [Some(major), Some(moderate), Some(minor)] => Ok(Verdict {
            major,
            moderate,
            minor,
            file,
        }),
        _ => Err("major=, moderate= and minor= are all required".to_string()),
    }
}

pub fn parse_for(gate: &str, block: &str) -> Result<Vec<Verdict>, String> {
    let key = key_for(gate);
    let mut verdicts = Vec::new();
    for line in block.lines() {
        let Some(value) = line.strip_prefix(&key).and_then(|r| r.strip_prefix(':')) else {
            continue;
        };
        verdicts.push(parse_value(value.trim()).map_err(|e| format!("{key}: {e}"))?);
    }
    Ok(verdicts)
}

// git only parses a trailing paragraph, so a trailer written mid-body is invisible to it.
pub fn present_but_unparsed(gate: &str, raw: &str, block: &str) -> bool {
    let key = key_for(gate);
    raw.lines().any(|l| l.trim_start().starts_with(&key)) && !block.contains(&key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(block: &str) -> Result<Vec<Verdict>, String> {
        parse_for("standards", block)
    }

    #[test]
    fn accepts_a_clean_verdict() {
        let v = one("Reviewed-standards: reviewer=opus major=0 moderate=0 minor=3").unwrap();
        assert_eq!(v.len(), 1);
        assert!(!v[0].blocks());
        assert_eq!(v[0].minor, 3);
    }

    #[test]
    fn a_declared_blocker_blocks() {
        let v = one("Reviewed-standards: reviewer=opus major=1 moderate=0 minor=0").unwrap();
        assert!(v[0].blocks());
    }

    #[test]
    fn a_path_may_contain_spaces_because_file_is_terminal() {
        let v = one("Reviewed-standards: reviewer=opus major=0 moderate=0 minor=0 file=my file.rs")
            .unwrap();
        assert_eq!(v[0].file.as_deref(), Some("my file.rs"));
    }

    #[test]
    fn a_missing_count_is_rejected() {
        assert!(one("Reviewed-standards: reviewer=opus major=0 minor=0").is_err());
    }

    #[test]
    fn an_unnamed_reviewer_is_rejected() {
        assert!(one("Reviewed-standards: major=0 moderate=0 minor=0").is_err());
    }

    #[test]
    fn another_gates_trailer_is_not_this_gates() {
        assert!(
            one("Reviewed-prose: reviewer=opus major=0 moderate=0 minor=0")
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn a_trailer_outside_the_trailing_paragraph_is_detected() {
        let raw =
            "subject\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=0\n\nbody\n";
        assert!(present_but_unparsed("standards", raw, ""));
    }
}
