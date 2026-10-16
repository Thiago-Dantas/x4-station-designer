//! The visual builder (D-6): a Tauri window showing the Three.js UI from `ui/`.
//! Its commands are thin wrappers over `x4sd_core::design`, like the MCP tools;
//! every edit returns the whole scene for the UI to redraw. Agents reach the
//! same design through the live link (`link.rs`, D-7); their changes are pushed
//! to the UI as `scene-changed` events.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};
use tauri::ipc::Response;
use tauri::{AppHandle, Emitter, Manager, State};
use x4sd_core::catalog::Catalog;
use x4sd_core::design::Design;
use x4sd_core::equipment::KINDS;
use x4sd_core::geometry::{fit, Quat, Transform};
use x4sd_core::import::{file_name, new_plan_id, plan_files, renew_id_if_renamed, write_import_file};
use x4sd_core::plan::{parse_plans, write_plans, Plan};

use crate::link;
use crate::ops::{self, Ctx, ModuleNames, Op};

struct App {
    game_dir: Option<PathBuf>,
    profile_dir: Option<PathBuf>,
    catalog: Option<Arc<Catalog>>,
    info: ModuleNames,
    design: Design,
    /// The blueprint limit (D-16); the UI keeps it across restarts and sets it at start.
    blueprints: ops::Blueprints,
}

type Shared = Mutex<App>;
type CmdResult = Result<Value, String>;

pub fn run(game_dir: Option<PathBuf>, profile_dir: Option<PathBuf>) -> x4sd_core::Result<()> {
    #[cfg(windows)]
    detach_own_console();
    let app = App { game_dir, profile_dir, catalog: None, info: HashMap::new(), design: Design::new("New station"), blueprints: Default::default() };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(Mutex::new(app))
        .setup(|app| {
            let handle = app.handle().clone();
            if let Err(e) = link::serve(Arc::new(move |op| agent_op(&handle, op))) {
                eprintln!("live link for agents unavailable ({e}); is another builder open?");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            load_catalog,
            set_game_dir,
            module_shape,
            module_model,
            material_maps,
            texture,
            scene,
            new_design,
            list_game_plans,
            list_plans_in_file,
            delete_import_file,
            open_design,
            save_design,
            export_design,
            rename_design,
            add_module,
            attach_module,
            set_twist,
            remove_module,
            copy_sequence,
            set_plot,
            fit_plot,
            move_in_build_order,
            move_block_in_build_order,
            build_first,
            build_last,
            set_stage_end,
            reattach_module,
            detach_module,
            move_root,
            preview_gizmo,
            rotate_attached,
            straighten,
            build_cost,
            analyze_station,
            list_sectors,
            target_rates,
            list_wares,
            plan_production,
            auto_place,
            read_blueprints,
            set_blueprints,
            run_op,
            undo,
            redo,
        ])
        .run(tauri::generate_context!())
        .map_err(|e| x4sd_core::Error(format!("builder: {e}")))
}

/// Loads the game data (about half a second) and returns the module palette.
#[tauri::command]
async fn load_catalog(state: State<'_, Shared>) -> CmdResult {
    let (game, profile) = {
        let app = state.lock().unwrap();
        (app.game_dir.clone(), app.profile_dir.clone())
    };
    let game = game.ok_or(GAME_NOT_FOUND)?;
    let loaded = tauri::async_runtime::spawn_blocking(move || -> x4sd_core::Result<(Catalog, Vec<_>)> {
        let catalog = Catalog::load(&game, profile.as_deref())?;
        let modules = catalog.station_modules()?;
        Ok((catalog, modules))
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    let (catalog, modules) = loaded;
    let mut app = state.lock().unwrap();
    app.info = modules.iter().map(|m| (m.macro_name.to_ascii_lowercase(), (m.name.clone(), m.class.clone()))).collect();
    app.catalog = Some(Arc::new(catalog));
    let palette: Vec<Value> = modules
        .iter()
        .map(|m| json!({"macro": m.macro_name, "class": m.class, "race": m.race, "source": m.source, "name": m.name}))
        .collect();
    Ok(json!({"modules": palette, "profile": app.profile_dir, "game": app.game_dir}))
}

/// The UI offers to pick the folder when `load_catalog` fails with this.
const GAME_NOT_FOUND: &str = "X4 install not found";

/// The X4 folder the player picked: checked, remembered for later starts, used from now on.
#[tauri::command]
fn set_game_dir(state: State<'_, Shared>, dir: PathBuf) -> CmdResult {
    x4sd_core::game::save_game_dir(&dir).map_err(|e| e.to_string())?;
    state.lock().unwrap().game_dir = Some(dir);
    Ok(Value::Null)
}

/// Snaps (position, facing, rotation) and boxes of a module, in its own frame.
#[tauri::command]
fn module_shape(state: State<'_, Shared>, macro_name: String) -> CmdResult {
    let app = state.lock().unwrap();
    let module = catalog(&app)?.module(&macro_name).map_err(|e| e.to_string())?;
    let snaps: Vec<Value> = module.snaps.iter().map(|s| snap_json(&s.name, &s.transform)).collect();
    let boxes: Vec<Value> = module
        .shape
        .iter()
        .map(|b| json!({"position": b.transform.position, "rotation": quat_json(b.transform.rotation), "half": b.half_extents}))
        .collect();
    Ok(json!({"macro": module.macro_name, "class": module.class, "snaps": snaps, "boxes": boxes}))
}

/// A module's real model as raw little-endian bytes, game frame:
/// u32 vertex count, u32 index count, u32 group count; per group u32 first
/// index, u32 index count, u32 name length, material name (padded to 4 bytes);
/// then f32 positions (x, y, z), f32 normals (x, y, z), f32 uvs (u, v), u32 indices.
/// `quality`: "fast" (LOD 3, hull only), "normal" (LOD 2, large and medium
/// details) or "detailed" (LOD 1, all details).
#[tauri::command]
async fn module_model(state: State<'_, Shared>, macro_name: String, quality: String) -> Result<Response, String> {
    let catalog = catalog(&state.lock().unwrap())?;
    let (lod, details) = match quality.as_str() {
        "fast" => (3, 0),
        "detailed" => (1, 4),
        _ => (2, 2),
    };
    let model = tauri::async_runtime::spawn_blocking(move || catalog.module_model(&macro_name, lod, details))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let mut bytes = Vec::with_capacity(12 + model.positions.len() * 32 + model.indices.len() * 4);
    for n in [model.positions.len(), model.indices.len(), model.groups.len()] {
        bytes.extend((n as u32).to_le_bytes());
    }
    for (name, first, count) in &model.groups {
        bytes.extend(first.to_le_bytes());
        bytes.extend(count.to_le_bytes());
        bytes.extend((name.len() as u32).to_le_bytes());
        bytes.extend(name.as_bytes());
        bytes.resize(bytes.len().next_multiple_of(4), 0);
    }
    for v in model.positions.iter().chain(&model.normals) {
        v.iter().for_each(|f| bytes.extend(f.to_le_bytes()));
    }
    for uv in &model.uvs {
        uv.iter().for_each(|f| bytes.extend(f.to_le_bytes()));
    }
    model.indices.iter().for_each(|i| bytes.extend(i.to_le_bytes()));
    Ok(Response::new(bytes))
}

/// Texture maps of a mesh material: `{"diffuse": path | null, "normal": path | null}`.
#[tauri::command]
fn material_maps(state: State<'_, Shared>, name: String) -> CmdResult {
    let maps = catalog(&state.lock().unwrap())?.material(&name).unwrap_or_default();
    Ok(json!({"diffuse": maps.diffuse, "normal": maps.normal}))
}

/// A texture as raw bytes: u32 format (1 BC1, 2 BC2, 3 BC3, 4 BC4, 5 BC5, 7 BC7),
/// u32 mip count, then per mip u32 width, u32 height, u32 byte length, blocks.
/// Mips larger than 1024 pixels are dropped.
#[tauri::command]
async fn texture(state: State<'_, Shared>, path: String) -> Result<Response, String> {
    use x4sd_core::texture::Format;
    let catalog = catalog(&state.lock().unwrap())?;
    let t = tauri::async_runtime::spawn_blocking(move || catalog.texture(&path, 1024))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
    let code: u32 = match t.format {
        Format::Bc1 => 1,
        Format::Bc2 => 2,
        Format::Bc3 => 3,
        Format::Bc4 => 4,
        Format::Bc5 => 5,
        Format::Bc7 => 7,
    };
    let mut bytes = Vec::new();
    bytes.extend(code.to_le_bytes());
    bytes.extend((t.mips.len() as u32).to_le_bytes());
    for (w, h, data) in &t.mips {
        for n in [*w, *h, data.len() as u32] {
            bytes.extend(n.to_le_bytes());
        }
        bytes.extend(data);
    }
    Ok(Response::new(bytes))
}

#[tauri::command]
fn scene(state: State<'_, Shared>) -> CmdResult {
    scene_json(&state.lock().unwrap())
}

#[tauri::command]
fn new_design(state: State<'_, Shared>, name: String) -> CmdResult {
    let mut app = state.lock().unwrap();
    app.design = Design::new(&name);
    scene_json(&app)
}

/// Plans the player has: saved ones (`constructionplans.xml`) and import files.
#[tauri::command]
fn list_game_plans(state: State<'_, Shared>) -> CmdResult {
    let app = state.lock().unwrap();
    let profile = app.profile_dir.as_ref().ok_or("player profile not found")?;
    let mut out = Vec::new();
    for file in &plan_files(profile) {
        let source = if file.ends_with("constructionplans.xml") { "saved" } else { "import folder" };
        for p in read_plans(file)? {
            out.push(json!({"file": file, "source": source, "id": p.id, "name": p.name, "modules": p.entries.len()}));
        }
    }
    Ok(json!(out))
}

/// Deletes an import file: an `.xml` directly in `<profile>/constructionplan/`, nothing else.
#[tauri::command]
fn delete_import_file(state: State<'_, Shared>, file: String) -> CmdResult {
    let app = state.lock().unwrap();
    let profile = app.profile_dir.as_ref().ok_or("player profile not found")?;
    let folder = profile.join("constructionplan").canonicalize().map_err(|e| format!("import folder: {e}"))?;
    let path = Path::new(&file).canonicalize().map_err(|e| format!("{file}: {e}"))?;
    if path.parent() != Some(folder.as_path()) || path.extension().is_none_or(|x| x != "xml") {
        return Err(format!("{file} is not an import file"));
    }
    std::fs::remove_file(&path).map_err(|e| format!("{file}: {e}"))?;
    Ok(json!({"deleted": file}))
}

#[tauri::command]
fn list_plans_in_file(file: String) -> CmdResult {
    let plans = read_plans(Path::new(&file))?;
    Ok(json!(plans.iter().map(|p| json!({"file": file, "id": p.id, "name": p.name, "modules": p.entries.len()})).collect::<Vec<_>>()))
}

#[tauri::command]
fn open_design(state: State<'_, Shared>, file: String, id: String) -> CmdResult {
    let plan = read_plans(Path::new(&file))?.into_iter().find(|p| p.id == id).ok_or(format!("no plan {id} in {file}"))?;
    let mut app = state.lock().unwrap();
    app.design = Design::from_plan(plan);
    // The plan file has no plot: start with the smallest that holds the station.
    if let Some(c) = app.catalog.clone() {
        let _ = app.design.fit_plot(&c, false);
    }
    scene_json(&app)
}

/// Writes the design to a plan file. A renamed plan becomes a new plan
/// (`new_plan`: it got a new id, see `renew_id_if_renamed`).
#[tauri::command]
fn save_design(state: State<'_, Shared>, file: String) -> CmdResult {
    let mut app = state.lock().unwrap();
    let App { design, profile_dir, .. } = &mut *app;
    let new_plan = profile_dir.as_deref().is_some_and(|p| renew_id_if_renamed(p, design));
    std::fs::write(&file, write_plans(std::slice::from_ref(design.plan()))).map_err(|e| format!("{file}: {e}"))?;
    design.mark_saved();
    Ok(json!({"file": file, "new_plan": new_plan}))
}

/// Writes the design into `<profile>/constructionplan/` for in-game Import.
/// Fails with a message starting "exists:" when the file is there and `overwrite` is false.
/// A renamed plan becomes a new plan, as in `save_design`.
#[tauri::command]
fn export_design(state: State<'_, Shared>, new_id: bool, overwrite: bool) -> CmdResult {
    let mut app = state.lock().unwrap();
    let App { design, profile_dir, .. } = &mut *app;
    let profile = profile_dir.as_deref().ok_or("player profile not found")?;
    let path = profile.join("constructionplan").join(file_name(&design.plan().name));
    if path.exists() && !overwrite {
        return Err(format!("exists: {}", path.display()));
    }
    let new_plan = renew_id_if_renamed(profile, design);
    let mut plan = design.plan().clone();
    if new_id {
        plan.id = new_plan_id();
    }
    let path = write_import_file(profile, &plan, true).map_err(|e| e.to_string())?;
    design.mark_saved();
    Ok(json!({"file": path, "id": plan.id, "new_plan": new_plan}))
}

#[tauri::command]
fn rename_design(state: State<'_, Shared>, name: String) -> CmdResult {
    edit(&state, |d, _| {
        d.rename(&name);
        Ok(())
    })
}

#[tauri::command]
fn add_module(state: State<'_, Shared>, macro_name: String, position: [f64; 3]) -> CmdResult {
    edit(&state, |d, c| d.add_root(c, &macro_name, Transform { position, rotation: Quat::IDENTITY }).map(|_| ()))
}

#[tauri::command]
fn attach_module(state: State<'_, Shared>, macro_name: String, snap: String, to: u32, to_snap: String, twist: f64) -> CmdResult {
    edit(&state, |d, c| d.attach(c, &macro_name, &snap, to, &to_snap, twist).map(|_| ()))
}

#[tauri::command]
fn set_twist(state: State<'_, Shared>, index: u32, twist: f64) -> CmdResult {
    edit(&state, |d, c| d.set_twist(c, index, twist))
}

/// Removes a module alone (what was attached to it stays, unattached) or, with
/// `sequence`, with everything attached to it.
#[tauri::command]
fn remove_module(state: State<'_, Shared>, index: u32, sequence: bool) -> CmdResult {
    edit(&state, |d, c| if sequence { d.remove(c, index).map(|_| ()) } else { d.remove_single(c, index) })
}

/// Copies a module and everything attached to it onto a free snap; `focus` is the copy.
#[tauri::command]
fn copy_sequence(state: State<'_, Shared>, index: u32, snap: String, to: u32, to_snap: String, twist: f64) -> CmdResult {
    let catalog = catalog(&state.lock().unwrap())?;
    reorder(&state, |d| d.copy_sequence(&catalog, index, &snap, to, &to_snap, twist))
}

/// Plot reach per side in whole km: `pos` = +X, +Y, +Z, `neg` = −X, −Y, −Z.
#[tauri::command]
fn set_plot(state: State<'_, Shared>, pos: [u32; 3], neg: [u32; 3]) -> CmdResult {
    edit(&state, |d, _| d.set_plot(Some(x4sd_core::plot::Plot { pos, neg })))
}

#[tauri::command]
fn fit_plot(state: State<'_, Shared>, grow_only: bool) -> CmdResult {
    edit(&state, |d, c| d.fit_plot(c, grow_only).map(|_| ()))
}

// Reorders renumber the modules: the scene's `focus` is the moved module's new index.
#[tauri::command]
fn move_in_build_order(state: State<'_, Shared>, index: u32, position: usize) -> CmdResult {
    reorder(&state, |d| d.move_in_build_order(index, position))
}

/// Moves `indices` together so `anchor` lands at `position`. `focus` is `anchor`'s new index.
#[tauri::command]
fn move_block_in_build_order(state: State<'_, Shared>, indices: Vec<u32>, anchor: u32, position: usize) -> CmdResult {
    reorder(&state, |d| d.move_block_in_build_order(&indices, anchor, position))
}

#[tauri::command]
fn build_first(state: State<'_, Shared>, index: u32) -> CmdResult {
    reorder(&state, |d| d.build_first(index))
}

#[tauri::command]
fn build_last(state: State<'_, Shared>, index: u32) -> CmdResult {
    reorder(&state, |d| d.build_last(index))
}

/// Build materials for module `index` (with its equipment) or, without, the whole station.
#[tauri::command]
fn build_cost(state: State<'_, Shared>, index: Option<u32>, method: String) -> CmdResult {
    let app = state.lock().unwrap();
    let catalog = catalog(&app)?;
    ops::build_cost(&catalog, &app.design, index, &method)
}

/// The whole station's production, workforce, storage and docks at `sunlight` percent.
#[tauri::command]
fn analyze_station(state: State<'_, Shared>, sunlight: f64) -> CmdResult {
    let app = state.lock().unwrap();
    let catalog = catalog(&app)?;
    ops::analyze_station(&catalog, &app.design, sunlight)
}

/// Sectors with their sunlight in percent, by name: [{macro, name, sunlight}].
#[tauri::command]
fn list_sectors(state: State<'_, Shared>) -> CmdResult {
    let catalog = catalog(&state.lock().unwrap())?;
    Ok(json!(catalog.sectors().iter().map(|s| json!({"macro": s.macro_name, "name": s.name, "sunlight": s.sunlight * 100.0})).collect::<Vec<_>>()))
}

/// Wares some buildable module makes.
#[tauri::command]
fn list_wares(state: State<'_, Shared>) -> CmdResult {
    let catalog = catalog(&state.lock().unwrap())?;
    ops::list_wares(&catalog)
}

/// The modules to add for the planner's targets (see `ops::PlanArgs`), for the
/// open design or, with `baseline`, for a station with those modules.
#[tauri::command]
fn plan_production(state: State<'_, Shared>, args: ops::PlanArgs, baseline: Option<Vec<ops::ModuleCount>>) -> CmdResult {
    let app = state.lock().unwrap();
    let catalog = catalog(&app)?;
    ops::plan_production(&catalog, &app.design, args, baseline, &app.blueprints)
}

/// What the planner's target modules make per hour, before planning (`ops::target_rates`).
#[tauri::command]
fn target_rates(state: State<'_, Shared>, args: ops::PlanArgs) -> CmdResult {
    let app = state.lock().unwrap();
    let catalog = catalog(&app)?;
    ops::target_rates(&catalog, args, &app.blueprints)
}

/// Runs a shared operation (`ops::Op`, as MCP agents do) on the open design: its
/// `result`, plus the `scene` when the design may have changed. Off the main
/// thread: some (optimizing a station) take seconds.
#[tauri::command]
async fn run_op(handle: AppHandle, op: Op) -> CmdResult {
    tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<Shared>();
        let mut app = state.lock().unwrap();
        let catalog = catalog(&app)?;
        let changes = op.changes_design();
        let App { info, design, profile_dir, blueprints, .. } = &mut *app;
        let result = ops::execute(op, &mut Ctx { catalog: &catalog, names: info, design, profile_dir: profile_dir.as_deref(), blueprints })?;
        let mut out = json!({"result": result});
        if changes {
            out["scene"] = scene_json(&app)?;
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Places modules (macro, count) with auto-placement (`ops::auto_place`, plot may
/// grow); the scene plus `placement` (what was placed, added and left over). Runs
/// off the main thread: a big station takes a few seconds.
#[tauri::command]
async fn auto_place(handle: AppHandle, modules: Vec<ops::ModuleCount>, race: Option<String>, fewer_connectors: bool) -> CmdResult {
    tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<Shared>();
        let mut app = state.lock().unwrap();
        let catalog = catalog(&app)?;
        let args = ops::AutoPlaceArgs { modules: Some(modules), from_plan: None, grow_plot: Some(true), race, fewer_connectors: Some(fewer_connectors), break_up: Some(true) };
        let App { design, blueprints, .. } = &mut *app;
        let placement = ops::auto_place(&catalog, design, args, blueprints)?;
        let mut scene = scene_json(&app)?;
        scene["placement"] = placement;
        Ok(scene)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Reads the player's blueprints from a save and turns the limit on; the new
/// `ops::Blueprints`. The save is read without holding the design.
#[tauri::command]
async fn read_blueprints(handle: AppHandle, file: String) -> CmdResult {
    tauri::async_runtime::spawn_blocking(move || {
        let state = handle.state::<Shared>();
        let catalog = catalog(&state.lock().unwrap())?;
        let blueprints = ops::Blueprints::read(&catalog, Path::new(&file))?;
        state.lock().unwrap().blueprints = blueprints.clone();
        Ok(json!(blueprints))
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Sets the blueprint limit: on/off, or the one the UI remembered at start.
#[tauri::command]
fn set_blueprints(state: State<'_, Shared>, blueprints: ops::Blueprints) -> CmdResult {
    let mut app = state.lock().unwrap();
    app.blueprints = blueprints;
    scene_json(&app)
}

/// Moves a placed module (and everything on it) onto another free snap; `focus` is its new index.
#[tauri::command]
fn reattach_module(state: State<'_, Shared>, index: u32, snap: String, to: u32, to_snap: String, twist: f64) -> CmdResult {
    let catalog = catalog(&state.lock().unwrap())?;
    reorder(&state, |d| d.reattach(&catalog, index, &snap, to, &to_snap, twist))
}

#[tauri::command]
fn detach_module(state: State<'_, Shared>, index: u32) -> CmdResult {
    edit(&state, |d, _| d.detach(index))
}

/// Moves a free-standing module (and everything on it) to `position`, turned to
/// `rotation` (w, x, y, z) if given, else keeping its rotation.
#[tauri::command]
fn move_root(state: State<'_, Shared>, index: u32, position: [f64; 3], rotation: Option<[f64; 4]>) -> CmdResult {
    edit(&state, |d, _| {
        let rotation = match rotation {
            Some([w, x, y, z]) => Quat { w, x, y, z },
            None => d.world(index)?.rotation,
        };
        d.move_root(index, Transform { position, rotation })
    })
}

/// Checks while a gizmo drag is in progress: the issues and plot status as if
/// module `index` were dropped there now (`move_root` for a free-standing one,
/// `rotate_attached` for an attached one), on a copy; the design doesn't change.
#[tauri::command]
fn preview_gizmo(state: State<'_, Shared>, index: u32, position: [f64; 3], rotation: [f64; 4]) -> CmdResult {
    let app = state.lock().unwrap();
    let catalog = catalog(&app)?;
    let mut d = Design::from_plan(app.design.plan().clone());
    d.set_plot(app.design.plot()).map_err(|e| e.to_string())?;
    let [w, x, y, z] = rotation;
    let rotation = Quat { w, x, y, z };
    let attached = d.plan().entries.iter().find(|e| e.index == index).ok_or_else(|| format!("no entry {index}"))?.predecessor.is_some();
    let moved = if attached {
        d.rotate_attached(&catalog, index, rotation)
    } else {
        d.move_root(index, Transform { position, rotation })
    };
    moved.map_err(|e| e.to_string())?;
    Ok(json!({"issues": issues_json(&d, &catalog, &app), "plot": ops::plot_status(&d, &catalog)}))
}

/// Turns an attached module (and everything on it) about its joint to `rotation`
/// (w, x, y, z), staying attached; turns off the snap axis tilt it.
#[tauri::command]
fn rotate_attached(state: State<'_, Shared>, index: u32, rotation: [f64; 4]) -> CmdResult {
    let [w, x, y, z] = rotation;
    edit(&state, |d, c| d.rotate_attached(c, index, Quat { w, x, y, z }))
}

/// Takes the tilt off an attached module, keeping its twist.
#[tauri::command]
fn straighten(state: State<'_, Shared>, index: u32) -> CmdResult {
    edit(&state, |d, c| d.straighten(c, index))
}

fn reorder(state: &State<'_, Shared>, op: impl FnOnce(&mut Design) -> x4sd_core::Result<u32>) -> CmdResult {
    let mut app = state.lock().unwrap();
    let focus = op(&mut app.design).map_err(|e| e.to_string())?;
    let mut scene = scene_json(&app)?;
    scene["focus"] = json!(focus);
    Ok(scene)
}

#[tauri::command]
fn set_stage_end(state: State<'_, Shared>, index: u32, end: bool) -> CmdResult {
    edit(&state, |d, _| d.set_stage_end(index, end))
}

#[tauri::command]
fn undo(state: State<'_, Shared>) -> CmdResult {
    edit(&state, |d, _| {
        d.undo();
        Ok(())
    })
}

#[tauri::command]
fn redo(state: State<'_, Shared>) -> CmdResult {
    edit(&state, |d, _| {
        d.redo();
        Ok(())
    })
}

/// An agent's operation from the live link, run on the open design.
fn agent_op(handle: &AppHandle, op: Op) -> Result<Value, String> {
    let state = handle.state::<Shared>();
    let mut app = state.lock().unwrap();
    let catalog = catalog(&app)?;
    let (changes, replaced, name, blueprints_changed) = (op.changes_design(), op.replaces_design(), op.name(), op.changes_blueprints());
    let App { info, design, profile_dir, blueprints, .. } = &mut *app;
    let result = ops::execute(op, &mut Ctx { catalog: &catalog, names: info, design, profile_dir: profile_dir.as_deref(), blueprints });
    if blueprints_changed && result.is_ok() {
        let _ = handle.emit("blueprints-changed", &app.blueprints);
    }
    if changes && result.is_ok() {
        let scene = scene_json(&app)?;
        let _ = handle.emit("scene-changed", json!({"scene": scene, "op": name, "replaced": replaced}));
    }
    result
}

fn catalog(app: &App) -> Result<Arc<Catalog>, String> {
    app.catalog.clone().ok_or_else(|| "game data is still loading".to_owned())
}

fn edit(state: &State<'_, Shared>, op: impl FnOnce(&mut Design, &Catalog) -> x4sd_core::Result<()>) -> CmdResult {
    let mut app = state.lock().unwrap();
    let catalog = catalog(&app)?;
    op(&mut app.design, &catalog).map_err(|e| e.to_string())?;
    scene_json(&app)
}

fn read_plans(file: &Path) -> Result<Vec<Plan>, String> {
    let xml = std::fs::read_to_string(file).map_err(|e| format!("{}: {e}", file.display()))?;
    parse_plans(&xml).map_err(|e| format!("{}: {e}", file.display()))
}

/// Everything the UI draws: modules with world transforms, links, twists and
/// free snaps (station frame), plus validation issues.
fn scene_json(app: &App) -> CmdResult {
    let design = &app.design;
    let plan = design.plan();
    let catalog = app.catalog.clone();
    let mut entries = Vec::new();
    for e in &plan.entries {
        let world = design.world(e.index).map_err(|e| e.to_string())?;
        let info = app.info.get(&e.macro_name.to_ascii_lowercase()).cloned().or_else(|| {
            // Not in the palette (a mod's module without build sets): name and class from the macro.
            let c = catalog.as_ref()?;
            Some((c.display_name(&e.macro_name)?, c.module(&e.macro_name).ok()?.class.clone()))
        });
        let info = info.as_ref();
        let attached = e.predecessor.as_ref().map(|p| {
            // Twist, tilt off the snap axis (degrees) and the joint (where the snaps meet).
            let joint = catalog.as_ref().and_then(|c| {
                let pred = plan.entries.iter().find(|x| x.index == p.index)?;
                let ps = c.module(&pred.macro_name).ok()?.snap(&p.connection)?.transform;
                let cs = c.module(&e.macro_name).ok()?.snap(e.connection.as_deref()?)?.transform;
                let at = design.world(p.index).ok()?;
                Some((fit(&at, &ps, &world, &cs), at.then(&ps).position))
            });
            json!({
                "to": p.index,
                "to_snap": p.connection,
                "snap": e.connection,
                // 359.99... rounds to 360: show it as 0.
                "twist": joint.map(|(f, _)| f.twist.round().rem_euclid(360.0)),
                "tilt": joint.map(|(f, _)| (f.misalignment * 10.0).round() / 10.0),
                "joint": joint.map(|(_, p)| p),
            })
        });
        let free: Vec<Value> = match &catalog {
            Some(c) => design.free_snaps(c, e.index).map(|v| v.iter().map(|s| snap_json(&s.name, &s.world)).collect()).unwrap_or_default(),
            None => Vec::new(),
        };
        // Turret and shield groups (one per group and kind): [equipped, total].
        let groups = catalog.as_ref().and_then(|c| c.module(&e.macro_name).ok()).map_or(0, |m| {
            m.equipment_groups().iter().map(|g| KINDS.iter().filter(|k| m.group_slots(g, k).next().is_some()).count()).sum::<usize>()
        });
        entries.push(json!({
            "index": e.index,
            "macro": e.macro_name,
            "name": info.map(|i| i.0.clone()),
            "class": info.map(|i| i.1.clone()),
            "position": world.position,
            "rotation": quat_json(world.rotation),
            "attached": attached,
            "stage_end": e.bookmark.is_some(),
            "free_snaps": free,
            "equipment": [e.upgrades.len().min(groups), groups],
        }));
    }
    let issues = catalog.as_ref().map(|c| issues_json(design, c, app)).unwrap_or_default();
    let plot = catalog.as_ref().map(|c| ops::plot_status(design, c)).unwrap_or(Value::Null);
    Ok(json!({
        "id": plan.id,
        "name": plan.name,
        "patches": plan.patches.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
        "can_undo": design.can_undo(),
        "history": design.history_len(),
        "dirty": design.is_dirty(),
        "can_redo": design.can_redo(),
        "entries": entries,
        "issues": issues,
        "plot": plot,
    }))
}

/// The design's validation issues (`ops::issue_list`), with the blueprint limit of `app`.
fn issues_json(design: &Design, catalog: &Catalog, app: &App) -> Vec<Value> {
    ops::issue_list(design, catalog, &app.blueprints, &app.info).iter().map(|i| json!({"index": i.index(), "text": i.to_string(), "note": i.is_note()})).collect()
}

/// `x4sd.exe` is a console program (the CLI and MCP need stdio). Started by
/// double-click, Windows gives it a console of its own; drop that so only the
/// builder window remains. Started from a terminal, the terminal is kept.
#[cfg(windows)]
fn detach_own_console() {
    extern "system" {
        fn GetConsoleProcessList(list: *mut u32, count: u32) -> u32;
        fn FreeConsole() -> i32;
    }
    let mut processes = [0u32; 2];
    // SAFETY: plain kernel32 calls; the buffer length matches `count`.
    unsafe {
        if GetConsoleProcessList(processes.as_mut_ptr(), processes.len() as u32) == 1 {
            FreeConsole();
        }
    }
}

fn quat_json(q: Quat) -> [f64; 4] {
    [q.w, q.x, q.y, q.z]
}

/// A snap as position + facing direction (its local +Z), in the frame of `t`.
fn snap_json(name: &str, t: &Transform) -> Value {
    json!({"name": name, "position": t.position, "facing": t.rotation.rotate([0.0, 0.0, 1.0]), "rotation": quat_json(t.rotation)})
}
