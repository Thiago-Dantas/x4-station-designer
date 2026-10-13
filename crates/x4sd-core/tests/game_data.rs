//! Tests against a real X4 install (`X4_GAME_DIR`, default: the dev machine's
//! Steam path) and the extracted samples in `research/extracted/`. Skipped when
//! those are missing.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use x4sd_core::catalog::Catalog;
use x4sd_core::design::Design;
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

    assert_eq!(base.shape.len(), 1, "{:?}", base.shape);
    assert_eq!(base.shape[0].half_extents[2], 400.0, "the beam's main part spans its two snaps");

    let dlc = c.module("hab_ter_l_01_macro").unwrap();
    assert_eq!(dlc.source.as_deref(), Some("ego_dlc_terran"));
    let pier = c.module("pier_ter_harbor_02_macro").unwrap();
    assert!(pier.shape.iter().all(|b| b.half_extents[0] < 3000.0), "docking volume must be skipped: {:?}", pier.shape);

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

    // Every listed module resolves to a component with snaps and a shape to draw.
    let mut no_snaps = Vec::new();
    let mut no_shape = Vec::new();
    for m in &modules {
        let module = c.module(&m.macro_name).unwrap_or_else(|e| panic!("{}: {e}", m.macro_name));
        if module.snaps.is_empty() {
            no_snaps.push(m.macro_name.clone());
        }
        if module.shape.is_empty() {
            no_shape.push(m.macro_name.clone());
        }
    }
    eprintln!("without snaps: {no_snaps:?}\nwithout shape: {no_shape:?}");
    assert!(no_shape.is_empty());
}

/// Real models: every station module decodes to a mesh whose bounds fit its part boxes.
#[test]
fn builds_module_models() {
    let Some(c) = catalog() else { return };
    let start = Instant::now();
    let (mut total_vertices, mut total_triangles, mut empty) = (0, 0, Vec::new());
    for m in c.station_modules().unwrap() {
        let model = c.module_model(&m.macro_name, 1, 4).unwrap_or_else(|e| panic!("{}: {e}", m.macro_name));
        if model.indices.is_empty() {
            empty.push(m.macro_name.clone());
            continue;
        }
        assert_eq!(model.positions.len(), model.normals.len());
        assert!(model.indices.iter().all(|&i| (i as usize) < model.positions.len()));
        total_vertices += model.positions.len();
        total_triangles += model.indices.len() / 3;
    }
    eprintln!("models built in {:?}: {total_vertices} vertices, {total_triangles} triangles (lod 1, all details); empty: {empty:?}", start.elapsed());
    // Assembled from child landmark pieces; has no geometry of its own (keeps its box).
    assert_eq!(empty, ["landmarks_par_storage_01_macro"]);
    for (lod, details) in [(2, 2), (3, 0)] {
        let triangles: usize = c.station_modules().unwrap().iter().map(|m| c.module_model(&m.macro_name, lod, details).unwrap().indices.len() / 3).sum();
        eprintln!("lod {lod}, details <= {details}: {triangles} triangles in all modules");
    }

    // Textures: every material used by the beam has a diffuse map that loads.
    let beam = c.module_model("struct_arg_base_02_macro", 1, 4).unwrap();
    assert!(beam.uvs.iter().any(|uv| uv[0] != 0.0 || uv[1] != 0.0), "the beam has texture coordinates");
    let mut formats = std::collections::BTreeMap::new();
    for (material, _, _) in &beam.groups {
        let maps = c.material(material).unwrap_or_else(|| panic!("material {material} not in the library"));
        let texture = c.texture(maps.diffuse.as_deref().unwrap(), 1024).unwrap();
        let (w, h, _) = &texture.mips[0];
        assert!(*w <= 1024 && *h <= 1024, "{material}: {w}x{h}");
        formats.insert(material.clone(), format!("{:?} {w}x{h}, {} mips", texture.format, texture.mips.len()));
    }
    eprintln!("beam textures: {formats:?}");

    // Newer `p2` materials come from material_library_1.xml, e.g. on the scrap recycler.
    let recycler = c.module_model("prod_gen_scrap_recycler_macro", 2, 2).unwrap();
    let unknown: Vec<_> = recycler.groups.iter().filter(|g| c.material(&g.0).is_none()).map(|g| g.0.clone()).collect();
    assert!(unknown.is_empty(), "materials not in any library: {unknown:?}");

    // The Argon beam: its main part spans exactly the box from the component XML.
    let beam = c.module_model("struct_arg_base_02_macro", 1, 0).unwrap();
    let max_z = beam.positions.iter().map(|p| p[2]).fold(f32::MIN, f32::max);
    assert!((max_z - 400.0).abs() < 0.5, "{max_z}");
}

/// A station built with the editing operations from real modules passes validation.
#[test]
fn designed_station_validates() {
    let Some(c) = catalog() else { return };
    let mut d = Design::new("x4sd built");
    let hub = d.add_root(c, "struct_ter_cross_01_macro", Transform::IDENTITY).unwrap();
    let arms: Vec<String> = d.free_snaps(c, hub).unwrap().into_iter().map(|s| s.name).collect();
    assert!(arms.len() >= 4, "{arms:?}");
    let beam = c.module("struct_ter_base_02_macro").unwrap();
    for (i, arm) in arms.iter().enumerate() {
        d.attach(c, "struct_ter_base_02_macro", &beam.snaps[0].name, hub, arm, 90.0 * i as f64).unwrap();
    }
    let hab_snap = c.module("hab_ter_l_01_macro").unwrap().snaps[0].name.clone();
    let end = d.free_snaps(c, 2).unwrap()[0].name.clone();
    d.attach(c, "hab_ter_l_01_macro", &hab_snap, 2, &end, 0.0).unwrap();

    assert_eq!(validate(d.plan(), c), Vec::<Issue>::new());
    assert_eq!(d.plan().patches.iter().map(|p| p.extension.as_str()).collect::<Vec<_>>(), ["ego_dlc_terran"]);
    let xml = x4sd_core::plan::write_plans(std::slice::from_ref(d.plan()));
    assert_eq!(validate(&parse_plans(&xml).unwrap()[0], c), Vec::<Issue>::new(), "still valid after f32/mm rounding");
}

/// Build costs: module recipes from wares.xml (+ DLC patches), method fallback, equipment.
#[test]
fn build_costs() {
    let Some(c) = catalog() else { return };
    let start = Instant::now();
    let w = c.wares().unwrap();
    eprintln!("wares loaded in {:?}; methods: {:?}", start.elapsed(), w.methods().iter().map(|m| format!("{} ({})", m.id, m.name)).collect::<Vec<_>>());
    let amounts = |cost: &x4sd_core::wares::Cost| cost.lines.iter().map(|l| (l.ware.clone(), l.amount)).collect::<std::collections::BTreeMap<_, _>>();

    let cells = w.cost([("prod_gen_energycells_macro", 1.0)], "default");
    assert_eq!(amounts(&cells), [("claytronics".into(), 260.0), ("energycells".into(), 520.0), ("hullparts".into(), 951.0)].into());
    assert!(cells.value > 0.0);
    // Terran modules: their default recipe uses Terran wares; an unknown method falls back to default.
    let hab = w.cost([("hab_ter_l_01_macro", 2.0)], "terran");
    assert_eq!(amounts(&hab), [("computronicsubstrate".into(), 160.0), ("energycells".into(), 1186.0), ("siliconcarbide".into(), 330.0)].into());
    assert!(w.methods().iter().any(|m| m.id == "terran") && !w.methods().iter().any(|m| m.id == "xenon"), "xenon is NPC-only");

    // Whole plans, equipment included; every module of the player's plans has a recipe.
    let Some(plans) = sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml") else { return };
    for plan in plans.iter().filter(|p| !p.name.starts_with("X4SD")) {
        let cost = w.plan_cost(plan, None, "default");
        eprintln!("[{}] {:.0} Cr, {} resources, unknown {:?}", plan.name, cost.value, cost.lines.len(), cost.unknown);
        assert!(cost.value > 0.0);
        assert!(cost.unknown.iter().all(|m| m == "recyclemodule_macro"), "{:?}", cost.unknown);
    }
    let defence = plans.iter().find(|p| p.name == "defense station").unwrap();
    let armed = defence.entries.iter().find(|e| !e.upgrades.is_empty()).expect("a module with turrets");
    let module_only = w.cost([(armed.macro_name.as_str(), 1.0)], "default").value;
    let with_equipment = w.plan_cost(defence, Some(armed.index), "default").value;
    eprintln!("#{} {}: {module_only:.0} Cr alone, {with_equipment:.0} Cr with its {} turret/shield groups", armed.index, armed.macro_name, armed.upgrades.len());
    assert!(with_equipment > module_only, "turrets and shields add to the module's cost");
}

/// Station analysis: production rates, workforce bonus and upkeep, sunlight, storage.
#[test]
fn station_analysis() {
    use x4sd_core::economy::{analyze, Analysis};
    let Some(c) = catalog() else { return };
    let net = |a: &Analysis, ware: &str| a.flows.iter().find(|f| f.ware == ware).map_or(0.0, |f| f.net());
    let close = |a: f64, b: f64| (a - b).abs() < 1e-6;

    // Energy cells: 175 per 60 s, unstaffed.
    let mut d = Design::new("economy");
    d.add_root(c, "prod_gen_energycells_macro", Transform::IDENTITY).unwrap();
    let a = analyze(c, d.plan(), 1.0).unwrap();
    assert!(close(net(&a, "energycells"), 10_500.0), "{:?}", a.flows);
    assert_eq!((a.workforce_needed, a.workforce_fill), (90, 0.0));

    // An argon L habitat staffs it (+43%) and its 1000 workers eat food rations and medical supplies.
    let far = Transform { position: [3000.0, 0.0, 0.0], ..Transform::IDENTITY };
    d.add_root(c, "hab_arg_l_01_macro", far).unwrap();
    let a = analyze(c, d.plan(), 1.0).unwrap();
    assert_eq!((a.housing.clone(), a.workforce_fill), (vec![("argon".to_owned(), 1000)], 1.0));
    assert!(close(net(&a, "energycells"), 15_015.0), "{:?}", a.flows);
    assert!(close(net(&a, "foodrations"), -2250.0) && close(net(&a, "medicalsupplies"), -1350.0), "{:?}", a.flows);
    // Solar output follows the sunlight.
    assert!(close(net(&analyze(c, d.plan(), 0.5).unwrap(), "energycells"), 7507.5));
    // Container storage: none yet, but energy cells, food and medicine need it.
    let container = a.storage.iter().find(|s| s.transport == "container").expect("container flow");
    assert_eq!(container.capacity, 0.0);
    assert!(container.volume_per_hour > 15_015.0);

    // The player's plans: every module is known; print a summary.
    let Some(plans) = sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml") else { return };
    for plan in plans.iter().filter(|p| !p.name.starts_with("X4SD")) {
        let a = analyze(c, plan, 1.0).unwrap();
        let top: Vec<String> = a.flows.iter().take(3).map(|f| format!("{} {:+.0}", f.ware, f.net())).collect();
        let short: Vec<String> = a.flows.iter().rev().take(3).filter(|f| f.net() < 0.0).map(|f| format!("{} {:.0}", f.ware, f.net())).collect();
        eprintln!(
            "[{}] workforce {}/{} housed {:?}; top {top:?}; short {short:?}; storage {:?}; docks {} piers {}; unknown {:?}",
            plan.name,
            a.housing.iter().map(|h| h.1).sum::<u32>(),
            a.workforce_needed,
            a.housing,
            a.storage.iter().map(|s| format!("{} {:.0}/{:.0}", s.transport, s.capacity, s.volume_per_hour)).collect::<Vec<_>>(),
            a.dock_areas,
            a.piers,
            a.unknown
        );
        assert!(a.unknown.iter().all(|m| m == "recyclemodule_macro"), "{:?}", a.unknown);
    }
}

/// Plots: the required plot holds the station; a smaller plot flags the modules outside.
#[test]
fn plots() {
    use x4sd_core::plot::{required_plot, Plot};
    use x4sd_core::validate::validate_plot;
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml")) else { return };
    for plan in plans.iter().filter(|p| !p.name.starts_with("X4SD")) {
        let required = required_plot(plan, c).unwrap();
        eprintln!("[{}] needs {required}", plan.name);
        assert!(required.is_allowed(), "the game built these, so they fit a plot");
        assert!(validate_plot(plan, c, &required).is_empty());
    }
    let mega = plans.iter().find(|p| p.name == "megashipyard terran").unwrap();
    let issues = validate_plot(mega, c, &Plot::around(1));
    assert!(matches!(issues[0], Issue::PlotTooSmall { .. }), "{issues:?}");
    assert!(issues.iter().any(|i| matches!(i, Issue::OutsidePlot { .. })));

    // A design grows its plot only when asked.
    let mut d = Design::new("plot test");
    assert_eq!(d.plot(), Some(Plot::around(1)));
    let hub = d.add_root(c, "struct_ter_cross_01_macro", Transform::IDENTITY).unwrap();
    let beam = c.module("struct_ter_base_02_macro").unwrap();
    let mut end = (hub, d.free_snaps(c, hub).unwrap()[0].name.clone());
    for _ in 0..4 {
        let next = d.attach(c, "struct_ter_base_02_macro", &beam.snaps[0].name, end.0, &end.1, 0.0).unwrap();
        end = (next, d.free_snaps(c, next).unwrap()[0].name.clone());
    }
    let plot = d.plot().unwrap();
    assert!(!validate_plot(d.plan(), c, &plot).is_empty(), "a 3 km arm does not fit 2x2x2");
    let grown = d.fit_plot(c, true).unwrap();
    assert!(grown.contains(&plot) && validate_plot(d.plan(), c, &grown).is_empty(), "{grown}");
    eprintln!("grown to {grown}");
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
