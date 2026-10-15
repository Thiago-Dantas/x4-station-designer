//! Tests against a real X4 install (found like the app finds it: `X4_GAME_DIR`,
//! the folder picked in the builder, Steam, GOG) and the extracted samples in `research/extracted/`. Skipped when
//! those are missing.

use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Instant;

use x4sd_core::catalog::Catalog;
use x4sd_core::design::Design;
use x4sd_core::game::{find_game_dir, find_profile_dir, is_game_dir};
use x4sd_core::geometry::{attach, fit, Transform};
use x4sd_core::import::write_import_file;
use x4sd_core::plan::{parse_plans, Plan};
use x4sd_core::validate::{validate, Issue};

fn catalog() -> Option<&'static Catalog> {
    static CATALOG: OnceLock<Option<Catalog>> = OnceLock::new();
    CATALOG
        .get_or_init(|| {
            let Some(game) = find_game_dir().filter(|g| is_game_dir(g)) else {
                eprintln!("skipped: no X4 install found");
                return None;
            };
            let start = Instant::now();
            let c = Catalog::load(&game, find_profile_dir().as_deref()).unwrap();
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
        modules: Vec::new(),
        race: race.map(str::to_owned),
        workforce: true,
        storage_hours: 6.0,
        storage: Vec::new(),
        buy: buy.iter().map(|w| w.to_string()).collect(),
        sunlight: 1.0,
        method: "default".to_owned(),
        owned: None,
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
        "[{}] placed {} in {elapsed:?} with connectors {connectors:?} ({:.1}% of the {:.1} M Cr station); docks blocked {}; unplaced {:?}; plot {plot}",
        d.plan().name,
        r.placed.len(),
        100.0 * connector_cost / total_cost,
        total_cost / 1e6,
        r.docks_blocked,
        r.unplaced
    );
    // Every module that was there, docks and piers too, stays where it was.
    let same = |a: &x4sd_core::plan::Entry, b: &x4sd_core::plan::Entry| a.index == b.index && a.macro_name == b.macro_name && a.offset == b.offset;
    for b in &before.entries {
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
    let class = |e: &x4sd_core::plan::Entry| c.module(&e.macro_name).unwrap().class.clone();
    assert!(d.plan().entries.iter().rev().take(2).all(|e| ["pier", "dockarea"].contains(&class(e).as_str())));
    let docks: Vec<u32> = d.plan().entries.iter().rev().take(2).map(|e| e.index).collect();
    assert_clear_out(c, d, &docks, station_centre(c, d));
}

/// The centre of the station's bounds.
fn station_centre(c: &Catalog, d: &Design) -> [f64; 3] {
    let parts: Vec<_> = d.plan().entries.iter().filter_map(|e| x4sd_core::plot::entry_bounds(d.plan(), c, e.index)).collect();
    let (lo, hi) = parts.iter().fold(parts[0], |(l, h), (a, b)| (std::array::from_fn(|i| l[i].min(a[i])), std::array::from_fn(|i| h[i].max(b[i]))));
    std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0)
}

/// The line from `station` out through each of `docks`, for 3 km, passes through no other module.
fn assert_clear_out(c: &Catalog, d: &Design, docks: &[u32], station: [f64; 3]) {
    use x4sd_core::layout::Body;
    let bodies: Vec<(u32, Body)> = d
        .plan()
        .entries
        .iter()
        .map(|e| (e.index, Body::new(&c.collision_boxes(&e.macro_name).unwrap(), &c.module(&e.macro_name).unwrap().snaps, &d.world(e.index).unwrap())))
        .collect();
    for (index, body) in bodies.iter().filter(|(i, _)| docks.contains(i)) {
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
        modules: Vec::new(),
        race: race.map(str::to_owned),
        workforce: true,
        storage_hours: 6.0,
        storage: Vec::new(),
        buy: Vec::new(),
        sunlight: 1.0,
        method: "default".to_owned(),
        owned: None,
    };
    let planned = |d: &Design, r: &Request| -> Vec<(String, u32)> {
        plan_production(c, &x4sd_core::planner::module_counts(d.plan()), r).unwrap().modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect()
    };
    let grow = Options { grow_plot: true, race: None, layered: false, ..Default::default() };

    let mut d = Design::new("100 solar");
    let modules = planned(&d, &req(&[("energycells", 100)], None));
    let r = place_and_check(c, &mut d, &modules, &grow);
    assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
    assert_eq!(r.placed.len() as u32, modules.iter().map(|m| m.1).sum::<u32>());
    // Docks are placed last, so they come last in the build order, with nothing in
    // their way out from the station centre.
    assert_docks_outside(c, &d);
    // A second run (10 more solar modules) leaves the docks where they are
    // (place_and_check: nothing moves) and keeps their space clear: the pier's
    // exclusion zones, and the dock area's corridor out from the station as it was.
    let dock_areas: Vec<u32> = d.plan().entries.iter().filter(|e| c.module(&e.macro_name).unwrap().class == "dockarea").map(|e| e.index).collect();
    let centre = station_centre(c, &d);
    let more = place_and_check(c, &mut d, &[("prod_gen_energycells_macro".to_owned(), 10)], &grow);
    eprintln!("second run: {} placed, {} without a clear corridor", more.placed.len(), more.docks_blocked);
    assert_eq!((more.placed.len(), more.docks_blocked), (10, 0));
    assert_eq!(x4sd_core::layout::blocked_zones(d.plan(), c), Vec::<(u32, u32)>::new());
    assert_clear_out(c, &d, &dock_areas, centre);
    let path = std::env::temp_dir().join("x4sd-auto-place.xml");
    std::fs::write(&path, x4sd_core::plan::write_plans(std::slice::from_ref(d.plan()))).unwrap();
    eprintln!("saved to {}", path.display());

    // Terran modules get Terran connection structures.
    let mut d = Design::new("terran hull parts");
    let modules = planned(&d, &req(&[("hullparts", 2)], Some("terran")));
    let r = place_and_check(c, &mut d, &modules, &grow);
    assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
    assert!(r.connectors.iter().all(|(_, m)| m.starts_with("struct_ter_")), "{:?}", r.connectors);

    // Without growing, the default plot (2 km per side) holds only some of the solar station.
    let mut d = Design::new("small plot");
    let modules = planned(&d, &req(&[("energycells", 100)], None));
    let r = place_and_check(c, &mut d, &modules, &Options { grow_plot: false, race: None, layered: false, ..Default::default() });
    assert!(!r.placed.is_empty() && !r.unplaced.is_empty());

    // An existing station: the planner's fix for the scrapyard, placed onto it.
    let Some(plans) = sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml") else { return };
    let mut d = Design::from_plan(plans.iter().find(|p| p.name == "TerranScrapyard").unwrap().clone());
    let modules = planned(&d, &req(&[], Some("terran")));
    let r = place_and_check(c, &mut d, &modules, &grow);
    eprintln!("scrapyard fix: {} modules to place", modules.iter().map(|m| m.1).sum::<u32>());
    assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
}

/// A big station whose modules are mostly dead ends (one snap: Teladi-planned
/// claytronics, microchips, water) keeps room to grow and places everything (user
/// report 2026-10-15: 38 of 461 placed, every free snap capped), also with fewer
/// connection structures. Since modules no longer stack (facing rule), it seals
/// up anyway; breaking it up (`Options::break_up`) places the last few.
#[test]
fn auto_place_keeps_room_to_grow() {
    use x4sd_core::layout::Options;
    use x4sd_core::planner::{plan_production, Request};
    let Some(c) = catalog() else { return };
    let req = Request {
        targets: vec![("claytronics".to_owned(), 50)],
        modules: Vec::new(),
        race: Some("teladi".to_owned()),
        workforce: true,
        storage_hours: 6.0,
        storage: Vec::new(),
        buy: Vec::new(),
        sunlight: 1.0,
        method: "default".to_owned(),
        owned: None,
    };
    let modules: Vec<(String, u32)> = plan_production(c, &[], &req).unwrap().modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect();
    for fewer_connectors in [false, true] {
        let mut d = Design::new(if fewer_connectors { "50 claytronics, fewer connectors" } else { "50 claytronics" });
        let r = place_and_check(c, &mut d, &modules, &Options { grow_plot: true, race: Some("teladi".to_owned()), fewer_connectors, break_up: true, ..Default::default() });
        assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
    }
}

/// "Fewer connection structures": a new station ("100 solar") and a rebuilt one
/// (TerranScrapyard, Optimize without keeping its shape) need fewer of them than
/// with the default placement, and still place everything without overlaps.
#[test]
#[ignore = "expectations from layouts that stacked modules (before the facing rule); re-tune: tasks/workflow-retune-placement.md"]
fn fewer_connectors() {
    use x4sd_core::layout::{optimize, overlaps, OptimizeOptions, Options};
    use x4sd_core::planner::{module_counts, plan_production, Request};
    let Some(c) = catalog() else { return };
    let req = |targets: &[(&str, u32)], race: Option<&str>| Request {
        targets: targets.iter().map(|(w, n)| (w.to_string(), *n)).collect(),
        modules: Vec::new(),
        race: race.map(str::to_owned),
        workforce: true,
        storage_hours: 6.0,
        storage: Vec::new(),
        buy: Vec::new(),
        sunlight: 1.0,
        method: "default".to_owned(),
        owned: None,
    };
    let planned = |p: &x4sd_core::plan::Plan, r: &Request| -> Vec<(String, u32)> {
        plan_production(c, &module_counts(p), r).unwrap().modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect()
    };
    let solar = planned(&Design::new("").plan().clone(), &req(&[("energycells", 100)], None));
    let mut connectors = Vec::new();
    for fewer_connectors in [false, true] {
        let mut d = Design::new(if fewer_connectors { "100 solar, fewer connectors" } else { "100 solar" });
        let r = place_and_check(c, &mut d, &solar, &Options { grow_plot: true, fewer_connectors, ..Default::default() });
        assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
        connectors.push(r.connectors.len());
    }
    assert!(connectors[1] < connectors[0], "{connectors:?}");

    let Some(plans) = sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml") else { return };
    let yard = plans.iter().find(|p| p.name == "TerranScrapyard").unwrap();
    let fixes = planned(yard, &req(&[], Some("terran")));
    let yard_load = x4sd_core::perf::load(yard, c);
    eprintln!("[scrapyard] {yard_load:?}");
    assert!(yard_load.connectors > 0);
    // Turrets and shields count the slots filled (`exact`): counted in the file, 144
    // turret entries filling 288 slots and 144 shields.
    let defence = x4sd_core::perf::load(plans.iter().find(|p| p.name == "defense station").unwrap(), c);
    assert_eq!((defence.modules, defence.turrets, defence.shields), (14, 288, 144), "{defence:?}");
    let docks_added: u32 = fixes.iter().filter(|(m, _)| ["dockarea", "pier"].contains(&c.module(m).unwrap().class.as_str())).map(|f| f.1).sum();
    let mut connectors = Vec::new();
    for fewer_connectors in [false, true] {
        let mut d = Design::from_plan(yard.clone());
        let options = OptimizeOptions { keep_shape: false, grow_plot: true, race: Some("terran".to_owned()), fewer_connectors, owned: None, partial: false };
        let r = optimize(c, &mut d, &fixes, &options).unwrap();
        let plot = x4sd_core::plot::required_plot(d.plan(), c).unwrap();
        let load = x4sd_core::perf::load(d.plan(), c);
        eprintln!("[scrapyard rebuilt, fewer connectors: {fewer_connectors}] {load:?}, plot {plot}");
        assert!(overlaps(d.plan(), c).is_empty());
        assert!(plot.is_allowed(), "{plot}");
        // A rebuild removes every connection structure, so all of them are new ones.
        assert_eq!((load.modules as usize, load.connectors as usize), (d.plan().entries.len(), r.placement.connectors.len()));
        assert_eq!((load.turrets, load.shields, load.docks), (yard_load.turrets, yard_load.shields, yard_load.docks + docks_added), "equipment and docks stay");
        assert!(load.size_km.iter().zip(plot.size()).all(|(s, p)| *s > 0.0 && *s <= f64::from(p)), "{:?} in {plot}", load.size_km);
        connectors.push(load.connectors);
    }
    assert!(connectors[1] < connectors[0], "{connectors:?}");
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

/// Optimize (the x4ep_planner's, plus our re-layout): TerranScrapyard with the
/// modules that fix its shortfalls, re-laid out keeping its shape and rebuilt.
/// Nothing is lost, equipment stays, no overlaps, docks outside, layers inside
/// out, one undo step.
#[test]
fn optimize_station() {
    use std::collections::BTreeMap;
    use x4sd_core::layout::{optimize, overlaps, OptimizeOptions};
    use x4sd_core::planner::{module_counts, plan_production, Request};
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml")) else { return };
    let yard = plans.iter().find(|p| p.name == "TerranScrapyard").unwrap();
    let req = Request { targets: Vec::new(), modules: Vec::new(), race: Some("terran".to_owned()), workforce: true, storage_hours: 6.0, storage: Vec::new(), buy: Vec::new(), sunlight: 1.0, method: "default".to_owned(), owned: None };
    let fixes: Vec<(String, u32)> = plan_production(c, &module_counts(yard), &req).unwrap().modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect();
    let class = |m: &str| c.module(m).unwrap().class.clone();
    let counts = |p: &x4sd_core::plan::Plan| {
        let mut n: BTreeMap<String, u32> = BTreeMap::new();
        for e in p.entries.iter().filter(|e| class(&e.macro_name) != "connectionmodule") {
            *n.entry(e.macro_name.clone()).or_default() += 1;
        }
        n
    };
    let equipped = |p: &x4sd_core::plan::Plan| p.entries.iter().filter(|e| !e.upgrades.is_empty()).count();
    for keep_shape in [true, false] {
        let mut d = Design::from_plan(yard.clone());
        let start = Instant::now();
        let r = optimize(c, &mut d, &fixes, &OptimizeOptions { keep_shape, grow_plot: true, race: Some("terran".to_owned()), ..Default::default() }).unwrap();
        let elapsed = start.elapsed();
        let plot = x4sd_core::plot::required_plot(d.plan(), c).unwrap();
        // Mean distance from the station centre of the re-placed modules, per layer.
        let placed: Vec<u32> = r.placement.placed.iter().map(|p| p.0).collect();
        let bounds: Vec<_> = d.plan().entries.iter().filter_map(|e| x4sd_core::plot::entry_bounds(d.plan(), c, e.index)).collect();
        let (lo, hi) = bounds.iter().fold(bounds[0], |(l, h), (a, b)| (std::array::from_fn(|i| l[i].min(a[i])), std::array::from_fn(|i| h[i].max(b[i]))));
        let centre: [f64; 3] = std::array::from_fn(|i| (lo[i] + hi[i]) / 2.0);
        let mut layers: BTreeMap<&str, (f64, u32)> = BTreeMap::new();
        for e in d.plan().entries.iter().filter(|e| placed.contains(&e.index)) {
            let m = c.module(&e.macro_name).unwrap();
            let layer = if ["dockarea", "pier"].contains(&m.class.as_str()) { "3 docks" } else if m.equipment_slots.is_empty() { "1 no slots" } else { "2 defensible" };
            let (l, h) = x4sd_core::plot::entry_bounds(d.plan(), c, e.index).unwrap();
            let dist = (0..3).map(|i| ((l[i] + h[i]) / 2.0 - centre[i]).powi(2)).sum::<f64>().sqrt();
            let slot = layers.entry(layer).or_default();
            slot.0 += dist;
            slot.1 += 1;
        }
        let means: BTreeMap<&str, f64> = layers.iter().map(|(k, (s, n))| (*k, s / f64::from(*n))).collect();
        eprintln!(
            "[keep shape: {keep_shape}] {elapsed:?}: moved {}, added {}, connectors -{} +{}, docks blocked {}; plot {plot}; mean distance per layer {:?}",
            r.moved,
            fixes.iter().map(|f| f.1).sum::<u32>(),
            r.connectors_removed,
            r.placement.connectors.len(),
            r.placement.docks_blocked,
            means.iter().map(|(k, v)| format!("{k} {v:.0} m")).collect::<Vec<_>>()
        );
        // Nothing lost: every module the yard had, plus the fixes.
        let mut expected = counts(yard);
        for (m, n) in &fixes {
            *expected.entry(m.clone()).or_default() += n;
        }
        assert_eq!(counts(d.plan()), expected);
        assert_eq!(equipped(d.plan()), equipped(yard), "equipment stays");
        assert!(overlaps(d.plan(), c).is_empty());
        assert!(plot.is_allowed(), "{plot}");
        assert_docks_outside(c, &d);
        assert!(means["1 no slots"] < means["2 defensible"], "{means:?}");
        assert!(d.undo());
        assert_eq!(d.plan(), yard, "one undo step");
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
    auto_place(c, &mut d, &modules, &Options { grow_plot: true, race: None, layered: false, ..Default::default() }).unwrap();
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
    assert_eq!(d.plot(), Some(Plot::around(2)));
    let hub = d.add_root(c, "struct_ter_cross_01_macro", Transform::IDENTITY).unwrap();
    let beam = c.module("struct_ter_base_02_macro").unwrap();
    let mut end = (hub, d.free_snaps(c, hub).unwrap()[0].name.clone());
    for _ in 0..4 {
        let next = d.attach(c, "struct_ter_base_02_macro", &beam.snaps[0].name, end.0, &end.1, 0.0).unwrap();
        end = (next, d.free_snaps(c, next).unwrap()[0].name.clone());
    }
    let plot = d.plot().unwrap();
    assert!(!validate_plot(d.plan(), c, &plot).is_empty(), "a 3.6 km arm does not fit 2 km per side");
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
/// plans as built have no overlaps (see `overlap_calibration`). The megashipyard's
/// observation deck (173) is placed apart, unattached; the other modules without a
/// link meet a later module at a snap.
#[test]
fn validates_player_plans() {
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml")) else { return };
    let mut found = Vec::new();
    for plan in &plans {
        for issue in validate(plan, c) {
            if plan.name == "megashipyard terran" && issue == (Issue::Unattached { index: 173 }) {
                continue;
            }
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
/// The faction plans also put modules in 18 exclusion zones (D-12): 17 of fabrication
/// bays, 1 of a pier. The game builds them; the validator still reports them.
/// 14 modules are placed apart, unattached (13 defence platforms, a scanning array).
#[test]
fn validates_library_plans() {
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/base/libraries/constructionplans.xml")) else { return };
    let issues: Vec<Issue> = plans.iter().flat_map(|p| validate(p, c)).collect();
    let gaps: Vec<f64> = issues.iter().filter_map(|i| match i { Issue::SnapGap { metres, .. } => Some(*metres), _ => None }).collect();
    let overlaps = issues.iter().filter(|i| matches!(i, Issue::Overlap { .. })).count();
    let zones: Vec<&str> = issues.iter().filter_map(|i| match i { Issue::BlocksZone { owner_class, .. } => Some(owner_class.as_str()), _ => None }).collect();
    let unattached = issues.iter().filter(|i| matches!(i, Issue::Unattached { .. })).count();
    let other: Vec<_> = issues.iter().filter(|i| !matches!(i, Issue::SnapGap { .. } | Issue::Overlap { .. } | Issue::BlocksZone { .. } | Issue::Unattached { .. })).collect();
    eprintln!("library: {} gaps, {overlaps} overlaps, {} in exclusion zones, other {other:?}", gaps.len(), zones.len());
    assert_eq!(gaps.len(), 17);
    assert!(gaps.iter().all(|m| *m > 1.0), "{gaps:?}");
    assert_eq!(zones.len(), 18, "{zones:?}");
    assert_eq!(zones.iter().filter(|c| **c == "pier").count(), 1, "{zones:?}");
    assert_eq!(unattached, 14);
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

#[test]
fn clearance_zones() {
    use x4sd_core::layout::blocked_zones;
    let Some(c) = catalog() else { return };
    assert_eq!(c.exclusion_size("ship_xl"), Some([4000.0, 4000.0, 100_000.0]));
    assert_eq!(c.exclusion_size("ship_m"), Some([180.0, 180.0, 500.0]));
    let towards = |m: &str| -> Vec<(String, [f64; 3])> {
        let m = c.module(m).unwrap();
        m.clearance.iter().map(|z| (z.tag.clone(), z.transform.rotation.rotate([0.0, 0.0, 1.0]).map(|v| v.round()))).collect()
    };
    // The 3-Dock E Pier's three docks face away from its snaps (+X, ±Y).
    assert_eq!(towards("pier_ter_harbor_03_macro"), vec![("ship_xl".to_owned(), [-1.0, 0.0, 0.0]); 3]);
    // An XL bay opens at both ends, at right angles to its snaps (±X).
    let bay = towards("buildmodule_ter_ships_xl_macro");
    assert_eq!(bay.iter().map(|(_, d)| d[2]).collect::<Vec<_>>(), vec![1.0, -1.0], "{bay:?}");
    assert!(towards("hab_ter_l_01_macro").is_empty());

    // Calibration: the player's game-built plans block nothing (back-to-back piers
    // are joined at a snap); our first Optimize of megashipyard terran blocked a lot.
    let Some(plans) = sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml") else { return };
    let start = Instant::now();
    for plan in plans.iter().filter(|p| !p.name.starts_with("X4SD")) {
        let blocked = blocked_zones(plan, c);
        assert!(blocked.is_empty(), "[{}] {blocked:?}", plan.name);
    }
    eprintln!("player plans checked in {:?}", start.elapsed());
    let Some(optimized) = sample("research/extracted/profile-samples/megashipyard terran2.xml") else { return };
    let issues: Vec<String> = validate(&optimized[0], c).iter().filter(|i| matches!(i, Issue::BlocksZone { .. })).map(|i| i.to_string()).collect();
    eprintln!("megashipyard terran2: {} blocked, e.g. {:?}", issues.len(), &issues[..3.min(issues.len())]);
    assert!(issues.len() > 20 && issues.iter().any(|i| i.contains("approach of pier")) && issues.iter().any(|i| i.contains("exit of bay")));
}

/// Optimize keeps every pier approach and bay exit clear (D-12): the user's
/// game-built megashipyard terran, with its shortfall fixes, both modes.
#[test]
fn optimize_keeps_zones_clear() {
    use std::collections::BTreeMap;
    use x4sd_core::layout::{blocked_zones, optimize, overlaps, OptimizeOptions};
    use x4sd_core::planner::{module_counts, plan_production, Request};
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/constructionplans-20261013-1501.xml")) else { return };
    let yard = plans.iter().find(|p| p.name == "megashipyard terran").unwrap();
    let req = Request { targets: Vec::new(), modules: Vec::new(), race: Some("terran".to_owned()), workforce: true, storage_hours: 6.0, storage: Vec::new(), buy: Vec::new(), sunlight: 1.0, method: "default".to_owned(), owned: None };
    let fixes: Vec<(String, u32)> = plan_production(c, &module_counts(yard), &req).unwrap().modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect();
    let counts = |p: &x4sd_core::plan::Plan| {
        let mut n: BTreeMap<String, u32> = BTreeMap::new();
        for e in p.entries.iter().filter(|e| c.module(&e.macro_name).unwrap().class != "connectionmodule") {
            *n.entry(e.macro_name.clone()).or_default() += 1;
        }
        n
    };
    let mut expected = counts(yard);
    for (m, n) in &fixes {
        *expected.entry(m.clone()).or_default() += n;
    }
    for keep_shape in [true, false] {
        let mut d = Design::from_plan(yard.clone());
        let start = Instant::now();
        let r = optimize(c, &mut d, &fixes, &OptimizeOptions { keep_shape, grow_plot: true, race: Some("terran".to_owned()), ..Default::default() }).unwrap();
        let plot = x4sd_core::plot::required_plot(d.plan(), c).unwrap();
        let blocked = blocked_zones(d.plan(), c);
        eprintln!(
            "[keep shape: {keep_shape}] {:?}: {} entries, added {}, connectors -{} +{}, blocked {}; plot {plot}; zones blocked {:?}",
            start.elapsed(),
            d.plan().entries.len(),
            fixes.iter().map(|f| f.1).sum::<u32>(),
            r.connectors_removed,
            r.placement.connectors.len(),
            r.placement.docks_blocked,
            &blocked[..blocked.len().min(10)]
        );
        assert_eq!(counts(d.plan()), expected, "nothing lost");
        assert!(overlaps(d.plan(), c).is_empty());
        assert!(plot.is_allowed(), "{plot}");
        assert!(blocked.is_empty(), "{} blocked", blocked.len());
        assert_eq!(r.placement.docks_blocked, 0);
    }
}

/// Planner extras (workflow-planner-extras): storage minimums (the largest module
/// for the bulk, the smallest that covers the rest), extra modules (a ship bay and
/// defence discs) planned with their workforce housed, and discs placed on their
/// sides of the station with nothing in front of them.
#[test]
#[ignore = "expectations from layouts that stacked modules (before the facing rule); re-tune: tasks/workflow-retune-placement.md"]
fn planner_extras() {
    use x4sd_core::layout::{Body, Options, Side};
    use x4sd_core::planner::{plan_production, Request, Role};
    let Some(c) = catalog() else { return };
    let req = |targets: &[(&str, u32)], modules: &[(&str, u32)], storage: &[(&str, f64)]| Request {
        targets: targets.iter().map(|(w, n)| (w.to_string(), *n)).collect(),
        modules: modules.iter().map(|(m, n)| (m.to_string(), *n)).collect(),
        race: Some("argon".to_owned()),
        workforce: true,
        storage_hours: 6.0,
        storage: storage.iter().map(|(t, v)| (t.to_string(), *v)).collect(),
        buy: Vec::new(),
        sunlight: 1.0,
        method: "default".to_owned(),
        owned: None,
    };
    let listed = |p: &x4sd_core::planner::ProductionPlan| -> Vec<(String, u32)> { p.modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect() };

    // Storage alone: 1.2M container = L + M (250k), 500k solid = M, 200k liquid = M (500k).
    let p = plan_production(c, &[], &req(&[], &[], &[("container", 1.2e6), ("solid", 5e5), ("liquid", 2e5)])).unwrap();
    let mut got = listed(&p);
    got.sort();
    let want: Vec<(String, u32)> = [("storage_arg_l_container_01_macro", 1), ("storage_arg_m_container_01_macro", 1), ("storage_arg_m_liquid_01_macro", 1), ("storage_arg_m_solid_01_macro", 1)]
        .iter()
        .map(|(m, n)| (m.to_string(), *n))
        .collect();
    assert_eq!(got, want);

    // With production: each type gets at least the minimum and at least 6 hours.
    let p = plan_production(c, &[], &req(&[("hullparts", 2)], &[], &[("container", 1e6), ("solid", 5e5), ("liquid", 2e5)])).unwrap();
    for s in &p.analysis.storage {
        let min = [("container", 1e6), ("solid", 5e5), ("liquid", 2e5)].iter().find(|(t, _)| *t == s.transport).map_or(0.0, |x| x.1);
        eprintln!("[storage] {}: {:.0} m³ for {:.0} m³/h, minimum {min:.0}", s.transport, s.capacity, s.volume_per_hour);
        assert!(s.capacity >= min && s.capacity >= s.volume_per_hour * 6.0 - 1e-6, "{s:?}");
    }

    // Extras: an L fabrication bay and 6 discs, role extra, their workers housed.
    let (bay, disc) = ("buildmodule_gen_ships_l_macro", "defence_arg_disc_01_macro");
    let p = plan_production(c, &[], &req(&[("hullparts", 2)], &[(bay, 1), (disc, 6)], &[])).unwrap();
    let role = |m: &str| p.modules.iter().find(|x| x.macro_name == m).map(|x| (x.role, x.count));
    assert_eq!((role(bay), role(disc)), (Some((Role::Extra, 1)), Some((Role::Extra, 6))));
    assert!(p.analysis.workforce_fill >= 1.0 - 1e-9, "{}", p.analysis.workforce_fill);

    // Sides. Each disc has nothing in front of it. Off its half of the station only
    // when counted in sides_blocked: on this small, tightly packed station some sides
    // have no free snaps; on "100 solar" (many hubs) none is off, front/back too.
    let solar = plan_production(c, &[], &req(&[("energycells", 100)], &[(bay, 1), (disc, 6)], &[])).unwrap();
    let dir = |s: Side| match s {
        Side::Front => [0.0, 0.0, 1.0],
        Side::Back => [0.0, 0.0, -1.0],
        Side::Left => [-1.0, 0.0, 0.0],
        Side::Right => [1.0, 0.0, 0.0],
        Side::Top => [0.0, 1.0, 0.0],
        Side::Bottom => [0.0, -1.0, 0.0],
    };
    for (plan, sides) in [(&p, Side::ALL.to_vec()), (&solar, Side::ALL.to_vec()), (&solar, vec![Side::Front, Side::Back])] {
        let mut d = Design::new(&format!("{} modules, 6 discs on {sides:?}", plan.modules.iter().map(|m| m.count).sum::<u32>()));
        let options = Options { grow_plot: true, sides: vec![(disc.to_owned(), sides.clone())], ..Default::default() };
        let r = place_and_check(c, &mut d, &listed(plan), &options);
        assert!(r.unplaced.is_empty(), "{:?}", r.unplaced);
        let bodies: Vec<(u32, Body)> = d
            .plan()
            .entries
            .iter()
            .map(|e| (e.index, Body::new(&c.collision_boxes(&e.macro_name).unwrap(), &c.module(&e.macro_name).unwrap().snaps, &d.world(e.index).unwrap())))
            .collect();
        let discs: Vec<u32> = r.placed.iter().filter(|(_, m)| m == disc).map(|(i, _)| *i).collect();
        assert_eq!(discs.len(), 6);
        let mut off = 0;
        for (k, index) in discs.iter().enumerate() {
            let side = sides[k % sides.len()];
            let v = dir(side);
            let (l, h) = bodies.iter().find(|(i, _)| i == index).unwrap().1.bounds();
            let mid: [f64; 3] = std::array::from_fn(|i| (l[i] + h[i]) / 2.0);
            let out: f64 = (0..3).map(|i| mid[i] * v[i]).sum();
            // Nothing in front: samples every 50 m from its face out to 3 km.
            let face = (0..3).map(|i| if v[i] > 0.0 { h[i] } else { -l[i] }).sum::<f64>();
            let in_front: Vec<u32> = bodies
                .iter()
                .filter(|(i, b)| i != index && (1..=60).any(|s| b.contains(std::array::from_fn(|j| if v[j] != 0.0 { v[j] * (face + s as f64 * 50.0) } else { mid[j] }))))
                .map(|(i, _)| *i)
                .collect();
            eprintln!("disc #{index} {side:?}: centre {mid:.0?}, {out:.0} m out, in front: {in_front:?}");
            assert!(in_front.is_empty());
            // On its half: all of it beyond the centre plane (the origin here).
            let near = (0..3).map(|i| if v[i] > 0.0 { l[i] } else if v[i] < 0.0 { -h[i] } else { 0.0 }).sum::<f64>();
            off += u32::from(near <= 0.0);
        }
        assert_eq!(off, r.sides_blocked);
        if plan == &solar {
            assert_eq!(r.sides_blocked, 0);
        }
    }
}

#[test]
fn blueprints_from_save_limit_the_planner() {
    use std::collections::HashSet;
    use x4sd_core::layout::{auto_place, Options};
    use x4sd_core::planner::{plan_production, Request};
    use x4sd_core::save::{newest_save, owned_modules, read_blueprints};
    let Some(c) = catalog() else { return };
    let modules = c.station_modules().unwrap();

    // The newest real save, if the profile has one.
    if let Some(save) = find_profile_dir().and_then(|p| newest_save(&p)) {
        let start = Instant::now();
        let wares = read_blueprints(&save).unwrap();
        let owned = owned_modules(c, &wares).unwrap();
        eprintln!("{}: {} blueprints, {} station modules, in {:?}", save.display(), wares.len(), owned.len(), start.elapsed());
        assert!(!owned.is_empty() && owned.len() <= modules.len());
    } else {
        eprintln!("skipped the save part: no save in the profile");
    }

    // Everything but the energy-cell makers: the planner can't choose one.
    let makes = |m: &str, w: &str| c.module(m).is_ok_and(|x| x.economy.production.iter().any(|(ware, _)| ware == w));
    let owned: HashSet<String> = modules.iter().filter(|m| !makes(&m.macro_name, "energycells")).map(|m| m.macro_name.to_ascii_lowercase()).collect();
    let req = |targets: &[(&str, u32)], owned: Option<HashSet<String>>| Request {
        targets: targets.iter().map(|(w, n)| (w.to_string(), *n)).collect(),
        modules: Vec::new(),
        race: Some("argon".to_owned()),
        workforce: true,
        storage_hours: 6.0,
        storage: Vec::new(),
        buy: Vec::new(),
        sunlight: 1.0,
        method: "default".to_owned(),
        owned,
    };
    let p = plan_production(c, &[], &req(&[("energycells", 4)], Some(owned.clone()))).unwrap();
    assert_eq!(p.no_blueprint, ["energycells"]);
    assert!(p.unknown_targets.is_empty());
    let p = plan_production(c, &[], &req(&[("hullparts", 4)], Some(owned.clone()))).unwrap();
    assert!(p.modules.iter().all(|m| owned.contains(&m.macro_name.to_ascii_lowercase())), "{:?}", p.modules);
    assert!(p.no_blueprint.contains(&"energycells".to_owned()), "hull parts need energy cells: {:?}", p.no_blueprint);
    let free = plan_production(c, &[], &req(&[("hullparts", 4)], None)).unwrap();
    assert!(free.no_blueprint.is_empty() && free.modules.iter().any(|m| m.ware.as_deref() == Some("energycells")));

    // Connection structures: only owned ones, even for another race.
    let teladi: HashSet<String> = modules
        .iter()
        .filter(|m| m.class != "connectionmodule" || m.race.as_deref() == Some("teladi"))
        .map(|m| m.macro_name.to_ascii_lowercase())
        .collect();
    let mut d = Design::new("blueprints");
    let r = auto_place(c, &mut d, &[("prod_gen_energycells_macro".to_owned(), 6)], &Options { grow_plot: true, race: Some("argon".to_owned()), owned: Some(teladi.clone()), ..Default::default() }).unwrap();
    assert!(!r.connectors.is_empty());
    assert!(r.connectors.iter().all(|(_, m)| teladi.contains(&m.to_ascii_lowercase())), "{:?}", r.connectors);
}

/// Sectors and their sunlight (`mapdefaults.xml`): the base game's own value,
/// a Boron sector taking its cluster's, names resolved, no demo or Timelines
/// mission maps.
#[test]
fn sectors_and_sunlight() {
    let Some(c) = catalog() else { return };
    let sectors = c.sectors();
    eprintln!("{} sectors", sectors.len());
    let find = |m: &str| sectors.iter().find(|s| s.macro_name.eq_ignore_ascii_case(m));
    let grand_exchange = find("Cluster_01_Sector001_macro").unwrap();
    assert_eq!((grand_exchange.name.as_str(), grand_exchange.sunlight), ("Grand Exchange I", 1.23));
    assert_eq!(find("Cluster_14_Sector001_macro").unwrap().name, "Argon Prime");
    if c.extensions().iter().any(|e| e.folder == "ego_dlc_boron") {
        // From their clusters; the game's encyclopedia shows the same (user, 2026-10-15).
        for (m, name, sunlight) in [("Cluster_608_Sector001_macro", "Atreus' Clouds", 0.41), ("Cluster_609_Sector001_macro", "Menelaus' Oasis", 2.71), ("Cluster_606_Sector002_macro", "Reflected Stars", 1.18)] {
            let s = find(m).unwrap();
            assert_eq!((s.name.as_str(), s.sunlight), (name, sunlight));
        }
    }
    for s in sectors {
        let m = s.macro_name.to_ascii_lowercase();
        assert!(!m.starts_with("demo_") && !m.starts_with("timelines_map_"), "{m}");
        assert!(!s.name.contains('{') && s.sunlight >= 0.0, "{s:?}");
    }
}

/// Target rates (the planner's "≈ N/h" per target): the module the plan uses and
/// its output, the same as the plan's; solar output follows the sunlight; without
/// a blueprint, no producer and `no_blueprint`.
#[test]
fn target_rates() {
    use std::collections::HashSet;
    use x4sd_core::planner::{plan_production, target_rates, Request};
    let Some(c) = catalog() else { return };
    let req = |targets: &[(&str, u32)], sunlight: f64, owned: Option<HashSet<String>>| Request {
        targets: targets.iter().map(|(w, n)| (w.to_string(), *n)).collect(),
        modules: Vec::new(),
        race: Some("argon".to_owned()),
        workforce: true,
        storage_hours: 6.0,
        storage: Vec::new(),
        buy: Vec::new(),
        sunlight,
        method: "default".to_owned(),
        owned,
    };
    let r = req(&[("hullparts", 2), ("energycells", 3), ("ore", 1)], 1.0, None);
    let rates = target_rates(c, &r).unwrap();
    eprintln!("{rates:?}");
    let (macro_name, _, per_hour) = rates[0].producer.clone().unwrap();
    let p = plan_production(c, &[], &r).unwrap();
    assert!(p.modules.iter().any(|m| m.macro_name == macro_name && m.count == 2), "the plan's module");
    let made = p.analysis.flows.iter().find(|f| f.ware == "hullparts").unwrap().produced;
    assert!((made - 2.0 * per_hour).abs() < 1e-6, "{made} vs 2 x {per_hour}");
    assert!(rates[2].producer.is_none() && !rates[2].no_blueprint, "ore is mined");
    let sunny = target_rates(c, &req(&[("energycells", 1)], 1.23, None)).unwrap();
    let solar = |r: &[x4sd_core::planner::TargetRate]| r.iter().find(|t| t.ware == "energycells").unwrap().producer.clone().unwrap().2;
    assert!((solar(&sunny) - 1.23 * solar(&rates)).abs() < 1e-6, "solar follows the sunlight");
    let none = target_rates(c, &req(&[("hullparts", 1)], 1.0, Some(HashSet::new()))).unwrap();
    assert!(none[0].producer.is_none() && none[0].no_blueprint);
}

/// Breaking a sealed station up (user's "test" station, 2026-10-15: every free
/// snap faces into a neighbour): without it nothing of the user's extras places;
/// with it all of them, and the result has no overlaps or other problems, in one
/// undo step.
#[test]
fn break_up_opens_a_sealed_station() {
    use x4sd_core::layout::{auto_place, Options};
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/test-20261015.xml")) else { return };
    let modules: Vec<(String, u32)> = [("dockarea_ter_m_station_01_standard_macro", 5), ("pier_ter_harbor_03_macro", 10), ("defence_ter_claim_01_macro", 1)].iter().map(|(m, n)| (m.to_string(), *n)).collect();
    for break_up in [false, true] {
        let mut d = Design::from_plan(plans[0].clone());
        let before = d.plan().clone();
        let r = auto_place(c, &mut d, &modules, &Options { grow_plot: true, race: Some("terran".to_owned()), break_up, ..Default::default() }).unwrap();
        eprintln!("[break up: {break_up}] placed {}, connectors {}, broken up {}, unplaced {:?}", r.placed.len(), r.connectors.len(), r.broken_up, r.unplaced);
        if !break_up {
            assert!(r.placed.is_empty(), "sealed: nothing fits");
            continue;
        }
        assert!(r.unplaced.is_empty() && r.placed.len() == 16 && r.broken_up > 0);
        assert_eq!(validate(d.plan(), c), Vec::<Issue>::new());
        assert_eq!(d.plan().entries.len(), before.entries.len() + r.placed.len() + r.connectors.len());
        for (i, m) in r.placed.iter().chain(&r.connectors) {
            assert_eq!(&d.plan().entries.iter().find(|e| e.index == *i).unwrap().macro_name, m, "indices follow the renumbering");
        }
        assert!(d.undo());
        assert_eq!(d.plan(), &before, "one undo step");
    }
}

/// Optimize with a rebuild on the user's "test" station with 12 Terran discs
/// (2026-10-15): it fills a compact plot instead of growing arms, and defence
/// platforms and piers end up outermost (they were deep inside): nothing on the
/// line out from the station's centre through them, the defence platforms
/// spread out, each in the outer half of the station's body.
#[test]
fn optimize_puts_defence_and_piers_outside() {
    use x4sd_core::layout::{optimize, overlaps, OptimizeOptions};
    use x4sd_core::plot::{entry_bounds, required_plot};
    let (Some(c), Some(plans)) = (catalog(), sample("research/extracted/profile-samples/test-20261015-discs.xml")) else { return };
    let mut d = Design::from_plan(plans[0].clone());
    let start = Instant::now();
    let r = optimize(c, &mut d, &[], &OptimizeOptions { grow_plot: true, race: Some("terran".to_owned()), ..Default::default() }).unwrap();
    let plot = required_plot(d.plan(), c).unwrap();
    eprintln!("{:?}: {} entries, connectors -{} +{}, broken up {}, docks blocked {}; plot {plot}", start.elapsed(), d.plan().entries.len(), r.connectors_removed, r.placement.connectors.len(), r.placement.broken_up, r.placement.docks_blocked);
    assert!(overlaps(d.plan(), c).is_empty());
    assert_eq!(r.placement.docks_blocked, 0);
    assert!(plot.size().iter().all(|&s| s <= 12), "compact: {plot} (20x11x19 km before)");
    let class = |e: &x4sd_core::plan::Entry| c.module(&e.macro_name).unwrap().class.clone();
    let outside: Vec<u32> = d.plan().entries.iter().filter(|e| ["defencemodule", "pier"].contains(&class(e).as_str())).map(|e| e.index).collect();
    assert_eq!(outside.len(), 23);
    let centre = station_centre(c, &d);
    assert_clear_out(c, &d, &outside, centre);
    // Defence platforms against the station's body (what isn't docks, piers, bays,
    // defence or connection structures; piers reach 4-6 km out, past them).
    let body: Vec<[f64; 3]> = d
        .plan()
        .entries
        .iter()
        .filter(|e| !["connectionmodule", "dockarea", "pier", "defencemodule", "buildmodule"].contains(&class(e).as_str()))
        .map(|e| entry_bounds(d.plan(), c, e.index).unwrap())
        .map(|(l, h)| std::array::from_fn(|i| (l[i] + h[i]) / 2.0))
        .collect();
    let middle: [f64; 3] = std::array::from_fn(|i| body.iter().map(|p| p[i]).sum::<f64>() / body.len() as f64);
    let out = |p: [f64; 3]| (0..3).map(|i| (p[i] - middle[i]).powi(2)).sum::<f64>().sqrt();
    for e in d.plan().entries.iter().filter(|e| class(e) == "defencemodule") {
        let (l, h) = entry_bounds(d.plan(), c, e.index).unwrap();
        let at = out(std::array::from_fn(|i| (l[i] + h[i]) / 2.0));
        let farther = body.iter().filter(|p| out(**p) > at).count();
        eprintln!("defence #{} {at:.0} m from the body's middle, {farther} of {} body modules farther", e.index, body.len());
        assert!(farther * 2 < body.len(), "#{}: in the outer half of the body", e.index);
    }
}
