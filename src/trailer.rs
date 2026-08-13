// Concern: the verdict trailer's grammar — its key, its fields, what makes one blocking | Non-concern: obtaining the trailer block, or reporting a rejection | IO: (gate, block) -> verdicts

// One ladder for every gate: an advisory one has no MAJOR rung and reports zero. A count that cannot reach zero gives a review no place to stop.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub major: u32,
    pub moderate: u32,
    pub minor: u32,
}

// The session is evidence, not a claim: it names a transcript on one machine, so it is kept in the diary beside the counts and never published into a message.
pub struct Verdict {
    pub reviewer: String,
    pub counts: Counts,
    pub token: String,
    pub resets: u32,
    pub session: String,
}

impl Counts {
    // Written the one way the grammar defines them, wherever they are read back: a trailer, a total, or a line an author is shown.
    pub fn render(self) -> String {
        let Counts {
            major,
            moderate,
            minor,
        } = self;
        format!("major={major} moderate={moderate} minor={minor}")
    }
}

impl Verdict {
    // major= alone. A MODERATE is fixed without a second look, so its count records what the reviewer found, not what is left outstanding — blocking on it would demand a re-review that no longer happens.
    pub fn blocks(&self) -> bool {
        self.counts.major > 0
    }
}

pub fn total(verdicts: &[Verdict]) -> Counts {
    verdicts.iter().map(|v| v.counts).fold(
        Counts {
            major: 0,
            moderate: 0,
            minor: 0,
        },
        |a, b| Counts {
            major: a.major.saturating_add(b.major),
            moderate: a.moderate.saturating_add(b.moderate),
            minor: a.minor.saturating_add(b.minor),
        },
    )
}

// The shape a rejection shows the author, and the shape a blocking gate demands back.
pub const COUNTS_SHAPE: &str = "major=<n> moderate=<n> minor=<n>";

// An advisory gate is never asked for major=: it has no MAJOR rung, and the tool records the zero rather than asking a reviewer to type a constant.
pub const ADVISORY_SHAPE: &str = "moderate=<n> minor=<n>";

pub fn key_for(gate: &str) -> String {
    format!("Reviewed-{gate}")
}

fn counts_from(slots: [Option<u32>; 3]) -> Result<Counts, String> {
    match slots {
        [Some(major), Some(moderate), Some(minor)] => Ok(Counts {
            major,
            moderate,
            minor,
        }),
        _ => Err("counts are major=, moderate= and minor= together".to_string()),
    }
}

fn slot_of(name: &str) -> Option<usize> {
    match name {
        "major" => Some(0),
        "moderate" => Some(1),
        "minor" => Some(2),
        "resets" => Some(3),
        _ => None,
    }
}

struct Fields {
    reviewer: Option<String>,
    token: Option<String>,
    numbers: [Option<u32>; 4],
}

fn read_field(f: &mut Fields, field: &str) -> Result<(), String> {
    let (name, raw) = field
        .split_once('=')
        .ok_or_else(|| format!("field '{field}' is not name=value"))?;
    let taken = match name {
        "reviewer" => f.reviewer.is_some(),
        "token" => f.token.is_some(),
        other => slot_of(other).is_some_and(|s| f.numbers[s].is_some()),
    };
    // Last-wins would let `major=1 major=0` bury a declared blocker.
    if taken {
        return Err(format!("{name}= is given more than once"));
    }
    match name {
        "reviewer" => f.reviewer = Some(raw.to_string()),
        "token" => f.token = Some(raw.to_string()),
        other => {
            let slot = slot_of(other).ok_or_else(|| format!("unknown field '{other}'"))?;
            let value = raw
                .parse()
                .map_err(|_| format!("{name}={raw} is not a number"))?;
            f.numbers[slot] = Some(value);
        }
    }
    Ok(())
}

fn parse_value(value: &str) -> Result<Verdict, String> {
    let mut fields = Fields {
        reviewer: None,
        token: None,
        numbers: [None; 4],
    };
    for field in value.split_whitespace() {
        read_field(&mut fields, field)?;
    }
    let reviewer = fields
        .reviewer
        .filter(|r| !r.is_empty())
        .ok_or("no reviewer= named")?;
    let token = fields
        .token
        .filter(|t| !t.is_empty())
        .ok_or("no token=: it is issued by `git agent-verdict attest`")?;
    let [major, moderate, minor, resets] = fields.numbers;
    Ok(Verdict {
        reviewer,
        counts: counts_from([major, moderate, minor])?,
        token,
        resets: resets.unwrap_or(0),
        session: String::new(),
    })
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

// The one place the grammar is written rather than read, so `attest` hands back a line this file will accept.
pub fn render(gate: &str, verdict: &Verdict) -> String {
    let counts = verdict.counts.render();
    let resets = match verdict.resets {
        0 => String::new(),
        n => format!(" resets={n}"),
    };
    let key = key_for(gate);
    format!(
        "{key}: reviewer={} {counts} token={}{resets}",
        verdict.reviewer, verdict.token
    )
}

// Matched on the address, not the name: a human co-author called Claude keeps their credit.
pub fn is_agent_coauthor(line: &str) -> bool {
    let Some((key, value)) = line.split_once(':') else {
        return false;
    };
    key.eq_ignore_ascii_case("co-authored-by") && value.contains("@anthropic.com")
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

    // The edge cases the end-to-end gate tests cannot reach cheaply: everything else about the grammar is frozen there instead.
    #[test]
    fn a_repeated_count_cannot_bury_a_blocker() {
        let line = "Reviewed-standards: reviewer=opus major=1 major=0 moderate=0 minor=0 token=ab";
        assert!(one(line).is_err());
    }

    #[test]
    fn a_trailer_missing_a_rung_is_refused() {
        let line = "Reviewed-standards: reviewer=opus major=0 moderate=0 token=ab";
        assert!(one(line).is_err());
    }

    #[test]
    fn another_gates_trailer_is_not_this_gates() {
        let line = "Reviewed-prose: reviewer=opus major=0 moderate=0 minor=0 token=ab";
        assert!(one(line).unwrap().is_empty());
    }

    #[test]
    fn a_trailer_outside_the_trailing_paragraph_is_detected() {
        let raw = "subject\n\nReviewed-standards: reviewer=opus major=0 moderate=0 minor=0 token=ab\n\nbody\n";
        assert!(present_but_unparsed("standards", raw, ""));
    }
}
