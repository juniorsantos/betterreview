use std::collections::BTreeMap;

use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlameLine {
    pub author: String,
    pub age: String,
}

const UNCOMMITTED: &str = "0000000000000000000000000000000000000000";

pub fn parse_blame(output: &str, now: OffsetDateTime) -> BTreeMap<u32, BlameLine> {
    let mut lines = BTreeMap::new();
    let mut line_number: Option<u32> = None;
    let mut author: Option<String> = None;
    let mut when: Option<i64> = None;
    let mut uncommitted = false;

    for row in output.lines() {
        if let Some(rest) = row.strip_prefix("author ") {
            author = Some(rest.trim().to_owned());
            continue;
        }
        if let Some(rest) = row.strip_prefix("author-time ") {
            when = rest.trim().parse().ok();
            continue;
        }
        if row.starts_with('\t') {
            if let (Some(number), Some(name)) = (line_number.take(), author.take()) {
                let age = if uncommitted {
                    "uncommitted".to_owned()
                } else {
                    when.map_or_else(|| "?".to_owned(), |seconds| age(seconds, now))
                };
                lines.insert(number, BlameLine { author: name, age });
            }
            when = None;
            uncommitted = false;
            continue;
        }
        let mut parts = row.split_whitespace();
        let Some(sha) = parts.next() else { continue };
        if !sha.chars().all(|value| value.is_ascii_hexdigit()) {
            continue;
        }
        let Some(number) = parts.nth(1).and_then(|value| value.parse().ok()) else {
            continue;
        };
        uncommitted = sha.trim_start_matches('0').is_empty() && sha.len() == UNCOMMITTED.len();
        line_number = Some(number);
    }
    lines
}

fn age(seconds: i64, now: OffsetDateTime) -> String {
    let elapsed = (now.unix_timestamp() - seconds).max(0);
    let days = elapsed / 86_400;
    match days {
        0 => "today".to_owned(),
        1..=6 => format!("{days}d"),
        7..=29 => format!("{}w", days / 7),
        30..=364 => format!("{}mo", days / 30),
        _ => format!("{}y", days / 365),
    }
}

pub async fn load(
    runner: &dyn crate::process::CommandRunner,
    path: &crate::domain::RepoPath,
    revision: &crate::domain::CommitOid,
) -> Result<BTreeMap<u32, BlameLine>, String> {
    let output = runner
        .run(crate::process::CommandSpec {
            program: std::path::PathBuf::from("git"),
            args: vec![
                "blame".into(),
                "--line-porcelain".into(),
                revision.as_ref().into(),
                "--".into(),
                path.0.clone().into(),
            ],
            stdin: None,
            cwd: None,
            timeout: std::time::Duration::from_secs(30),
            env: Default::default(),
            env_remove: Vec::new(),
        })
        .await
        .map_err(|error| error.to_string())?;
    if output.status != 0 {
        return Err(format!(
            "blame needs the base commit in this clone: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(parse_blame(
        &String::from_utf8_lossy(&output.stdout),
        OffsetDateTime::now_utc(),
    ))
}
