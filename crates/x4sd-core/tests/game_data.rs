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
        Ok(m) => {
            assert_eq!(m.source.as_deref(), Some("RecycleShipsAndStations"));
            // Its component borrows another's meshes (`<source geometry>`); its name is in the mod's texts.
            let model = c.module_model("recyclemodule_macro", 1, 4).unwrap();
            let textured = model.groups.iter().filter(|g| c.material(&g.0).and_then(|m| m.diffuse).is_some()).count();
            let name = c.display_name("recyclemodule_macro");
            eprintln!("mod module: {} triangles, {textured} of {} material groups textured, name {name:?}", model.indices.len() / 3, model.groups.len());
            assert!(model.indices.len() > 1000 && textured > 0 && name.is_some());
        }
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

/// Production planner: a target's whole chain, housed and stored; race, buy list, existing stations.
#[test]
fn production_planner() {
    use x4sd_core::planner::{plan_production, producible_wares, Request, Role};
    let Some(c) = catalog() else { return };
    let start = Instant::now();
    let producible: Vec<String> = producible_wares(c).unwrap().into_iter().map(|w| w.0).collect();
    eprintln!("{} producible wares in {:?}", producible.len(), start.elapsed());
    assert!(producible.contains(&"hullparts".to_owned()) && !producible.contains(&"ore".to_owned()), "ore is mined");
    let req = |targets: &[(&str, u32)], race: Option<&str>, buy: &[&str]| Request {
        targets: targets.iter().map(|(w, n)| (w.to_string(), *n)).collect(),
        race: race.map(str::to_owned),
        workforce: true,
        storage_hours: 6.0,
        buy: buy.iter().map(|w| w.to_string()).collect(),
        sunlight: 1.0,
        method: "default".to_owned(),
    };
    let empty: Vec<(String, u32)> = Vec::new();
    let count = |p: &x4sd_core::planner::ProductionPlan, m: &str| p.modules.iter().find(|x| x.macro_name == m).map_or(0, |x| x.count);

    let start = Instant::now();
    let p = plan_production(c, &empty, &req(&[("hullparts", 2)], None, &[])).unwrap();
    eprintln!("hull parts x2 in {:?}:", start.elapsed());
    for m in &p.modules {
        eprintln!("  {:>3} x {} ({}) {}", m.count, m.name, m.macro_name, m.role.as_str());
    }
    let short: Vec<String> = p.analysis.flows.iter().filter(|f| f.net() < -1e-6).map(|f| format!("{} {:.0}", f.ware, f.net())).collect();
    eprintln!("  to supply: {short:?}; cost {:.0} Cr", p.cost.value);
    assert_eq!(count(&p, "prod_gen_hullparts_macro"), 2);
    // Self-sufficient: only raw materials (mined, never made) are left short.
    assert!(p.analysis.self_sufficient(), "{short:?}");
    for f in p.analysis.flows.iter().filter(|f| f.net() < -1e-6) {
        assert!(f.raw && !producible.contains(&f.ware), "{} is still short but could be made", f.ware);
    }
    let housed: u32 = p.analysis.housing.iter().map(|h| h.1).sum();
    assert!(housed >= p.analysis.workforce_needed && p.analysis.workforce_fill == 1.0);
    for s in p.analysis.storage.iter().filter(|s| s.volume_per_hour > 0.0) {
        assert!(s.capacity >= 6.0 * s.volume_per_hour - 1e-6, "{} storage", s.transport);
    }
    assert!(p.modules.iter().any(|m| m.role == Role::Dock) && p.cost.value > 0.0);

    // Buying energy cells: no solar modules. Teladi: Teladi hull parts (teladianium).
    let bought = plan_production(c, &empty, &req(&[("hullparts", 2)], None, &["energycells"])).unwrap();
    assert_eq!(count(&bought, "prod_gen_energycells_macro"), 0);
    assert!(bought.analysis.flows.iter().any(|f| f.ware == "energycells" && f.net() < 0.0 && !f.raw));
    assert!(!bought.analysis.self_sufficient(), "energy cells are bought");
    let teladi = plan_production(c, &empty, &req(&[("hullparts", 1)], Some("teladi"), &[])).unwrap();
    assert_eq!(count(&teladi, "prod_tel_hullparts_macro"), 1);
    assert!(teladi.modules.iter().any(|m| m.macro_name == "prod_tel_teladianium_macro"));
    assert!(teladi.modules.iter().filter(|m| m.role == Role::Habitat).all(|m| m.macro_name.starts_with("hab_tel_")));

    // An existing station with no targets: its shortfalls get producers.
    let Some(plans) = sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml") else { return };
    let yard = plans.iter().find(|p| p.name == "TerranScrapyard").unwrap();
    let fix = plan_production(c, &x4sd_core::planner::module_counts(yard), &req(&[], Some("terran"), &[])).unwrap();
    eprintln!("TerranScrapyard fix: {:?}", fix.modules.iter().map(|m| format!("{} x{}", m.macro_name, m.count)).collect::<Vec<_>>());
    assert!(count(&fix, "prod_ter_energycells_macro") > 0);
    // Scrap metal is raw (salvaged, brought in): the fixed scrapyard is self-sufficient.
    assert!(fix.analysis.flows.iter().any(|f| f.ware == "scrapmetal" && f.raw && f.net() < 0.0));
    assert!(fix.analysis.self_sufficient(), "{:?}", fix.analysis.flows.iter().filter(|f| f.net() < 0.0).map(|f| &f.ware).collect::<Vec<_>>());
}

/// Places `modules` with `auto_place` and checks the result: no overlaps involving
/// the new modules, every new module linked to its snap, inside a plot the game
/// allows (or the design's plot), and one undo step. Prints a summary.
fn place_and_check(c: &Catalog, d: &mut Design, modules: &[(String, u32)], options: &x4sd_core::layout::Options) -> x4sd_core::layout::Placement {
    use x4sd_core::layout::{auto_place, overlaps};
    use x4sd_core::plot::required_plot;
    use x4sd_core::validate::validate_plot;
    let before = d.plan().clone();
    let start = Instant::now();
    let r = auto_place(c, d, modules, options).unwrap();
    let elapsed = start.elapsed();
    let mut connectors: std::collections::BTreeMap<&str, usize> = Default::default();
    for (_, m) in &r.connectors {
        *connectors.entry(m.as_str()).or_default() += 1;
    }
    let w = c.wares().unwrap();
    let total_cost = w.plan_cost(d.plan(), None, "default").value;
    let connector_cost = w.cost(r.connectors.iter().map(|(_, m)| (m.as_str(), 1.0)), "default").value;
    let plot = required_plot(d.plan(), c).unwrap();
    eprintln!(
        "[{}] placed {} in {elapsed:?} with connectors {connectors:?} ({:.1}% of the {:.1} M Cr station); docks moved {}, blocked {}; unplaced {:?}; plot {plot}",
        d.plan().name,
        r.placed.len(),
        100.0 * connector_cost / total_cost,
        total_cost / 1e6,
        r.docks_moved,
        r.docks_blocked,
        r.unplaced
    );
    // Every module that was there stays where it was, except docks and piers (lifted
    // off and placed again, so entries may be renumbered); nothing is lost.
    let dock = |m: &str| ["pier", "dockarea"].contains(&c.module(m).unwrap().class.as_str());
    let same = |a: &x4sd_core::plan::Entry, b: &x4sd_core::plan::Entry| a.macro_name == b.macro_name && a.offset == b.offset;
    for b in before.entries.iter().filter(|b| !dock(&b.macro_name)) {
        assert!(d.plan().entries.iter().any(|e| same(e, b)), "#{} {} moved", b.index, b.macro_name);
    }
    assert_eq!(d.plan().entries.len(), before.entries.len() + r.placed.len() + r.connectors.len());
    let new: Vec<u32> = d.plan().entries.iter().filter(|e| !before.entries.iter().any(|b| same(e, b))).map(|e| e.index).collect();
    let found: Vec<_> = overlaps(d.plan(), c).into_iter().filter(|o| new.contains(&o.0) || new.contains(&o.1)).collect();
    assert!(found.is_empty(), "{found:?}");
    let issues: Vec<Issue> = validate(d.plan(), c).into_iter().filter(|i| i.index().is_none_or(|i| new.contains(&i))).collect();
    assert_eq!(issues, Vec::<Issue>::new());
    match (options.grow_plot, d.plot()) {
        (false, Some(p)) => assert!(validate_plot(d.plan(), c, &p).is_empty(), "inside {p}"),
        _ => assert!(plot.is_allowed(), "{plot}"),
    }
    assert!(d.undo());
    assert_eq!(d.plan(), &before, "one undo step");
    assert!(d.redo());
    r
}

/// The last two entries are a dock area and a pier, and the line from the station
/// centre out through each, for 3 km, passes through no other module.
fn assert_docks_outside(c: &Catalog, d: &Design) {
    use x4sd_core::layout::Body;
    let class = |e: &x4sd_core::plan::Entry| c.module(&e.macro_name).unwrap().class.clone();
    assert!(d.plan().entries.iter().rev().take(2).all(|e| ["pier", "dockarea"].contains(&class(e).as_str())));
    let bodies: Vec<(u32, Body)> = d
        .plan()
        .entries
        .iter()
        .map(|e| (e.index, Body::new(&c.collision_boxes(&e.macro_name).unwrap(), &c.module(&e.macro_name).unwrap().snaps, &d.world(e.index).unwrap())))
        .collect();
    let parts: Vec<_> = d.plan().entries.iter().filter_map(|e| x4sd_core::plot::entry_bounds(d.plan(), c, e.index)).collect();
    let (lo, hi) = parts.iter().fold(parts[0], |(l, h), (a, b)| (std::array::from_fn(|i| l[i].min(a[i])), std::array::from_fn(|i| h[i].max(b[i]))));
    let station: [f64; 3] = std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0);
    for (index, body) in bodies.iter().rev().take(2) {
        let (l, h) = body.bounds();
        let mid: [f64; 3] = std::array::from_fn(|i| (l[i] + h[i]) / 2.0);
        let out: [f64; 3] = std::array::from_fn(|i| mid[i] - station[i]);
        let len = out.iter().map(|v| v * v).sum::<f64>().sqrt();
        // Samples every 50 m along the line out to 3 km past the dock's centre.
        let blocked: Vec<u32> = bodies
            .iter()
            .filter(|(i, b)| i != index && (1..=60).any(|k| b.contains(std::array::from_fn(|j| mid[j] + out[j] / len * k as f64 * 50.0))))
            .map(|(i, _)| *i)
            .collect();
        eprintln!("dock #{index}: {len:.0} m from the station centre, in the way: {blocked:?}");
        assert!(blocked.is_empty());
    }
}

/// Auto-placement: "a staffed station with 100 solar panels" planned and placed
/// without overlaps inside a plot; a Terran station; a small plot; an existing station.
#[test]
fn auto_place_planned_stations() {
    use x4sd_core::layout::Options;
    use x4sd_core::planner::{plan_production, Request};
    let Some(c) = catalog() else { return };
    let req = |targets: &[(&str, u32)], race: Option<&str>| Request {
        targets: targets.iter().map(|(w, n)| (w.to_string(), *n)).collect(),
        race: race.map(str::to_owned),
        workforce: true,
        storage_hours: 6.0,
        buy: Vec::new(),
        sunlight: 1.0,
        method: "default".to_owned(),
    };
    let planned = |d: &Design, r: &Request| -> Vec<(String, u32)> {
        plan_production(c, &x4sd_core::planner::module_counts(d.plan()), r).unwrap().modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect()
    };
    let grow = Options { grow_plot: true, race: None };

    let mut d = Design::new("100 solar");
    let modules = planned(&d, &req(&[("energycells", 100)], None));
    let r = place_and_check(c, &mut d, &modules, &grow);
    assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
    assert_eq!(r.placed.len() as u32, modules.iter().map(|m| m.1).sum::<u32>());
    // Docks are placed last, so they come last in the build order, with nothing in
    // their way out from the station centre.
    assert_docks_outside(c, &d);
    // A second run (10 more solar modules) lifts the docks off and places them
    // outside the grown station again.
    let docks = |d: &Design| d.plan().entries.iter().filter(|e| ["pier", "dockarea"].contains(&c.module(&e.macro_name).unwrap().class.as_str())).count();
    let before = docks(&d);
    let more = place_and_check(c, &mut d, &[("prod_gen_energycells_macro".to_owned(), 10)], &grow);
    eprintln!("second run: {} placed, {} docks moved, {} without a clear corridor", more.placed.len(), more.docks_moved, more.docks_blocked);
    assert_eq!((more.placed.len(), more.docks_moved, more.docks_blocked, docks(&d)), (10, 2, 0, before));
    assert_docks_outside(c, &d);
    let path = std::env::temp_dir().join("x4sd-auto-place.xml");
    std::fs::write(&path, x4sd_core::plan::write_plans(std::slice::from_ref(d.plan()))).unwrap();
    eprintln!("saved to {}", path.display());

    // Terran modules get Terran connection structures.
    let mut d = Design::new("terran hull parts");
    let modules = planned(&d, &req(&[("hullparts", 2)], Some("terran")));
    let r = place_and_check(c, &mut d, &modules, &grow);
    assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
    assert!(r.connectors.iter().all(|(_, m)| m.starts_with("struct_ter_")), "{:?}", r.connectors);

    // Without growing, a 2x2x2 km plot holds only some of the solar station.
    let mut d = Design::new("small plot");
    let modules = planned(&d, &req(&[("energycells", 100)], None));
    let r = place_and_check(c, &mut d, &modules, &Options { grow_plot: false, race: None });
    assert!(!r.placed.is_empty() && !r.unplaced.is_empty());

    // An existing station: the planner's fix for the scrapyard, placed onto it.
    let Some(plans) = sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml") else { return };
    let mut d = Design::from_plan(plans.iter().find(|p| p.name == "TerranScrapyard").unwrap().clone());
    let modules = planned(&d, &req(&[], Some("terran")));
    let r = place_and_check(c, &mut d, &modules, &grow);
    eprintln!("scrapyard fix: {} modules to place", modules.iter().map(|m| m.1).sum::<u32>());
    assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
}

/// Equipment: modules' turret and shield groups, the buildable turrets and shields,
/// and the fit rule (equipment tags within the slot tags) against every equipped
/// group in the player's and the library plans.
#[test]
fn equipment_groups_and_fit() {
    use std::collections::BTreeMap;
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml")) else { return };
    let start = Instant::now();
    let equipment = c.equipment().unwrap();
    let mut kinds: BTreeMap<(String, String), usize> = BTreeMap::new();
    for e in equipment {
        *kinds.entry((e.kind.clone(), e.size().unwrap_or("?").to_owned())).or_default() += 1;
    }
    eprintln!("{} buildable turrets and shields in {:?}: {kinds:?}", equipment.len(), start.elapsed());
    assert!(equipment.iter().any(|e| e.macro_name == "turret_arg_m_laser_02_mk1_macro"));

    // Which modules have groups, and what a group holds: (kind, size) -> slots.
    let mut by_class: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let mut shapes: BTreeMap<String, usize> = BTreeMap::new();
    for m in c.station_modules().unwrap() {
        let module = c.module(&m.macro_name).unwrap();
        let groups = module.equipment_groups();
        let entry = by_class.entry(m.class.clone()).or_default();
        entry.0 += 1;
        if groups.is_empty() {
            continue;
        }
        entry.1 += 1;
        for g in groups {
            let mut slots: BTreeMap<String, usize> = BTreeMap::new();
            for s in module.equipment_slots.iter().filter(|s| s.group == g) {
                *slots.entry(format!("{} {}", x4sd_core::catalog::size_of(&s.tags).unwrap_or("?"), s.kind)).or_default() += 1;
            }
            *shapes.entry(format!("{slots:?}")).or_default() += 1;
        }
        // Every group can take some buildable equipment of each kind it has slots for.
        for g in module.equipment_groups() {
            for kind in ["turrets", "shields"] {
                let slots: Vec<_> = module.group_slots(g, kind).collect();
                let fitting: Vec<&str> = equipment.iter().filter(|e| slots.iter().all(|s| e.fits(s))).map(|e| e.macro_name.as_str()).collect();
                if !slots.is_empty() && fitting.is_empty() {
                    eprintln!("  nothing fits {} {g} {kind}: {:?}", m.macro_name, slots[0].tags);
                }
                // Xenon equipment (tagged `xenon`) fits no module players build.
                if m.sets.iter().any(|s| s == "factory") {
                    assert!(!slots.is_empty() || fitting.is_empty());
                    assert!(fitting.iter().all(|f| !f.contains("_xen_")), "{} {g}: {fitting:?}", m.macro_name);
                }
            }
        }
    }
    eprintln!("modules with groups, per class (modules, with groups): {by_class:?}");
    eprintln!("group shapes: {shapes:?}");

    let library = sample("research/extracted/base/libraries/constructionplans.xml").unwrap_or_default();
    for (set, plans) in [("player plans", &plans), ("library plans", &library)] {
        let (mut checked, mut misfits, mut counts) = (0, Vec::new(), BTreeMap::<String, usize>::new());
        for plan in plans.iter() {
            for e in &plan.entries {
                let module = c.module(&e.macro_name).unwrap();
                for u in &e.upgrades {
                    let slots: Vec<_> = module.group_slots(&u.group, &u.kind).collect();
                    let fits = c.equipment().unwrap().iter().find(|x| x.macro_name.eq_ignore_ascii_case(&u.macro_name)).map(|x| slots.iter().all(|s| x.fits(s)));
                    *counts.entry(format!("{} exact {:?} of {} slots", u.kind, u.exact, slots.len())).or_default() += 1;
                    checked += 1;
                    if slots.is_empty() || fits != Some(true) {
                        misfits.push(format!("[{}] {} {} {} {}: {} slots, fits {fits:?}", plan.name, e.index, e.macro_name, u.group, u.macro_name, slots.len()));
                    }
                }
            }
        }
        eprintln!("{set}: {checked} equipped groups; {counts:?}; misfits: {misfits:?}");
        assert!(misfits.is_empty(), "{set}");
    }
}

/// Equipping a station: one group by hand, every group by picks and by level,
/// copying to modules of the same type; each is one undo step, and the result
/// validates, costs more and survives writing the plan.
#[test]
fn equip_station() {
    use x4sd_core::equipment::{module_groups, Fill, Level};
    use x4sd_core::layout::{auto_place, Options};
    let Some(c) = catalog() else { return };
    let mut d = Design::new("equipped");
    let modules = [("defence_arg_disc_01_macro", 1), ("prod_arg_foodrations_macro", 2), ("storage_arg_l_container_01_macro", 1)];
    let modules: Vec<(String, u32)> = modules.iter().map(|(m, n)| (m.to_string(), *n)).collect();
    auto_place(c, &mut d, &modules, &Options { grow_plot: true, race: None }).unwrap();
    let index_of = |d: &Design, m: &str| d.plan().entries.iter().filter(|e| e.macro_name == m).map(|e| e.index).collect::<Vec<_>>();
    let disc = index_of(&d, "defence_arg_disc_01_macro")[0];
    let food = index_of(&d, "prod_arg_foodrations_macro");
    let equipment = c.equipment().unwrap();
    let groups = |d: &Design, i: u32| {
        let e = d.plan().entries.iter().find(|e| e.index == i).unwrap();
        module_groups(equipment, &c.module(&e.macro_name).unwrap(), &e.upgrades)
    };
    for g in groups(&d, disc) {
        eprintln!("disc {} {}: {} x {:?}, {} fit", g.group, g.kind, g.slots, g.size, g.fitting.len());
    }
    let cost = |d: &Design| c.wares().unwrap().plan_cost(d.plan(), None, "default").value;
    let bare = cost(&d);

    // One group by hand: a medium laser turret on the first food module's turret slots.
    let g = groups(&d, food[0]).into_iter().find(|g| g.kind == "turrets").unwrap();
    let laser = g.fitting.iter().find(|e| e.macro_name == "turret_arg_m_laser_02_mk1_macro").unwrap().macro_name.clone();
    d.set_equipment(c, food[0], &g.group, "turrets", Some((&laser, g.slots))).unwrap();
    assert_eq!(groups(&d, food[0]).iter().find(|x| x.group == g.group && x.kind == "turrets").unwrap().current, Some((laser.clone(), g.slots)));
    assert!(d.set_equipment(c, food[0], &g.group, "turrets", Some(("shield_arg_m_standard_02_mk1_macro", 1))).is_err(), "a shield is no turret");
    assert!(d.set_equipment(c, food[0], &g.group, "turrets", Some((&laser, g.slots + 1))).is_err(), "more than the slots");
    assert!(d.set_equipment(c, food[0], "group99", "turrets", Some((&laser, 1))).is_err(), "no such group");
    // Copy to the other food module.
    assert_eq!(d.copy_equipment(food[0]).unwrap(), 1);
    assert_eq!(d.plan().entries.iter().find(|e| e.index == food[1]).unwrap().upgrades, d.plan().entries.iter().find(|e| e.index == food[0]).unwrap().upgrades);
    assert!(d.undo());
    assert!(d.plan().entries.iter().find(|e| e.index == food[1]).unwrap().upgrades.is_empty(), "the copy is one undo step");

    // Every group by picks: one M turret, one M shield, one L turret, one L shield.
    let all: Vec<u32> = d.plan().entries.iter().map(|e| e.index).collect();
    let picks = ["turret_arg_m_plasma_02_mk1_macro", "shield_arg_m_standard_02_mk1_macro", "turret_arg_l_plasma_01_mk1_macro", "shield_arg_l_standard_01_mk1_macro"];
    let (filled, cleared, skipped) = d.equip_modules(c, &all, &Fill::Picks(picks.iter().map(|s| s.to_string()).collect())).unwrap();
    eprintln!("picks: {filled} filled, {cleared} cleared, {skipped} skipped; cost {bare:.0} -> {:.0} Cr", cost(&d));
    assert!(filled > 0 && skipped == 0 && cleared == 0);
    assert!(groups(&d, disc).iter().all(|g| g.current.as_ref().is_some_and(|(_, n)| *n == g.slots)), "every disc group is full");
    assert_eq!(validate(d.plan(), c), Vec::<Issue>::new());
    assert!(cost(&d) > bare);
    // The plan file keeps it, with `exact` as the game writes it.
    let xml = x4sd_core::plan::write_plans(std::slice::from_ref(d.plan()));
    assert!(xml.contains(r#"<turrets macro="turret_arg_m_plasma_02_mk1_macro" group="#) && xml.contains(r#" exact="2"/>"#));
    assert!(xml.contains(r#"<shields macro="shield_arg_m_standard_02_mk1_macro" group="group01"/>"#), "one of one shield slot: no exact");
    assert_eq!(&parse_plans(&xml).unwrap()[0], d.plan());

    // Levels: high costs more than low; none removes everything; each is one undo step.
    d.equip_modules(c, &all, &Fill::Level(Level::Low)).unwrap();
    let low = cost(&d);
    d.equip_modules(c, &all, &Fill::Level(Level::High)).unwrap();
    let high = cost(&d);
    eprintln!("levels: low {low:.0} Cr, high {high:.0} Cr; disc high: {:?}", groups(&d, disc).iter().map(|g| g.current.clone()).collect::<Vec<_>>());
    assert!(high > low && low > bare);
    assert_eq!(validate(d.plan(), c), Vec::<Issue>::new());
    d.equip_modules(c, &all, &Fill::Level(Level::None)).unwrap();
    assert!(d.plan().entries.iter().all(|e| e.upgrades.is_empty()));
    assert!(d.undo());
    assert!((cost(&d) - high).abs() < 1e-6);

    // validate catches a turret on a shield slot (a hand-edited plan).
    let mut plan = d.plan().clone();
    let e = plan.entries.iter_mut().find(|e| e.index == disc).unwrap();
    e.upgrades[0].macro_name = laser.clone();
    e.upgrades[0].kind = "shields".to_owned();
    let issues = validate(&plan, c);
    assert!(issues.iter().any(|i| matches!(i, Issue::EquipmentMisfit { .. })), "{issues:?}");
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

/// Overlap calibration (docs/x4-construction-plans.md, "Overlaps"): how deep the
/// collision boxes of modules joined at a snap intersect (they interlock by
/// design, so joints are exempt), and which other pairs intersect, in the player's
/// plans (built in game) and the game's library plans.
#[test]
fn overlap_calibration() {
    use x4sd_core::layout::{intersection_depth, meshes_intersect, overlaps, Body, OVERLAP_TOLERANCE_M};
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml")) else { return };
    let start = Instant::now();
    let modules = c.station_modules().unwrap();
    let fallback: Vec<&str> = modules
        .iter()
        .filter(|m| {
            let module = c.module(&m.macro_name).unwrap();
            !module.collision.is_empty() && *c.collision_boxes(&m.macro_name).unwrap() == module.shape
        })
        .map(|m| m.macro_name.as_str())
        .collect();
    let counts: Vec<usize> = modules.iter().map(|m| c.collision_boxes(&m.macro_name).unwrap().len()).collect();
    eprintln!(
        "collision boxes of {} modules in {:?}: {} to {} per module; unreadable collision meshes: {fallback:?}",
        modules.len(),
        start.elapsed(),
        counts.iter().min().unwrap(),
        counts.iter().max().unwrap()
    );
    assert!(fallback.is_empty());

    let player: Vec<Plan> = plans.into_iter().filter(|p| !p.name.starts_with("X4SD")).collect();
    let library = sample("research/extracted/base/libraries/constructionplans.xml").unwrap_or_default();
    for (set, plans) in [("player plans", &player), ("library plans", &library)] {
        let start = Instant::now();
        let (mut joints, mut others) = (Vec::new(), Vec::new());
        for plan in plans {
            let bodies: Vec<_> = plan
                .entries
                .iter()
                .filter_map(|e| {
                    let (m, boxes) = (c.module(&e.macro_name).ok()?, c.collision_boxes(&e.macro_name).ok()?);
                    Some((e, Body::new(&boxes, &m.snaps, &Transform::from_offset(e.offset.as_ref()))))
                })
                .collect();
            for (i, (a, body_a)) in bodies.iter().enumerate() {
                for (b, body_b) in &bodies[i + 1..] {
                    let Some(depth) = intersection_depth(body_a, body_b, 0.0) else { continue };
                    let what = format!("[{}] {} {} / {} {}", plan.name, a.index, a.macro_name, b.index, b.macro_name);
                    if body_a.joined(body_b) { joints.push(depth) } else { others.push((depth, what)) }
                }
            }
        }
        joints.sort_by(|a, b| b.total_cmp(a));
        others.sort_by(|a, b| b.0.total_cmp(&a.0));
        let deeper = |v: &[f64], t: f64| v.iter().filter(|&&d| d > t).count();
        let other_depths: Vec<f64> = others.iter().map(|o| o.0).collect();
        eprintln!(
            "{set}: {} joints intersect (> 1 m: {}, > 20 m: {}, deepest {:.1} m); {} other pairs intersect (> 1 m: {}, > 20 m: {}), checked in {:?}",
            joints.len(),
            deeper(&joints, 1.0),
            deeper(&joints, 20.0),
            joints.first().copied().unwrap_or(0.0),
            others.len(),
            deeper(&other_depths, OVERLAP_TOLERANCE_M),
            deeper(&other_depths, 20.0),
            start.elapsed()
        );
        for (depth, what) in others.iter().take(if set == "player plans" { 20 } else { 5 }) {
            eprintln!("  {depth:6.1} m  {what}");
        }
    }
    // The boxes flag 14 pairs in the player's plans (more than 1 m), but on the
    // collision meshes none intersect: the game built these stations, overlap
    // detection on (user, 2026-10-14). `overlaps` confirms box pairs on the meshes.
    let start = Instant::now();
    let found: usize = player.iter().map(|p| overlaps(p, c).len()).sum();
    eprintln!("player plans: {found} overlaps on the meshes, in {:?} (meshes loaded)", start.elapsed());
    let start = Instant::now();
    player.iter().for_each(|p| drop(overlaps(p, c)));
    eprintln!("  again, meshes cached: {:?}", start.elapsed());
    assert_eq!(found, 0);
    let in_library: Vec<String> =
        library.iter().flat_map(|p| overlaps(p, c).into_iter().map(move |(a, b, d)| format!("[{}] {a}/{b} {d:.0} m", p.name))).collect();
    eprintln!("library plans: {} overlaps on the meshes (of 84 box pairs), e.g. {:?}", in_library.len(), &in_library[..in_library.len().min(6)]);

    // The mesh test does find real overlaps: a module shifted 30 m into a copy of itself.
    let mesh = c.collision_mesh("prod_ter_medicalsupplies_macro").unwrap().unwrap();
    let at = |x: f64| Transform { position: [x, 0.0, 0.0], ..Transform::IDENTITY };
    eprintln!("medical supplies collision mesh: {} triangles, {} tree nodes", mesh.triangles.len(), mesh.nodes.len());
    assert!(meshes_intersect(&mesh, &at(0.0), &mesh, &at(30.0)));
    assert!(!meshes_intersect(&mesh, &at(0.0), &mesh, &at(5000.0)));
}

/// Player plans: the only real problem is the deliberately moved casino of test C
/// (entry 32, 1 km up), which also overlaps the modules it was moved into. The
/// plans as built have no overlaps (see `overlap_calibration`).
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
    assert!(matches!(&found[0], (name, Issue::SnapGap { index: 32, metres }) if name == "X4SD test C" && (*metres - 1000.0).abs() < 0.01));
    let rest = &found[1..];
    assert!(!rest.is_empty() && rest.iter().all(|(name, i)| name == "X4SD test C" && matches!(i, Issue::Overlap { index, other } if *index == 32 || *other == 32)), "{found:?}");
}

/// Library plans: of the 31 off-link entries in hand-made faction plans, the 17
/// that are metres off (2.4 m to 400 m); 14 more are 1-3.5 cm off, within tolerance.
#[test]
fn validates_library_plans() {
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/base/libraries/constructionplans.xml")) else { return };
    let issues: Vec<Issue> = plans.iter().flat_map(|p| validate(p, c)).collect();
    let gaps: Vec<f64> = issues.iter().filter_map(|i| match i { Issue::SnapGap { metres, .. } => Some(*metres), _ => None }).collect();
    let overlaps = issues.iter().filter(|i| matches!(i, Issue::Overlap { .. })).count();
    let other: Vec<_> = issues.iter().filter(|i| !matches!(i, Issue::SnapGap { .. } | Issue::Overlap { .. })).collect();
    eprintln!("library: {} gaps, {overlaps} overlaps, other {other:?}", gaps.len());
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
