//! `x4sd mcp`: MCP server over stdio for AI agents (D-8). Headless for now: it
//! edits its own open design; the live link to the builder comes later (D-7).
//!
//! Every tool goes through `x4sd_core::design`, the same operations the
//! builder and CLI use.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use serde::Deserialize;
use serde_json::{json, Value};
use x4sd_core::catalog::Catalog;
use x4sd_core::design::Design;
use x4sd_core::geometry::{Quat, Transform};
use x4sd_core::import::{new_plan_id, write_import_file};
use x4sd_core::plan::{parse_plans, write_plans};
use x4sd_core::validate::validate;

const INSTRUCTIONS: &str = "\
Designs X4: Foundations station construction plans and puts them into the game.
Workflow: list_modules to find module macros -> new_design (or open_design) -> add_module for the first \
module -> free_snaps to see where modules can attach -> attach_module (snap to snap; twist rotates around \
the snap axis, usually in 90 degree steps) -> validate_design -> export_design, then the player clicks \
Import in the game's station build menu.
Coordinates are metres in the station frame, Y up. Module indices are 1..n and are renumbered after \
remove_module. Every edit can be undone with undo.";

#[derive(Clone)]
pub struct Server {
    state: Arc<Mutex<State>>,
    tool_router: ToolRouter<Server>,
}

struct State {
    game_dir: Option<PathBuf>,
    profile_dir: Option<PathBuf>,
    catalog: Option<Arc<Catalog>>,
    names: HashMap<String, String>,
    design: Option<Design>,
}

type ToolResult = Result<String, String>;

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ListModulesArgs {
    /// Only this class: connectionmodule, production, storage, habitation, dockarea, pier, defencemodule, buildmodule, welfaremodule, processingmodule, ventureplatform, radar
    class: Option<String>,
    /// Only this race, e.g. argon, terran, teladi
    race: Option<String>,
    /// Text to look for in the display name or macro
    search: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MacroArgs {
    /// Module macro, e.g. struct_arg_base_02_macro
    r#macro: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NameArgs {
    name: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct FileArgs {
    /// Path of a plan file (constructionplans.xml, an exported plan, or one saved by save_design)
    file: String,
    /// Plan name or id, when the file has several plans
    plan: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct PlaceArgs {
    /// Module macro, e.g. struct_ter_cross_01_macro
    r#macro: String,
    /// Position [x, y, z] in metres; default [0, 0, 0]
    position: Option<[f64; 3]>,
    /// Degrees around Y (up)
    yaw: Option<f64>,
    /// Degrees, game convention
    pitch: Option<f64>,
    /// Degrees, game convention
    roll: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AttachArgs {
    /// Module macro to add
    r#macro: String,
    /// Snap of the new module to connect; default: its first snap
    snap: Option<String>,
    /// Index of the placed module to attach to
    to: u32,
    /// Free snap of that module (see free_snaps)
    to_snap: String,
    /// Rotation around the snap axis in degrees; default 0
    twist: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct IndexArgs {
    /// Module index in the design
    index: u32,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct TwistArgs {
    index: u32,
    /// Rotation around the snap axis in degrees
    twist: f64,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MoveArgs {
    /// Index of a module that is not attached to anything
    index: u32,
    position: [f64; 3],
    yaw: Option<f64>,
    pitch: Option<f64>,
    roll: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ExportArgs {
    /// Give the plan a fresh id so the import adds a new plan instead of replacing one with the same id
    new_id: Option<bool>,
    /// Replace an existing import file of the same name
    overwrite: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SaveArgs {
    /// Where to write the plan file
    file: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NoArgs {}

#[tool_router]
impl Server {
    pub fn new(game_dir: Option<PathBuf>, profile_dir: Option<PathBuf>) -> Server {
        let state = State { game_dir, profile_dir, catalog: None, names: HashMap::new(), design: None };
        Server { state: Arc::new(Mutex::new(state)), tool_router: Self::tool_router() }
    }

    #[tool(description = "List buildable station modules (base game, DLCs, enabled mods): macro, class, race, source, display name.")]
    fn list_modules(&self, Parameters(a): Parameters<ListModulesArgs>) -> ToolResult {
        let catalog = self.catalog()?;
        let search = a.search.map(|s| s.to_lowercase());
        let modules: Vec<Value> = catalog
            .station_modules()
            .map_err(|e| e.to_string())?
            .into_iter()
            .filter(|m| a.class.as_ref().is_none_or(|c| m.class.eq_ignore_ascii_case(c)))
            .filter(|m| a.race.as_ref().is_none_or(|r| m.race.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(r))))
            .filter(|m| search.as_ref().is_none_or(|s| m.name.to_lowercase().contains(s) || m.macro_name.to_lowercase().contains(s)))
            .map(|m| json!({"macro": m.macro_name, "class": m.class, "race": m.race, "source": m.source, "name": m.name}))
            .collect();
        Ok(json!(modules).to_string())
    }

    #[tool(description = "A module's snap connections: name, position and facing direction in the module's own frame.")]
    fn module_snaps(&self, Parameters(a): Parameters<MacroArgs>) -> ToolResult {
        let module = self.catalog()?.module(&a.r#macro).map_err(|e| e.to_string())?;
        let snaps: Vec<Value> = module.snaps.iter().map(|s| snap_json(&s.name, &s.transform)).collect();
        Ok(json!({"macro": module.macro_name, "class": module.class, "snaps": snaps}).to_string())
    }

    #[tool(description = "Start a new, empty design (replaces the open one).")]
    fn new_design(&self, Parameters(a): Parameters<NameArgs>) -> ToolResult {
        self.state.lock().unwrap().design = Some(Design::new(&a.name));
        self.describe_design(Parameters(NoArgs {}))
    }

    #[tool(description = "Open a plan from a plan file for editing (replaces the open design).")]
    fn open_design(&self, Parameters(a): Parameters<FileArgs>) -> ToolResult {
        let xml = std::fs::read_to_string(&a.file).map_err(|e| format!("{}: {e}", a.file))?;
        let mut plans = parse_plans(&xml).map_err(|e| e.to_string())?;
        if let Some(w) = &a.plan {
            plans.retain(|p| &p.id == w || &p.name == w);
        }
        if plans.len() != 1 {
            let names: Vec<_> = plans.iter().map(|p| p.name.as_str()).collect();
            return Err(format!("{} plans match; pass plan= one of {names:?}", plans.len()));
        }
        self.state.lock().unwrap().design = Some(Design::from_plan(plans.remove(0)));
        self.describe_design(Parameters(NoArgs {}))
    }

    #[tool(description = "The open design: id, name, DLC patches, and every module with index, macro, name, position, what it is attached to, and its free snaps.")]
    fn describe_design(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        let catalog = self.catalog()?;
        let state = self.state.lock().unwrap();
        let design = state.design.as_ref().ok_or("no open design; use new_design or open_design")?;
        let plan = design.plan();
        let modules: Vec<Value> = plan
            .entries
            .iter()
            .map(|e| {
                let p = design.world(e.index).map(|t| t.position).unwrap_or_default();
                let free: Vec<String> = design.free_snaps(&catalog, e.index).map(|v| v.into_iter().map(|s| s.name).collect()).unwrap_or_default();
                json!({
                    "index": e.index,
                    "macro": e.macro_name,
                    "name": state.names.get(&e.macro_name.to_ascii_lowercase()),
                    "position": round3(p),
                    "attached": e.predecessor.as_ref().map(|pr| json!({"to": pr.index, "to_snap": pr.connection, "snap": e.connection})),
                    "free_snaps": free,
                })
            })
            .collect();
        let patches: Vec<&str> = plan.patches.iter().map(|p| p.extension.as_str()).collect();
        Ok(json!({"id": plan.id, "name": plan.name, "patches": patches, "modules": modules}).to_string())
    }

    #[tool(description = "Rename the open design.")]
    fn rename_design(&self, Parameters(a): Parameters<NameArgs>) -> ToolResult {
        self.edit(|d, _| {
            d.rename(&a.name);
            Ok(json!({"name": a.name}))
        })
    }

    #[tool(description = "Place a module on its own (not attached), e.g. the first module of a station. Returns its index.")]
    fn add_module(&self, Parameters(a): Parameters<PlaceArgs>) -> ToolResult {
        let at = transform(a.position, a.yaw, a.pitch, a.roll);
        self.edit(|d, c| Ok(json!({"index": d.add_root(c, &a.r#macro, at)?})))
    }

    #[tool(description = "Add a module by connecting one of its snaps to a free snap of a placed module. Returns the new index.")]
    fn attach_module(&self, Parameters(a): Parameters<AttachArgs>) -> ToolResult {
        self.edit(|d, c| {
            let snap = match &a.snap {
                Some(s) => s.clone(),
                None => c.module(&a.r#macro)?.snaps.first().map(|s| s.name.clone()).ok_or_else(|| {
                    x4sd_core::Error(format!("{} has no snaps", a.r#macro))
                })?,
            };
            let index = d.attach(c, &a.r#macro, &snap, a.to, &a.to_snap, a.twist.unwrap_or(0.0))?;
            Ok(json!({"index": index, "snap": snap}))
        })
    }

    #[tool(description = "Rotate an attached module around its snap axis to an absolute twist (degrees). Everything attached to it moves along.")]
    fn set_twist(&self, Parameters(a): Parameters<TwistArgs>) -> ToolResult {
        self.edit(|d, c| {
            d.set_twist(c, a.index, a.twist)?;
            Ok(json!({"index": a.index, "twist": a.twist}))
        })
    }

    #[tool(description = "Move a module that is not attached to anything (and everything attached to it).")]
    fn move_module(&self, Parameters(a): Parameters<MoveArgs>) -> ToolResult {
        let at = transform(Some(a.position), a.yaw, a.pitch, a.roll);
        self.edit(|d, _| {
            d.move_root(a.index, at)?;
            Ok(json!({"index": a.index}))
        })
    }

    #[tool(description = "Remove a module and everything attached to it. The remaining modules are renumbered 1..n.")]
    fn remove_module(&self, Parameters(a): Parameters<IndexArgs>) -> ToolResult {
        self.edit(|d, c| Ok(json!({"removed": d.remove(c, a.index)?})))
    }

    #[tool(description = "Free snaps of a placed module, where new modules can attach: name, station-frame position and facing direction.")]
    fn free_snaps(&self, Parameters(a): Parameters<IndexArgs>) -> ToolResult {
        let catalog = self.catalog()?;
        let state = self.state.lock().unwrap();
        let design = state.design.as_ref().ok_or("no open design")?;
        let snaps = design.free_snaps(&catalog, a.index).map_err(|e| e.to_string())?;
        Ok(json!(snaps.iter().map(|s| snap_json(&s.name, &s.world)).collect::<Vec<_>>()).to_string())
    }

    #[tool(description = "Undo the last edit.")]
    fn undo(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        let mut state = self.state.lock().unwrap();
        let design = state.design.as_mut().ok_or("no open design")?;
        Ok(json!({"undone": design.undo()}).to_string())
    }

    #[tool(description = "Redo the last undone edit.")]
    fn redo(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        let mut state = self.state.lock().unwrap();
        let design = state.design.as_mut().ok_or("no open design")?;
        Ok(json!({"redone": design.redo()}).to_string())
    }

    #[tool(description = "Check the open design for problems the game does not catch (unknown modules, modules off their snaps, missing DLC patches). An empty list means it is fine.")]
    fn validate_design(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        let catalog = self.catalog()?;
        let state = self.state.lock().unwrap();
        let design = state.design.as_ref().ok_or("no open design")?;
        let issues: Vec<String> = validate(design.plan(), &catalog).iter().map(|i| i.to_string()).collect();
        Ok(json!({"issues": issues}).to_string())
    }

    #[tool(description = "Save the open design to a plan file (single-plan format, like the game's exports).")]
    fn save_design(&self, Parameters(a): Parameters<SaveArgs>) -> ToolResult {
        let state = self.state.lock().unwrap();
        let design = state.design.as_ref().ok_or("no open design")?;
        std::fs::write(&a.file, write_plans(std::slice::from_ref(design.plan()))).map_err(|e| format!("{}: {e}", a.file))?;
        Ok(json!({"saved": a.file}).to_string())
    }

    #[tool(description = "Put the open design into the game: writes it to the player profile's constructionplan folder. The player then clicks Import in the station build menu.")]
    fn export_design(&self, Parameters(a): Parameters<ExportArgs>) -> ToolResult {
        let state = self.state.lock().unwrap();
        let design = state.design.as_ref().ok_or("no open design")?;
        let profile = state.profile_dir.as_ref().ok_or("player profile not found")?;
        let mut plan = design.plan().clone();
        if a.new_id.unwrap_or(false) {
            plan.id = new_plan_id();
        }
        let path = write_import_file(profile, &plan, a.overwrite.unwrap_or(false)).map_err(|e| e.to_string())?;
        Ok(json!({"file": path, "id": plan.id, "next": "In game: station build menu > Import > select the plan"}).to_string())
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for Server {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("x4sd", env!("CARGO_PKG_VERSION")))
            .with_instructions(INSTRUCTIONS)
    }
}

impl Server {
    /// The catalogue, loaded on first use (about half a second).
    fn catalog(&self) -> Result<Arc<Catalog>, String> {
        let mut state = self.state.lock().unwrap();
        if let Some(c) = &state.catalog {
            return Ok(c.clone());
        }
        let game = state.game_dir.clone().ok_or("X4 install not found; start with --game-dir or set X4_GAME_DIR")?;
        let catalog = Arc::new(Catalog::load(&game, state.profile_dir.as_deref()).map_err(|e| e.to_string())?);
        state.names = catalog
            .station_modules()
            .map_err(|e| e.to_string())?
            .into_iter()
            .map(|m| (m.macro_name.to_ascii_lowercase(), m.name))
            .collect();
        state.catalog = Some(catalog.clone());
        Ok(catalog)
    }

    /// Runs an editing operation on the open design.
    fn edit(&self, op: impl FnOnce(&mut Design, &Catalog) -> x4sd_core::Result<Value>) -> ToolResult {
        let catalog = self.catalog()?;
        let mut state = self.state.lock().unwrap();
        let design = state.design.as_mut().ok_or("no open design; use new_design or open_design")?;
        op(design, &catalog).map(|v| v.to_string()).map_err(|e| e.to_string())
    }
}

fn transform(position: Option<[f64; 3]>, yaw: Option<f64>, pitch: Option<f64>, roll: Option<f64>) -> Transform {
    Transform {
        position: position.unwrap_or_default(),
        rotation: Quat::from_euler(yaw.unwrap_or(0.0), pitch.unwrap_or(0.0), roll.unwrap_or(0.0)),
    }
}

fn round3(v: [f64; 3]) -> [f64; 3] {
    v.map(|x| (x * 1000.0).round() / 1000.0 + 0.0)
}

fn snap_json(name: &str, t: &Transform) -> Value {
    json!({"name": name, "position": round3(t.position), "facing": round3(t.rotation.rotate([0.0, 0.0, 1.0]))})
}

pub fn serve(game_dir: Option<PathBuf>, profile_dir: Option<PathBuf>) -> x4sd_core::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let service = Server::new(game_dir, profile_dir)
            .serve(rmcp::transport::stdio())
            .await
            .map_err(|e| x4sd_core::Error(format!("MCP start failed: {e}")))?;
        service.waiting().await.map_err(|e| x4sd_core::Error(format!("MCP server stopped: {e}")))?;
        Ok(())
    })
}
