use betterreview::blame::{BlameLine, parse_blame};
use time::OffsetDateTime;

fn now() -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp(1_784_760_815).unwrap()
}

const PORCELAIN: &str = "\
c51dd88c 1 1 2
author J.S.Júnior
author-mail <j.s.junior@live.com>
author-time 1784760815
author-tz -0300
summary feat: bootstrap
filename Cargo.toml
\t[package]
c51dd88c 2 2
author J.S.Júnior
author-time 1784760815
filename Cargo.toml
\tname = \"betterreview\"
a1b2c3d4 7 3
author Outra Pessoa
author-time 1781168815
filename Cargo.toml
\tedition = \"2024\"
";

#[test]
fn every_line_of_the_file_gets_its_author_and_age() {
    let blame = parse_blame(PORCELAIN, now());

    assert_eq!(blame.len(), 3);
    assert_eq!(
        blame.get(&1),
        Some(&BlameLine {
            author: "J.S.Júnior".into(),
            age: "today".into()
        })
    );
    assert_eq!(
        blame.get(&3).map(|line| line.author.as_str()),
        Some("Outra Pessoa"),
        "the final line number is what the gutter is keyed by, not the original one"
    );
}

#[test]
fn the_age_is_relative_because_a_date_says_nothing_at_a_glance() {
    let blame = parse_blame(PORCELAIN, now());

    assert_eq!(
        blame.get(&3).map(|line| line.age.as_str()),
        Some("1mo"),
        "forty-one days reads better as a month than as five weeks"
    );
}

#[test]
fn an_uncommitted_line_is_named_rather_than_blamed() {
    let porcelain = "\
0000000000000000000000000000000000000000 1 1 1
author Not Committed Yet
author-time 1784760815
filename Cargo.toml
\tworking copy
";

    let blame = parse_blame(porcelain, now());

    assert_eq!(
        blame.get(&1).map(|line| line.age.as_str()),
        Some("uncommitted")
    );
}
