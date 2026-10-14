//! `x4sd mcp`: MCP server over stdio for AI agents (D-8). When the builder is
//! running, every operation goes to the design open there (D-7, `link.rs`);
//! otherwise the server keeps its own design (headless).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::{Implementation, ServerCapabilities, ServerConfig};
use rmcp::{schemars, tool, tool_handler, tool_router, ServerHandler, ServiceExt};
use serde::Deserialize;
use serde_json::json;
use x4sd_core::catalog::Catalog;
use x4sd_core::design::Design;

use crate::link::Client;
use crate::ops::{self, Ctx, ModuleNames, Op};

const INSTRUCTIONS: &str = "\
Designs X4: Foundations station construction plans and puts them into the game.
Workflow: list_modules to find module macros -> new_design (or open_design) -> add_module for the first \
module -> free_snaps to see where modules can attach -> attach_module (snap to snap; twist rotates around \
the snap axis, usually in 90 degree steps) -> validate_design -> export_design, then the player clicks \
Import in the game's station build menu.
Planning a working station (\"a staffed station with 100 solar panels\"): list_wares for ware ids (solar \
panels are Energy Cell Production, ware energycells) -> plan_production with targets (ware + number of \
production modules); it adds the production chain, habitats with their food and medicine, storage, a dock \
area and a pier -> auto_place with from_plan (the same settings) attaches every module it lists, adding \
connection structures where needed -> validate_design (if the station outgrew its plot, tell the user and \
fit_plot with grow_only) -> analyze_station to check (wares still short must be bought or mined) -> \
build_cost for the materials. auto_place also takes a plain module list. Self-sufficient means every made ware is produced on \
the station; raw materials (ore, ice, silicon, nividium, methane, helium, hydrogen, scrap metal: flows with \
raw=true) are always brought in (mined or salvaged), never planned, so a self-sufficient station still lists \
them as short (self_sufficient=true).
Coordinates are metres in the station frame, Y up. Module indices are 1..n and are renumbered after \
remove_module. Every edit can be undone with undo.
Equipment: turrets and shields go on a module's groups (most production, storage and defence modules have \
some; a typical group is 2 M turret slots + 1 M shield slot). module_equipment shows a module's groups with \
what fits; set_equipment fills one group; equip_modules fills many at once, from picks (e.g. one M turret, \
one M shield, one L turret, one L shield: each group takes the first that fits) or a loadout level \
(none/low/medium/high); copy_equipment copies one module's equipment to its type or, as picks, to all. \
list_equipment finds macros. build_cost includes equipment.
Overlaps: validate_design reports modules whose collision shapes intersect (modules joined at a snap may \
interlock); auto_place never creates any.
Plot: a station must fit its plot (whole km per side from the station origin, at most 20 km per axis). \
New designs start on a 2x2x2 km plot, opened ones on the smallest that fits. validate_design reports modules \
outside the plot and the size needed; set_plot / fit_plot change it (tell the user when the plot must grow).
Build order: modules are built in index order, each after the module it attaches to. move_in_build_order, \
build_first and build_last reorder (and renumber, so describe again); set_stage_end splits the build into stages.
Moving: reattach_module moves a placed module (with everything on it) onto another free snap; detach_module \
makes it free-standing; move_module moves free-standing ones.
If the X4 Station Designer builder window is open, all tools work on the design shown there and the \
user sees every change live (builder_status tells which).";

#[derive(Clone)]
pub struct Server {
    state: Arc<Mutex<State>>,
    tool_router: ToolRouter<Server>,
}

struct State {
    game_dir: Option<PathBuf>,
    profile_dir: Option<PathBuf>,
    /// Connection to a running builder, if any.
    link: Option<Client>,
    /// Own catalogue and design, used when no builder runs.
    local: Option<(Catalog, ModuleNames)>,
    design: Design,
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
pub struct PlotArgs {
    /// Reach on the positive side of X, Y, Z in whole km
    pos: [u32; 3],
    /// Reach on the negative side of X, Y, Z in whole km
    neg: [u32; 3],
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct FitPlotArgs {
    /// Only enlarge the current plot (default: set it to the smallest that holds the station)
    grow_only: Option<bool>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct MoveArgsOrder {
    /// Module index
    index: u32,
    /// New 1-based position in the build order
    position: usize,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ReattachArgs {
    /// Module to move (everything attached to it moves along)
    index: u32,
    /// Index of the placed module to attach it to (not the module itself or one attached to it)
    to: u32,
    /// Free snap of that module (see free_snaps)
    to_snap: String,
    /// Snap of the moved module to connect; default: its current one, else its first
    snap: Option<String>,
    /// Rotation around the snap axis in degrees; default 0
    twist: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CostArgs {
    /// Module index; omit for the whole station
    index: Option<u32>,
    /// Build method: default (Universal), terran, boron, ... (default: default)
    method: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct AnalyzeArgs {
    /// The sector's sunlight in percent; solar output (energy cells) follows it (default 100)
    sunlight: Option<f64>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct StageArgs {
    /// Module index
    index: u32,
    /// true: a build stage ends after this module
    end: bool,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ListEquipmentArgs {
    /// turrets or shields
    kind: Option<String>,
    /// small, medium, large, extralarge (station modules use medium and large)
    size: Option<String>,
    /// Maker race, e.g. argon, terran, boron
    race: Option<String>,
    /// Text to look for in the name or macro
    search: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SetEquipmentArgs {
    /// Module index
    index: u32,
    /// Group name from module_equipment, e.g. group01
    group: String,
    /// turrets or shields
    kind: String,
    /// Equipment macro that fits the group (see module_equipment); omit to remove the equipment
    r#macro: Option<String>,
    /// How many of the group's slots of that kind to fill; default all
    count: Option<u32>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CopyEquipmentArgs {
    /// Module whose equipment to copy
    from: u32,
    /// same_type (default): an exact copy onto every module of the same type; all: its equipment macros as picks for every module
    scope: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct NoArgs {}

#[tool_router]
impl Server {
    pub fn new(game_dir: Option<PathBuf>, profile_dir: Option<PathBuf>) -> Server {
        let state = State { game_dir, profile_dir, link: None, local: None, design: Design::new("New station") };
        Server { state: Arc::new(Mutex::new(state)), tool_router: Self::tool_router() }
    }

    #[tool(description = "Whether a builder window is running: if so, all tools act on the design shown there, live.")]
    fn builder_status(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        let mut state = self.state.lock().unwrap();
        if state.link.is_none() {
            state.link = Client::connect();
        }
        Ok(json!({"builder": state.link.is_some()}).to_string())
    }

    #[tool(description = "List buildable station modules (base game, DLCs, enabled mods): macro, class, race, source, display name.")]
    fn list_modules(&self, Parameters(a): Parameters<ListModulesArgs>) -> ToolResult {
        self.run(Op::ListModules { class: a.class, race: a.race, search: a.search })
    }

    #[tool(description = "A module's snap connections: name, position and facing direction in the module's own frame.")]
    fn module_snaps(&self, Parameters(a): Parameters<MacroArgs>) -> ToolResult {
        self.run(Op::ModuleSnaps { r#macro: a.r#macro })
    }

    #[tool(description = "Start a new, empty design (replaces the open one).")]
    fn new_design(&self, Parameters(a): Parameters<NameArgs>) -> ToolResult {
        self.run(Op::NewDesign { name: a.name })
    }

    #[tool(description = "Open a plan from a plan file for editing (replaces the open design).")]
    fn open_design(&self, Parameters(a): Parameters<FileArgs>) -> ToolResult {
        self.run(Op::OpenDesign { file: a.file, plan: a.plan })
    }

    #[tool(description = "The open design: id, name, DLC patches, and every module with index, macro, name, position, what it is attached to, and its free snaps.")]
    fn describe_design(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        self.run(Op::DescribeDesign)
    }

    #[tool(description = "Rename the open design.")]
    fn rename_design(&self, Parameters(a): Parameters<NameArgs>) -> ToolResult {
        self.run(Op::RenameDesign { name: a.name })
    }

    #[tool(description = "Place a module on its own (not attached), e.g. the first module of a station. Returns its index.")]
    fn add_module(&self, Parameters(a): Parameters<PlaceArgs>) -> ToolResult {
        self.run(Op::AddModule { r#macro: a.r#macro, position: a.position, yaw: a.yaw, pitch: a.pitch, roll: a.roll })
    }

    #[tool(description = "Add a module by connecting one of its snaps to a free snap of a placed module. Returns the new index.")]
    fn attach_module(&self, Parameters(a): Parameters<AttachArgs>) -> ToolResult {
        self.run(Op::AttachModule { r#macro: a.r#macro, snap: a.snap, to: a.to, to_snap: a.to_snap, twist: a.twist })
    }

    #[tool(description = "Rotate an attached module around its snap axis to an absolute twist (degrees). Everything attached to it moves along.")]
    fn set_twist(&self, Parameters(a): Parameters<TwistArgs>) -> ToolResult {
        self.run(Op::SetTwist { index: a.index, twist: a.twist })
    }

    #[tool(description = "Move a module that is not attached to anything (and everything attached to it).")]
    fn move_module(&self, Parameters(a): Parameters<MoveArgs>) -> ToolResult {
        self.run(Op::MoveModule { index: a.index, position: a.position, yaw: a.yaw, pitch: a.pitch, roll: a.roll })
    }

    #[tool(description = "Remove a module and everything attached to it. The remaining modules are renumbered 1..n.")]
    fn remove_module(&self, Parameters(a): Parameters<IndexArgs>) -> ToolResult {
        self.run(Op::RemoveModule { index: a.index })
    }

    #[tool(description = "Free snaps of a placed module, where new modules can attach: name, station-frame position and facing direction.")]
    fn free_snaps(&self, Parameters(a): Parameters<IndexArgs>) -> ToolResult {
        self.run(Op::FreeSnaps { index: a.index })
    }

    #[tool(description = "Undo the last edit.")]
    fn undo(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        self.run(Op::Undo)
    }

    #[tool(description = "Redo the last undone edit.")]
    fn redo(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        self.run(Op::Redo)
    }

    #[tool(description = "Check the open design for problems the game does not catch (unknown modules, modules off their snaps, overlapping modules, missing DLC patches, modules outside the plot). An empty list means it is fine.")]
    fn validate_design(&self, Parameters(_): Parameters<NoArgs>) -> ToolResult {
        self.run(Op::ValidateDesign)
    }

    #[tool(description = "Save the open design to a plan file (single-plan format, like the game's exports).")]
    fn save_design(&self, Parameters(a): Parameters<SaveArgs>) -> ToolResult {
        self.run(Op::SaveDesign { file: a.file })
    }

    #[tool(description = "Set the plot: how far it reaches from the station origin on each side, in whole km (each axis 1 to 20 km in total).")]
    fn set_plot(&self, Parameters(a): Parameters<PlotArgs>) -> ToolResult {
        self.run(Op::SetPlot { pos: a.pos, neg: a.neg })
    }

    #[tool(description = "Fit the plot to the station: the smallest plot that holds it, or with grow_only just enlarge the current one. Fails if the station is longer than 20 km on an axis.")]
    fn fit_plot(&self, Parameters(a): Parameters<FitPlotArgs>) -> ToolResult {
        self.run(Op::FitPlot { grow_only: a.grow_only })
    }

    #[tool(description = "Move a module to a position in the build order (modules are built in index order). Refused if it would come before the module it attaches to, or after one attached to it. Modules are renumbered.")]
    fn move_in_build_order(&self, Parameters(a): Parameters<MoveArgsOrder>) -> ToolResult {
        self.run(Op::MoveInBuildOrder { index: a.index, position: a.position })
    }

    #[tool(description = "Build a module as early as possible: it and the chain of modules it hangs on move to the front. Modules are renumbered.")]
    fn build_first(&self, Parameters(a): Parameters<IndexArgs>) -> ToolResult {
        self.run(Op::BuildFirst { index: a.index })
    }

    #[tool(description = "Build a module as late as possible: it and everything attached to it move to the end. Modules are renumbered.")]
    fn build_last(&self, Parameters(a): Parameters<IndexArgs>) -> ToolResult {
        self.run(Op::BuildLast { index: a.index })
    }

    #[tool(description = "Move a placed module, with everything attached to it, onto another free snap. Modules may be renumbered (the result has moved_to).")]
    fn reattach_module(&self, Parameters(a): Parameters<ReattachArgs>) -> ToolResult {
        self.run(Op::ReattachModule { index: a.index, to: a.to, to_snap: a.to_snap, snap: a.snap, twist: a.twist })
    }

    #[tool(description = "Make an attached module free-standing where it is (whatever is attached to it stays attached).")]
    fn detach_module(&self, Parameters(a): Parameters<IndexArgs>) -> ToolResult {
        self.run(Op::DetachModule { index: a.index })
    }

    #[tool(description = "Build materials: the resources (and estimated value at average prices) to build one module with its equipment, or the whole station. Also lists the build methods.")]
    fn build_cost(&self, Parameters(a): Parameters<CostArgs>) -> ToolResult {
        self.run(Op::BuildCost { index: a.index, method: a.method })
    }

    #[tool(description = "Station analysis: what the whole station produces and uses per hour (net per ware, largest surplus first), workforce needed vs housed (the work bonus scales with it; workers' food and medicine are included), storage per cargo type in hours, dock areas and piers, and notes on what is missing.")]
    fn analyze_station(&self, Parameters(a): Parameters<AnalyzeArgs>) -> ToolResult {
        self.run(Op::AnalyzeStation { sunlight: a.sunlight })
    }

    #[tool(description = "Wares that some buildable station module makes: ware id and name (for plan_production targets).")]
    fn list_wares(&self) -> ToolResult {
        self.run(Op::ListWares)
    }

    #[tool(description = "Production planner: the modules to add to the open design to make the target wares. It adds the whole production chain, habitats for the workforce with their food and medicine, storage for N hours, and a dock area and pier if missing. Existing modules count as supply; with no targets it fixes the station's shortfalls. Returns the modules (macro, count, role), the station's analysis with them (raw materials stay short: they are mined or salvaged; self_sufficient says whether every made ware is covered) and the cost. It doesn't place anything: auto_place does.")]
    fn plan_production(&self, Parameters(a): Parameters<ops::PlanArgs>) -> ToolResult {
        self.run(Op::PlanProduction(a))
    }

    #[tool(description = "Auto-placement: attach modules to the open design at exact snap positions, clear of each other and inside the plot, adding connection structures (hubs, straight pieces) where needed; one undo step. Give modules (macro and count), or from_plan (plan_production settings) to plan and place in one call (the result then has the plan too). Big modules go first, dock areas and piers last: every run also lifts off the dock areas and piers with nothing attached and places them again where a 3 km corridor out from the station is clear, and no module may block a dock's corridor. Returns the new modules and connection structures (index, macro), what didn't fit, docks_moved and docks_blocked (no clear corridor anywhere), and the plot: with grow_plot (default true) the station may outgrow its plot, up to 20 km per axis; then tell the user and fit_plot with grow_only.")]
    fn auto_place(&self, Parameters(a): Parameters<ops::AutoPlaceArgs>) -> ToolResult {
        self.run(Op::AutoPlace(a))
    }

    #[tool(description = "Turrets and shields players can build: macro, name, kind, size, race, mark and average price.")]
    fn list_equipment(&self, Parameters(a): Parameters<ListEquipmentArgs>) -> ToolResult {
        self.run(Op::ListEquipment { kind: a.kind, size: a.size, race: a.race, search: a.search })
    }

    #[tool(description = "A placed module's equipment groups: for each group and kind (turrets, shields) the slot size and count, what is equipped, and the equipment macros that fit.")]
    fn module_equipment(&self, Parameters(a): Parameters<IndexArgs>) -> ToolResult {
        self.run(Op::ModuleEquipment { index: a.index })
    }

    #[tool(description = "Equip one kind of slot (turrets or shields) in a group of a module with a fitting macro (default: all its slots), or without macro remove it. Returns the module's groups.")]
    fn set_equipment(&self, Parameters(a): Parameters<SetEquipmentArgs>) -> ToolResult {
        self.run(Op::SetEquipment { index: a.index, group: a.group, kind: a.kind, r#macro: a.r#macro, count: a.count })
    }

    #[tool(description = "Equip many modules at once (one undo step): picks (equipment macros; each group takes, per kind, the first that fits all its slots) or a loadout level (none, low, medium, high). Default: every module with groups; modules or defence_only narrow it. Returns how many group slots were filled, cleared or skipped (nothing fitted).")]
    fn equip_modules(&self, Parameters(a): Parameters<ops::EquipArgs>) -> ToolResult {
        self.run(Op::EquipModules(a))
    }

    #[tool(description = "Copy a module's equipment: scope same_type (default) gives every module of its type the same equipment; all uses its equipment macros as picks for every module.")]
    fn copy_equipment(&self, Parameters(a): Parameters<CopyEquipmentArgs>) -> ToolResult {
        self.run(Op::CopyEquipment { from: a.from, scope: a.scope })
    }

    #[tool(description = "End a build stage after this module (or remove that stage end). Stages let the station be built in steps.")]
    fn set_stage_end(&self, Parameters(a): Parameters<StageArgs>) -> ToolResult {
        self.run(Op::SetStageEnd { index: a.index, end: a.end })
    }

    #[tool(description = "Put the open design into the game: writes it to the player profile's constructionplan folder. The player then clicks Import in the station build menu.")]
    fn export_design(&self, Parameters(a): Parameters<ExportArgs>) -> ToolResult {
        self.run(Op::ExportDesign { new_id: a.new_id, overwrite: a.overwrite })
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
    /// Runs an operation in the builder if one is running, else on the own design.
    fn run(&self, op: Op) -> ToolResult {
        let mut state = self.state.lock().unwrap();
        if state.link.is_none() {
            state.link = Client::connect();
        }
        if let Some(link) = state.link.as_mut() {
            return match link.send(&op) {
                Ok(result) => result.map(|v| v.to_string()),
                Err(e) => {
                    state.link = None;
                    Err(format!(
                        "lost the connection to the builder ({e}); the operation may not have been applied. \
                         Further calls use the builder again if it is reopened, otherwise a separate headless design."
                    ))
                }
            };
        }
        if state.local.is_none() {
            let game = state.game_dir.clone().ok_or("X4 install not found; start with --game-dir or set X4_GAME_DIR")?;
            state.local = Some(ops::load(&game, state.profile_dir.as_deref()).map_err(|e| e.to_string())?);
        }
        let State { local: Some((catalog, names)), design, profile_dir, .. } = &mut *state else { unreachable!() };
        let mut ctx = Ctx { catalog, names, design, profile_dir: profile_dir.as_deref() };
        ops::execute(op, &mut ctx).map(|v| v.to_string())
    }
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
