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
use x4sd_core::geometry::{fit, Quat, Transform};
use x4sd_core::import::{new_plan_id, write_import_file};
use x4sd_core::plan::{parse_plans, write_plans, Plan};

use crate::link;
use crate::ops::{self, Ctx, ModuleNames, Op};

struct App {
    game_dir: Option<PathBuf>,
    profile_dir: Option<PathBuf>,
    catalog: Option<Arc<Catalog>>,
    info: ModuleNames,
    design: Design,
}

type Shared = Mutex<App>;
type CmdResult = Result<Value, String>;

pub fn run(game_dir: Option<PathBuf>, profile_dir: Option<PathBuf>) -> x4sd_core::Result<()> {
    #[cfg(windows)]
    detach_own_console();
    let app = App { game_dir, profile_dir, catalog: None, info: HashMap::new(), design: Design::new("New station") };
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
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
            module_shape,
            module_model,
            material_maps,
            texture,
            scene,
            new_design,
            list_game_plans,
            list_plans_in_file,
            open_design,
            save_design,
            export_design,
            rename_design,
            add_module,
            attach_module,
            set_twist,
            remove_module,
            set_plot,
            fit_plot,
            move_in_build_order,
            build_first,
            build_last,
            set_stage_end,
            reattach_module,
            detach_module,
            move_root,
            build_cost,
            analyze_station,
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
    let game = game.ok_or("X4 install not found; start x4sd with --game-dir or set X4_GAME_DIR")?;
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

/// Snaps (position, facing) and boxes of a module, in its own frame.
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
    let mut files = vec![profile.join("constructionplans.xml")];
    if let Ok(dir) = std::fs::read_dir(profile.join("constructionplan")) {
        files.extend(dir.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "xml")));
    }
    let mut out = Vec::new();
    for file in files.iter().filter(|f| f.exists()) {
        let source = if file.ends_with("constructionplans.xml") { "saved" } else { "import folder" };
        for p in read_plans(file)? {
            out.push(json!({"file": file, "source": source, "id": p.id, "name": p.name, "modules": p.entries.len()}));
        }
    }
    Ok(json!(out))
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

#[tauri::command]
fn save_design(state: State<'_, Shared>, file: String) -> CmdResult {
    let mut app = state.lock().unwrap();
    std::fs::write(&file, write_plans(std::slice::from_ref(app.design.plan()))).map_err(|e| format!("{file}: {e}"))?;
    app.design.mark_saved();
    Ok(json!({"file": file}))
}

/// Writes the design into `<profile>/constructionplan/` for in-game Import.
/// Fails with a message starting "exists:" when the file is there and `overwrite` is false.
#[tauri::command]
fn export_design(state: State<'_, Shared>, new_id: bool, overwrite: bool) -> CmdResult {
    let mut app = state.lock().unwrap();
    let profile = app.profile_dir.as_ref().ok_or("player profile not found")?;
    let mut plan = app.design.plan().clone();
    if new_id {
        plan.id = new_plan_id();
    }
    let path = profile.join("constructionplan").join(x4sd_core::import::file_name(&plan.name));
    if path.exists() && !overwrite {
        return Err(format!("exists: {}", path.display()));
    }
    let path = write_import_file(profile, &plan, true).map_err(|e| e.to_string())?;
    app.design.mark_saved();
    Ok(json!({"file": path, "id": plan.id}))
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

#[tauri::command]
fn remove_module(state: State<'_, Shared>, index: u32) -> CmdResult {
    edit(&state, |d, c| d.remove(c, index).map(|_| ()))
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

/// Moves a free-standing module (and everything on it) to `position`, keeping its rotation.
#[tauri::command]
fn move_root(state: State<'_, Shared>, index: u32, position: [f64; 3]) -> CmdResult {
    edit(&state, |d, _| {
        let rotation = d.world(index)?.rotation;
        d.move_root(index, Transform { position, rotation })
    })
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
    let (changes, replaced, name) = (op.changes_design(), op.replaces_design(), op.name());
    let App { info, design, profile_dir, .. } = &mut *app;
    let result = ops::execute(op, &mut Ctx { catalog: &catalog, names: info, design, profile_dir: profile_dir.as_deref() });
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
        let info = app.info.get(&e.macro_name.to_ascii_lowercase());
        let attached = e.predecessor.as_ref().map(|p| {
            let twist = catalog.as_ref().and_then(|c| {
                let pred = plan.entries.iter().find(|x| x.index == p.index)?;
                let ps = c.module(&pred.macro_name).ok()?.snap(&p.connection)?.transform;
                let cs = c.module(&e.macro_name).ok()?.snap(e.connection.as_deref()?)?.transform;
                // 359.99... rounds to 360: show it as 0.
                Some(fit(&design.world(p.index).ok()?, &ps, &world, &cs).twist.round().rem_euclid(360.0))
            });
            json!({"to": p.index, "to_snap": p.connection, "snap": e.connection, "twist": twist})
        });
        let free: Vec<Value> = match &catalog {
            Some(c) => design.free_snaps(c, e.index).map(|v| v.iter().map(|s| snap_json(&s.name, &s.world)).collect()).unwrap_or_default(),
            None => Vec::new(),
        };
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
        }));
    }
    let issues: Vec<Value> = match &catalog {
        Some(c) => {
            let mut all = x4sd_core::validate::validate(plan, c);
            if let Some(plot) = design.plot() {
                all.extend(x4sd_core::validate::validate_plot(plan, c, &plot));
            }
            all.iter().map(|i| json!({"index": i.index(), "text": i.to_string()})).collect()
        }
        None => Vec::new(),
    };
    let plot = catalog.as_ref().map(|c| ops::plot_status(design, c)).unwrap_or(Value::Null);
    Ok(json!({
        "id": plan.id,
        "name": plan.name,
        "patches": plan.patches.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
        "can_undo": design.can_undo(),
        "dirty": design.is_dirty(),
        "can_redo": design.can_redo(),
        "entries": entries,
        "issues": issues,
        "plot": plot,
    }))
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
    json!({"name": name, "position": t.position, "facing": t.rotation.rotate([0.0, 0.0, 1.0])})
}
