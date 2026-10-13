//! Tests against a real X4 install (`X4_GAME_DIR`, default: the dev machine's
//! Steam path) and the extracted samples in `research/extracted/`. Skipped when
//! those are missing.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use x4sd_core::catalog::Catalog;
use x4sd_core::game::find_profile_dir;
use x4sd_core::geometry::{attach, fit, Transform};
use x4sd_core::import::write_import_file;
use x4sd_core::plan::{parse_plans, Plan};
use x4sd_core::validate::{validate, Issue};

const DEFAULT_GAME_DIR: &str = r"F:\SteamLibrary\steamapps\common\X4 Foundations";

fn catalog() -> Option<&'static Catalog> {
    static CATALOG: OnceLock<Option<Catalog>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let game = std::env::var("X4_GAME_DIR").unwrap_or_else(|_| DEFAULT_GAME_DIR.to_owned());
            if !Path::new(&game).join("01.cat").exists() {
                eprintln!("skipped: no X4 install at {game}");
                return None;
            }
            let start = Instant::now();
            let c = Catalog::load(Path::new(&game), find_profile_dir().as_deref()).unwrap();
            eprintln!("catalog loaded in {:?}, {} enabled extensions", start.elapsed(), c.extensions().len());
            Some(c)
        })
        .as_ref()
}

fn sample(rel: &str) -> Option<Vec<Plan>> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    let xml = std::fs::read_to_string(&path).ok().or_else(|| {
        eprintln!("skipped: {rel} not extracted");
        None
    })?;
    Some(parse_plans(&xml).unwrap())
}

#[test]
fn resolves_base_dlc_and_mod_modules() {
    let Some(c) = catalog() else { return };
    let base = c.module("struct_arg_base_02_macro").unwrap();
    assert_eq!(base.class, "connectionmodule");
    assert_eq!(base.source, None);
    assert_eq!(base.snap("connectionsnap001").unwrap().transform.position, [0.0, 0.0, 400.0]);
    assert_eq!(base.snap("ConnectionSnap002").unwrap().transform.position, [0.0, 0.0, -400.0]);

    let dlc = c.module("hab_ter_l_01_macro").unwrap();
    assert_eq!(dlc.source.as_deref(), Some("ego_dlc_terran"));

    match c.module("recyclemodule_macro") {
        Ok(m) => assert_eq!(m.source.as_deref(), Some("RecycleShipsAndStations")),
        Err(e) => eprintln!("mod module not resolved (mod disabled or absent?): {e}"),
    }
}

#[test]
fn lists_station_modules_with_names() {
    let Some(c) = catalog() else { return };
    let start = Instant::now();
    let modules = c.station_modules().unwrap();
    eprintln!("{} station modules listed in {:?}", modules.len(), start.elapsed());
    let find = |m: &str| modules.iter().find(|x| x.macro_name == m).unwrap_or_else(|| panic!("{m} not listed"));
    assert_eq!(find("struct_arg_base_02_macro").name, "Argon Base Connection Structure 02");
    assert_eq!(find("hab_ter_l_01_macro").name, "Terran L Living Quarters");
    assert_eq!(find("prod_gen_energycells_macro").name, "Energy Cell Production");
    let unresolved: Vec<_> = modules.iter().filter(|m| m.name.contains('{')).map(|m| (&m.macro_name, &m.name)).collect();
    eprintln!("names with unresolved references: {unresolved:?}");
    assert!(modules.iter().all(|m| !m.name.is_empty()));
}

/// Player plans: the only real problem is the deliberately moved casino of test C.
#[test]
fn validates_player_plans() {
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml")) else { return };
    let mut found = Vec::new();
    for plan in &plans {
        for issue in validate(plan, c) {
            eprintln!("[{}] {issue}", plan.name);
            found.push((plan.name.clone(), issue));
        }
    }
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(matches!(&found[0], (name, Issue::SnapGap { index: 32, metres }) if name == "X4SD test C" && (*metres - 1000.0).abs() < 0.01));
}

/// Library plans: of the 31 off-link entries in hand-made faction plans, the 17
/// that are metres off (2.4 m to 400 m); 14 more are 1-3.5 cm off, within tolerance.
#[test]
fn validates_library_plans() {
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/base/libraries/constructionplans.xml")) else { return };
    let issues: Vec<Issue> = plans.iter().flat_map(|p| validate(p, c)).collect();
    let gaps: Vec<f64> = issues.iter().filter_map(|i| match i { Issue::SnapGap { metres, .. } => Some(*metres), _ => None }).collect();
    let other: Vec<_> = issues.iter().filter(|i| !matches!(i, Issue::SnapGap { .. })).collect();
    eprintln!("library: {} gaps, other {other:?}", gaps.len());
    assert_eq!(gaps.len(), 17);
    assert!(gaps.iter().all(|m| *m > 1.0), "{gaps:?}");
    assert!(other.is_empty(), "{other:?}");
}

/// `attach` with each entry's own twist puts every linked module back where the game stored it.
#[test]
fn attach_reproduces_game_placements() {
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml")) else { return };
    let mut checked = 0;
    for plan in plans.iter().filter(|p| !p.name.starts_with("X4SD")) {
        for e in &plan.entries {
            let (Some(conn), Some(pred)) = (&e.connection, &e.predecessor) else { continue };
            let p = plan.entries.iter().find(|x| x.index == pred.index).unwrap();
            let (Ok(cm), Ok(pm)) = (c.module(&e.macro_name), c.module(&p.macro_name)) else { continue };
            let (ps, cs) = (&pm.snap(&pred.connection).unwrap().transform, &cm.snap(conn).unwrap().transform);
            let (pt, stored) = (Transform::from_offset(p.offset.as_ref()), Transform::from_offset(e.offset.as_ref()));
            let placed = attach(&pt, ps, cs, fit(&pt, ps, &stored, cs).twist);
            let d = (0..3).map(|i| (placed.position[i] - stored.position[i]).powi(2)).sum::<f64>().sqrt();
            assert!(d < 0.02, "[{}] entry {}: {d} m", plan.name, e.index);
            let f = fit(&pt, ps, &placed, cs);
            assert!(f.gap < 1e-6 && f.misalignment < 1e-6);
            checked += 1;
        }
    }
    eprintln!("attach reproduced {checked} placements");
    assert!(checked > 500); // 534 linked entries in the 5 real player plans

}

#[test]
fn computes_patches_and_writes_import_file() {
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/TerranScrapyard.xml")) else { return };
    let plan = &plans[0];
    assert_eq!(c.required_patches(plan), plan.patches);

    let dir = std::env::temp_dir().join(format!("x4sd-test-{}", std::process::id()));
    let path = write_import_file(&dir, plan, false).unwrap();
    assert!(path.ends_with("constructionplan/TerranScrapyard.xml") || path.ends_with(r"constructionplan\TerranScrapyard.xml"));
    assert!(write_import_file(&dir, plan, false).is_err(), "must not overwrite by default");
    let reread = parse_plans(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(&reread[0], plan);
    std::fs::remove_dir_all(dir).unwrap();
}
