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
use x4sd_core::validate::validate;

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

/// A module's real model as raw little-endian bytes: u32 vertex count, u32 index
/// count, f32 positions (x, y, z), f32 normals (x, y, z), u32 indices. Game frame.
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
    let mut bytes = Vec::with_capacity(8 + model.positions.len() * 24 + model.indices.len() * 4);
    bytes.extend((model.positions.len() as u32).to_le_bytes());
    bytes.extend((model.indices.len() as u32).to_le_bytes());
    for v in model.positions.iter().chain(&model.normals) {
        v.iter().for_each(|f| bytes.extend(f.to_le_bytes()));
    }
    model.indices.iter().for_each(|i| bytes.extend(i.to_le_bytes()));
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
    scene_json(&app)
}

#[tauri::command]
fn save_design(state: State<'_, Shared>, file: String) -> CmdResult {
    let app = state.lock().unwrap();
    std::fs::write(&file, write_plans(std::slice::from_ref(app.design.plan()))).map_err(|e| format!("{file}: {e}"))?;
    Ok(json!({"file": file}))
}

/// Writes the design into `<profile>/constructionplan/` for in-game Import.
/// Fails with a message starting "exists:" when the file is there and `overwrite` is false.
#[tauri::command]
fn export_design(state: State<'_, Shared>, new_id: bool, overwrite: bool) -> CmdResult {
    let app = state.lock().unwrap();
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
                Some(fit(&design.world(p.index).ok()?, &ps, &world, &cs).twist.round())
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
            "free_snaps": free,
        }));
    }
    let issues: Vec<Value> = match &catalog {
        Some(c) => validate(plan, c).iter().map(|i| json!({"index": i.index(), "text": i.to_string()})).collect(),
        None => Vec::new(),
    };
    Ok(json!({
        "id": plan.id,
        "name": plan.name,
        "patches": plan.patches.iter().map(|p| p.name.clone()).collect::<Vec<_>>(),
        "can_undo": design.can_undo(),
        "can_redo": design.can_redo(),
        "entries": entries,
        "issues": issues,
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
