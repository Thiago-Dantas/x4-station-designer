//! Round-trip tests: read a plan file, write it back, compare.

use std::path::{Path, PathBuf};
use x4sd_core::plan::{parse_plans, write_plans, Orientation};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn fixture_round_trips_byte_for_byte() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/fixture.xml");
    let xml = std::fs::read_to_string(path).unwrap();
    let plans = parse_plans(&xml).unwrap();

    assert_eq!(plans.len(), 2);
    assert_eq!(plans[0].name, r#"Fixture & "friends""#);
    assert_eq!(plans[0].entries[1].offset.as_ref().unwrap().orientation,
               Orientation::Euler { yaw: 90.0, pitch: 0.0, roll: -90.00001 });
    assert_eq!(plans[1].description, None);
    assert_eq!(write_plans(&plans), xml);
}

/// Every game file we have: values must survive exactly. Files the game wrote
/// itself (LF line endings) must also match line for line, except for
/// rotation/quaternion lines, whose number text may differ (same f32 value).
#[test]
fn game_files_round_trip() {
    let candidates = [
        "research/extracted/profile-samples/constructionplans-20261013-1501.xml",
        "research/extracted/profile-samples/TerranScrapyard.xml",
        "research/extracted/base/libraries/constructionplans.xml",
    ];
    let mut checked = 0;
    for rel in candidates {
        let path = repo_root().join(rel);
        let Ok(xml) = std::fs::read_to_string(&path) else {
            eprintln!("skipped (not extracted): {rel}");
            continue;
        };
        let plans = parse_plans(&xml).unwrap_or_else(|e| panic!("{rel}: {e}"));
        let written = write_plans(&plans);
        let reread = parse_plans(&written).unwrap();
        for (p, q) in plans.iter().zip(&reread) {
            for (e, f) in p.entries.iter().zip(&q.entries) {
                assert_eq!(e, f, "{rel}: values changed in plan {} entry {}", p.id, e.index);
            }
        }
        assert_eq!(reread, plans, "{rel}: values changed");

        if !xml.contains("\r\n") {
            let (a, b): (Vec<_>, Vec<_>) = (xml.lines().collect(), written.lines().collect());
            assert_eq!(a.len(), b.len(), "{rel}: line count differs");
            let differing: Vec<_> = a.iter().zip(&b).filter(|(x, y)| x != y).collect();
            for (x, y) in &differing {
                assert!(x.contains("<rotation ") || x.contains("<quaternion "),
                        "{rel}: unexpected difference\n  game: {x}\n  ours: {y}");
            }
            eprintln!("{rel}: {} plans, {} lines, {} rotation/quaternion lines formatted differently",
                      plans.len(), a.len(), differing.len());
        } else {
            eprintln!("{rel}: {} plans, values identical (CRLF file, not compared line by line)", plans.len());
        }
        checked += 1;
    }
    eprintln!("checked {checked} of {} game files", candidates.len());
}
