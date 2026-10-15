//! The design operations agents use (D-7). `x4sd mcp` runs them on its own
//! design, or sends them to a running builder (`link.rs`), which runs them on
//! the design open there. Either way they end up here.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use x4sd_core::catalog::Catalog;
use x4sd_core::design::Design;
use x4sd_core::economy::{analyze, Analysis};
use x4sd_core::equipment::{module_groups, Fill, Level};
use x4sd_core::geometry::{Quat, Transform};
use x4sd_core::import::{new_plan_id, renew_id_if_renamed, write_import_file};
use x4sd_core::layout::{self, OptimizeOptions, Options, Side};
use x4sd_core::perf;
use x4sd_core::plan::{parse_plans, write_plans};
use x4sd_core::planner::{self, module_counts, producible_wares, ProductionPlan, Request};
use x4sd_core::plot::{required_plot, Plot};
use x4sd_core::save::{newest_save, owned_modules, read_blueprints};
use x4sd_core::validate::{validate, validate_plot, Issue};

/// A production target: this many modules making `ware`.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Target {
    /// Ware id, e.g. hullparts (see list_wares)
    pub ware: String,
    /// Production modules to add for it
    pub modules: u32,
}

/// Production planner settings.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct PlanArgs {
    /// Wares to make and how many production modules each; empty: just fix the station's shortfalls
    #[serde(default)]
    pub targets: Vec<Target>,
    /// Other modules to add, e.g. ship fabrication or maintenance bays, defence platforms (macros from list_modules); their workforce is housed like the rest. sides only matter to auto_place with from_plan
    pub extra_modules: Option<Vec<ModuleCount>>,
    /// Preferred race of the modules: argon, boron, paranid, split, teladi, terran; omit for any
    pub race: Option<String>,
    /// Add habitats for the workforce and make their food and medicine (default true)
    pub workforce: Option<bool>,
    /// Hours of production and consumption the storage should hold (default 6)
    pub storage_hours: Option<f64>,
    /// At least this much storage in m³ per cargo type (container, solid, liquid; gases are liquid), e.g. {"container": 1000000, "liquid": 200000}; on top of storage_hours (each type gets the larger)
    pub storage: Option<BTreeMap<String, f64>>,
    /// Ware ids to buy instead of making
    pub buy: Option<Vec<String>>,
    /// The sector's sunlight in percent (default 100)
    pub sunlight: Option<f64>,
    /// Build method for the cost (default: default)
    pub method: Option<String>,
}

/// Modules of one macro to place.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ModuleCount {
    /// Module macro, e.g. prod_gen_energycells_macro
    pub r#macro: String,
    /// How many to place
    pub count: u32,
    /// For auto_place: sides of the station to put them on, spread evenly (extras to the first): front (+Z), back, left (-X), right, top (+Y), bottom. Each goes on the outside on its side with nothing in front of it; meant for defence platforms. Default: anywhere
    pub sides: Option<Vec<String>>,
}

/// What auto_place places, and how.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct AutoPlaceArgs {
    /// Modules to place (macro and count); give this or from_plan
    pub modules: Option<Vec<ModuleCount>>,
    /// Plan first with these plan_production settings, then place every module the plan lists
    pub from_plan: Option<PlanArgs>,
    /// Let the station grow past its plot, up to the largest plot (20 km per axis); default true
    pub grow_plot: Option<bool>,
    /// Race of the connection structures it adds: argon, boron, paranid, split, teladi, terran; default: the plan's race, else the most common race of the modules
    pub race: Option<String>,
    /// Add fewer connection structures (less for the game to run, a larger station); default false
    pub fewer_connectors: Option<bool>,
    /// When a module finds no place, break the station up to make one: a connection hub goes into a joint and what hangs there moves out by its length (the station's modules may move); default true
    pub break_up: Option<bool>,
}

/// Optimize settings: the planner's (for the shortfall fixes) and the layout's.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OptimizeArgs {
    /// Planner settings for the modules that fix the shortfalls (targets are ignored)
    #[serde(flatten)]
    pub plan: PlanArgs,
    /// Keep the connection structures, and the modules carrying them, where they are (the station's shape); false rebuilds them from scratch (default true)
    pub keep_shape: Option<bool>,
    /// Add fewer connection structures (less for the game to run, a larger station); default false
    pub fewer_connectors: Option<bool>,
    /// The modules to add, e.g. a plan_production result (no targets) the user trimmed; default: plan the shortfall fixes with the settings
    pub modules: Option<Vec<ModuleCount>>,
    /// Keep a partial result when some of the added modules find no place (listed in unplaced; the station's own modules always stay); default false: all or nothing
    pub partial: Option<bool>,
}

/// How equip_modules chooses equipment, and for which modules.
#[derive(Debug, Clone, Default, Serialize, Deserialize, schemars::JsonSchema)]
pub struct EquipArgs {
    /// Equipment macros to use: each group gets, per kind, the first one that fits all its slots (e.g. one M turret, one M shield, one L turret, one L shield). Give this or level
    pub picks: Option<Vec<String>>,
    /// Loadout level instead of picks: none (remove), low, medium, high (cheapest, middle or most expensive fitting non-missile equipment, the module's race first)
    pub level: Option<String>,
    /// Module indices; default: every module (those without groups are left alone)
    pub modules: Option<Vec<u32>>,
    /// Only defence modules
    pub defence_only: Option<bool>,
}

/// Lowercase macro -> (display name, class) of every station module.
pub type ModuleNames = HashMap<String, (String, String)>;

/// The player's blueprints, read from a save (D-16). While `on`, the module
/// lists and the modules the planner and auto-placement choose are limited to
/// `modules`; modules named explicitly are used as given.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Blueprints {
    pub on: bool,
    /// The save read last.
    pub save: Option<String>,
    /// When it was read, unix seconds.
    pub read_at: Option<u64>,
    /// Station-module macros the player has blueprints for.
    pub modules: Vec<String>,
}

impl Blueprints {
    /// Reads the save's blueprints (about half a second; a save is ~1 GB of XML) and turns the filter on.
    pub fn read(catalog: &Catalog, save: &Path) -> Result<Blueprints, String> {
        let wares = read_blueprints(save).map_err(|e| e.to_string())?;
        let modules = owned_modules(catalog, &wares).map_err(|e| e.to_string())?;
        let read_at = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs());
        Ok(Blueprints { on: true, save: Some(save.display().to_string()), read_at: Some(read_at), modules })
    }

    /// The owned modules (lowercase macros) while the filter is on.
    pub fn owned(&self) -> Option<HashSet<String>> {
        (self.on && self.save.is_some()).then(|| self.modules.iter().map(|m| m.to_ascii_lowercase()).collect())
    }

    /// For agents: whether the filter is on, the save, and how many modules it allows.
    pub fn status(&self, names: &ModuleNames) -> Value {
        json!({"on": self.on, "save": self.save, "read_at": self.read_at, "modules": self.modules.len(), "station_modules": names.len()})
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    ListModules { class: Option<String>, race: Option<String>, search: Option<String> },
    ModuleSnaps { r#macro: String },
    NewDesign { name: String },
    OpenDesign { file: String, plan: Option<String> },
    DescribeDesign,
    RenameDesign { name: String },
    AddModule { r#macro: String, position: Option<[f64; 3]>, yaw: Option<f64>, pitch: Option<f64>, roll: Option<f64> },
    AttachModule { r#macro: String, snap: Option<String>, to: u32, to_snap: String, twist: Option<f64> },
    SetTwist { index: u32, twist: f64 },
    MoveModule { index: u32, position: [f64; 3], yaw: Option<f64>, pitch: Option<f64>, roll: Option<f64> },
    RemoveModule { index: u32 },
    FreeSnaps { index: u32 },
    Undo,
    Redo,
    ValidateDesign,
    SaveDesign { file: String },
    ExportDesign { new_id: Option<bool>, overwrite: Option<bool> },
    /// Plot reach per side in km: `pos` = +X, +Y, +Z, `neg` = −X, −Y, −Z.
    SetPlot { pos: [u32; 3], neg: [u32; 3] },
    FitPlot { grow_only: Option<bool> },
    /// Build order (= module order): move a module to a 1-based position.
    MoveInBuildOrder { index: u32, position: usize },
    BuildFirst { index: u32 },
    BuildLast { index: u32 },
    /// End (or not) a build stage after this module.
    SetStageEnd { index: u32, end: bool },
    /// Move a module (and everything on it) onto another free snap.
    ReattachModule { index: u32, to: u32, to_snap: String, snap: Option<String>, twist: Option<f64> },
    DetachModule { index: u32 },
    /// Resources to build one module (with its equipment) or the whole station.
    BuildCost { index: Option<u32>, method: Option<String> },
    /// Production and use per hour, workforce, storage and docks of the whole station.
    AnalyzeStation { sunlight: Option<f64> },
    /// What the station likely costs the game to run: modules, connection structures, turrets, docks, size.
    StationLoad,
    /// Wares some buildable module makes.
    ListWares,
    /// The modules to add for target wares (or to fix the station's shortfalls).
    PlanProduction(PlanArgs),
    /// Attach modules (a list, or a plan's) at free snaps without overlaps.
    AutoPlace(AutoPlaceArgs),
    /// Turrets and shields players can build.
    ListEquipment { kind: Option<String>, size: Option<String>, race: Option<String>, search: Option<String> },
    /// A module's equipment groups: slots, what is on them, what fits.
    ModuleEquipment { index: u32 },
    /// Equip (or with no macro, clear) one kind of slot in a group.
    SetEquipment { index: u32, group: String, kind: String, r#macro: Option<String>, count: Option<u32> },
    /// Equip many modules by picks or by loadout level.
    EquipModules(EquipArgs),
    /// Fix the station's shortfalls and re-lay it out in layers.
    OptimizeStation(OptimizeArgs),
    /// Give modules the equipment of one: `same_type` (default) copies it to the
    /// modules of its type, `all` uses its equipment as picks for every module.
    CopyEquipment { from: u32, scope: Option<String> },
    /// Read the player's blueprints from a save (default: the newest) and limit the modules to them.
    ReadBlueprints { save: Option<String> },
    /// Turn the blueprint limit on (the last save read) or off; no `on`: just the status.
    BlueprintFilter { on: Option<bool> },
}

impl Op {
    /// Whether the open design may change (the builder then redraws).
    pub fn changes_design(&self) -> bool {
        // Save and export count: they change whether the design has unsaved changes.
        !matches!(
            self,
            Op::ListModules { .. }
                | Op::ModuleSnaps { .. }
                | Op::DescribeDesign
                | Op::FreeSnaps { .. }
                | Op::ValidateDesign
                | Op::BuildCost { .. }
                | Op::AnalyzeStation { .. }
                | Op::StationLoad
                | Op::ListWares
                | Op::ListEquipment { .. }
                | Op::ModuleEquipment { .. }
                | Op::PlanProduction(_)
                | Op::ReadBlueprints { .. }
                | Op::BlueprintFilter { .. }
        )
    }

    /// Whether it changes the blueprint limit (the builder then updates its palette).
    pub fn changes_blueprints(&self) -> bool {
        matches!(self, Op::ReadBlueprints { .. } | Op::BlueprintFilter { on: Some(_) })
    }

    /// Whether the change replaces the whole design (the builder then refits the view).
    pub fn replaces_design(&self) -> bool {
        matches!(self, Op::NewDesign { .. } | Op::OpenDesign { .. })
    }

    pub fn name(&self) -> String {
        serde_json::to_value(self).ok().and_then(|v| v["op"].as_str().map(str::to_owned)).unwrap_or_default()
    }
}

pub struct Ctx<'a> {
    pub catalog: &'a Catalog,
    pub names: &'a ModuleNames,
    pub design: &'a mut Design,
    pub profile_dir: Option<&'a Path>,
    pub blueprints: &'a mut Blueprints,
}

/// Loads the game data and the station-module names.
pub fn load(game_dir: &Path, profile_dir: Option<&Path>) -> x4sd_core::Result<(Catalog, ModuleNames)> {
    let catalog = Catalog::load(game_dir, profile_dir)?;
    let names = catalog
        .station_modules()?
        .into_iter()
        .map(|m| (m.macro_name.to_ascii_lowercase(), (m.name, m.class)))
        .collect();
    Ok((catalog, names))
}

pub fn execute(op: Op, ctx: &mut Ctx) -> Result<Value, String> {
    let c = ctx.catalog;
    let err = |e: x4sd_core::Error| e.to_string();
    match op {
        Op::ListModules { class, race, search } => {
            let search = search.map(|s| s.to_lowercase());
            let owned = ctx.blueprints.owned();
            let modules: Vec<Value> = c
                .station_modules()
                .map_err(err)?
                .into_iter()
                .filter(|m| class.as_ref().is_none_or(|x| m.class.eq_ignore_ascii_case(x)))
                .filter(|m| race.as_ref().is_none_or(|r| m.race.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(r))))
                .filter(|m| search.as_ref().is_none_or(|s| m.name.to_lowercase().contains(s) || m.macro_name.to_lowercase().contains(s)))
                .filter(|m| owned.as_ref().is_none_or(|o| o.contains(&m.macro_name.to_ascii_lowercase())))
                .map(|m| json!({"macro": m.macro_name, "class": m.class, "race": m.race, "source": m.source, "name": m.name}))
                .collect();
            Ok(json!(modules))
        }
        Op::ModuleSnaps { r#macro } => {
            let module = c.module(&r#macro).map_err(err)?;
            let snaps: Vec<Value> = module.snaps.iter().map(|s| snap_json(&s.name, &s.transform)).collect();
            Ok(json!({"macro": module.macro_name, "class": module.class, "snaps": snaps}))
        }
        Op::NewDesign { name } => {
            *ctx.design = Design::new(&name);
            describe(ctx)
        }
        Op::OpenDesign { file, plan } => {
            let xml = std::fs::read_to_string(&file).map_err(|e| format!("{file}: {e}"))?;
            let mut plans = parse_plans(&xml).map_err(err)?;
            if let Some(w) = &plan {
                plans.retain(|p| &p.id == w || &p.name == w);
            }
            if plans.len() != 1 {
                let names: Vec<_> = plans.iter().map(|p| p.name.as_str()).collect();
                return Err(format!("{} plans match; pass plan= one of {names:?}", plans.len()));
            }
            *ctx.design = Design::from_plan(plans.remove(0));
            // The plan file has no plot: start with the smallest that holds the station.
            let _ = ctx.design.fit_plot(c, false);
            describe(ctx)
        }
        Op::DescribeDesign => describe(ctx),
        Op::RenameDesign { name } => {
            ctx.design.rename(&name);
            Ok(json!({"name": name}))
        }
        Op::AddModule { r#macro, position, yaw, pitch, roll } => {
            let index = ctx.design.add_root(c, &r#macro, transform(position, yaw, pitch, roll)).map_err(err)?;
            Ok(json!({"index": index}))
        }
        Op::AttachModule { r#macro, snap, to, to_snap, twist } => {
            let snap = match snap {
                Some(s) => s,
                None => c.module(&r#macro).map_err(err)?.snaps.first().map(|s| s.name.clone()).ok_or(format!("{} has no snaps", r#macro))?,
            };
            let index = ctx.design.attach(c, &r#macro, &snap, to, &to_snap, twist.unwrap_or(0.0)).map_err(err)?;
            Ok(json!({"index": index, "snap": snap}))
        }
        Op::SetTwist { index, twist } => {
            ctx.design.set_twist(c, index, twist).map_err(err)?;
            Ok(json!({"index": index, "twist": twist}))
        }
        Op::MoveModule { index, position, yaw, pitch, roll } => {
            ctx.design.move_root(index, transform(Some(position), yaw, pitch, roll)).map_err(err)?;
            Ok(json!({"index": index}))
        }
        Op::RemoveModule { index } => Ok(json!({"removed": ctx.design.remove(c, index).map_err(err)?})),
        Op::FreeSnaps { index } => {
            let snaps = ctx.design.free_snaps(c, index).map_err(err)?;
            Ok(json!(snaps.iter().map(|s| snap_json(&s.name, &s.world)).collect::<Vec<_>>()))
        }
        Op::Undo => Ok(json!({"undone": ctx.design.undo()})),
        Op::Redo => Ok(json!({"redone": ctx.design.redo()})),
        Op::ValidateDesign => Ok(json!({"issues": issue_list(ctx.design, c, ctx.blueprints, ctx.names).iter().map(|i| i.to_string()).collect::<Vec<_>>()})),
        Op::SetPlot { pos, neg } => {
            ctx.design.set_plot(Some(Plot { pos, neg })).map_err(err)?;
            Ok(plot_status(ctx.design, c))
        }
        Op::FitPlot { grow_only } => {
            ctx.design.fit_plot(c, grow_only.unwrap_or(false)).map_err(err)?;
            Ok(plot_status(ctx.design, c))
        }
        Op::MoveInBuildOrder { index, position } => {
            let moved_to = ctx.design.move_in_build_order(index, position).map_err(err)?;
            moved(ctx, moved_to)
        }
        Op::BuildFirst { index } => {
            let moved_to = ctx.design.build_first(index).map_err(err)?;
            moved(ctx, moved_to)
        }
        Op::BuildLast { index } => {
            let moved_to = ctx.design.build_last(index).map_err(err)?;
            moved(ctx, moved_to)
        }
        Op::SetStageEnd { index, end } => {
            ctx.design.set_stage_end(index, end).map_err(err)?;
            describe(ctx)
        }
        Op::ReattachModule { index, to, to_snap, snap, twist } => {
            let snap = match snap {
                Some(s) => s,
                None => default_snap(ctx.design, c, index)?,
            };
            let moved_to = ctx.design.reattach(c, index, &snap, to, &to_snap, twist.unwrap_or(0.0)).map_err(err)?;
            moved(ctx, moved_to)
        }
        Op::DetachModule { index } => {
            ctx.design.detach(index).map_err(err)?;
            describe(ctx)
        }
        Op::BuildCost { index, method } => build_cost(c, ctx.design, index, method.as_deref().unwrap_or("default")),
        Op::AnalyzeStation { sunlight } => analyze_station(c, ctx.design, sunlight.unwrap_or(100.0)),
        Op::StationLoad => Ok(station_load(c, ctx.design)),
        Op::ListWares => list_wares(c),
        Op::ListEquipment { kind, size, race, search } => list_equipment(c, kind, size, race, search),
        Op::ModuleEquipment { index } => module_equipment(c, ctx.design, index),
        Op::SetEquipment { index, group, kind, r#macro, count } => {
            let kind = if kind.ends_with('s') { kind } else { format!("{kind}s") };
            let slots = ctx.design.plan().entries.iter().find(|e| e.index == index).and_then(|e| c.module(&e.macro_name).ok());
            let slots = slots.map_or(0, |m| m.group_slots(&group, &kind).count() as u32);
            let equip = r#macro.as_deref().map(|m| (m, count.unwrap_or(slots)));
            ctx.design.set_equipment(c, index, &group, &kind, equip).map_err(err)?;
            module_equipment(c, ctx.design, index)
        }
        Op::EquipModules(args) => equip_modules(c, ctx.design, args),
        Op::OptimizeStation(args) => optimize_station(c, ctx.design, args, ctx.blueprints),
        Op::CopyEquipment { from, scope } => match scope.as_deref().unwrap_or("same_type") {
            "same_type" => Ok(json!({"modules": ctx.design.copy_equipment(from).map_err(err)?})),
            "all" => {
                let e = ctx.design.plan().entries.iter().find(|e| e.index == from).ok_or(format!("no entry {from}"))?;
                let mut picks: Vec<String> = e.upgrades.iter().map(|u| u.macro_name.clone()).collect();
                picks.dedup();
                if picks.is_empty() {
                    return Err(format!("module {from} has no equipment to copy"));
                }
                equip_modules(c, ctx.design, EquipArgs { picks: Some(picks), ..Default::default() })
            }
            other => Err(format!("scope is same_type or all, not {other}")),
        },
        Op::PlanProduction(args) => plan_production(c, ctx.design, args, None, ctx.blueprints),
        Op::AutoPlace(args) => auto_place(c, ctx.design, args, ctx.blueprints),
        Op::ReadBlueprints { save } => {
            let save = match save {
                Some(s) => s.into(),
                None => newest_save(ctx.profile_dir.ok_or("player profile not found; pass save")?).ok_or("no save in the profile's save folder; pass save")?,
            };
            *ctx.blueprints = Blueprints::read(c, &save)?;
            Ok(ctx.blueprints.status(ctx.names))
        }
        Op::BlueprintFilter { on } => {
            if on == Some(true) && ctx.blueprints.save.is_none() {
                return Err("no save read yet: read_blueprints first".to_owned());
            }
            if let Some(on) = on {
                ctx.blueprints.on = on;
            }
            Ok(ctx.blueprints.status(ctx.names))
        }
        Op::SaveDesign { file } => {
            let new_plan = ctx.profile_dir.is_some_and(|p| renew_id_if_renamed(p, ctx.design));
            std::fs::write(&file, write_plans(std::slice::from_ref(ctx.design.plan()))).map_err(|e| format!("{file}: {e}"))?;
            ctx.design.mark_saved();
            Ok(json!({"saved": file, "new_plan": new_plan}))
        }
        Op::ExportDesign { new_id, overwrite } => {
            let profile = ctx.profile_dir.ok_or("player profile not found")?;
            let new_plan = renew_id_if_renamed(profile, ctx.design);
            let mut plan = ctx.design.plan().clone();
            if new_id.unwrap_or(false) {
                plan.id = new_plan_id();
            }
            let path = write_import_file(profile, &plan, overwrite.unwrap_or(false)).map_err(err)?;
            ctx.design.mark_saved();
            Ok(json!({"file": path, "id": plan.id, "new_plan": new_plan, "next": "In game: station build menu > Import > select the plan"}))
        }
    }
}

/// The snap a moved module connects with by default: its current one, else its first.
fn default_snap(design: &Design, catalog: &Catalog, index: u32) -> Result<String, String> {
    let e = design.plan().entries.iter().find(|e| e.index == index).ok_or(format!("no entry {index}"))?;
    if let Some(c) = &e.connection {
        return Ok(c.clone());
    }
    let module = catalog.module(&e.macro_name).map_err(|e| e.to_string())?;
    module.snaps.first().map(|s| s.name.clone()).ok_or(format!("{} has no snaps", e.macro_name))
}

/// Resources for one entry (module + its equipment) or the whole plan, with an
/// estimated value at average prices and the build methods to choose from.
pub fn build_cost(catalog: &Catalog, design: &Design, index: Option<u32>, method: &str) -> Result<Value, String> {
    let wares = catalog.wares().map_err(|e| e.to_string())?;
    if let Some(i) = index {
        design.plan().entries.iter().find(|e| e.index == i).ok_or(format!("no entry {i}"))?;
    }
    let cost = wares.plan_cost(design.plan(), index, method);
    Ok(json!({
        "method": method,
        "methods": wares.methods().iter().map(|m| json!({"id": m.id, "name": m.name})).collect::<Vec<_>>(),
        "lines": cost.lines.iter().map(|l| json!({"ware": l.ware, "name": l.name, "amount": l.amount, "value": l.value})).collect::<Vec<_>>(),
        "value": cost.value,
        "unknown": cost.unknown,
    }))
}

/// The station's economy at `sunlight` percent: flows per hour (largest surplus
/// first), workforce, storage per cargo type, docks, and notes on what is missing.
pub fn analyze_station(catalog: &Catalog, design: &Design, sunlight: f64) -> Result<Value, String> {
    let a = analyze(catalog, design.plan(), sunlight / 100.0).map_err(|e| e.to_string())?;
    analysis_json(catalog, &a, sunlight)
}

/// What the station likely costs the game to run (`perf::load`; hypotheses, not measured).
pub fn station_load(catalog: &Catalog, design: &Design) -> Value {
    let l = perf::load(design.plan(), catalog);
    json!({
        "modules": l.modules,
        "connectors": l.connectors,
        "turrets": l.turrets,
        "shields": l.shields,
        "docks": l.docks,
        "size_km": l.size_km.map(|v| (v * 100.0).round() / 100.0),
        "unknown": l.unknown,
    })
}

/// Optimize (x4ep_planner's, plus a re-layout): adds the modules that fix the
/// station's shortfalls (or `modules`), then re-places its modules in layers
/// (without slots in the middle, defensible ones around them, docks, piers,
/// defence platforms and bays outside), as one undo step, and fits the plot to
/// the station. Nothing changes if anything finds no place.
pub fn optimize_station(catalog: &Catalog, design: &mut Design, args: OptimizeArgs, blueprints: &Blueprints) -> Result<Value, String> {
    let sunlight = args.plan.sunlight.unwrap_or(100.0);
    let mut req = request(args.plan, blueprints);
    req.targets.clear();
    req.modules.clear();
    let fixes: Vec<(String, u32)> = match args.modules {
        Some(list) => list.into_iter().filter(|m| m.count > 0).map(|m| (m.r#macro, m.count)).collect(),
        None => {
            let p = planner::plan_production(catalog, &module_counts(design.plan()), &req).map_err(|e| e.to_string())?;
            p.modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect()
        }
    };
    let options = OptimizeOptions {
        keep_shape: args.keep_shape.unwrap_or(true),
        grow_plot: true,
        race: req.race,
        fewer_connectors: args.fewer_connectors.unwrap_or(false),
        owned: req.owned,
        partial: args.partial.unwrap_or(false),
    };
    let r = layout::optimize(catalog, design, &fixes, &options).map_err(|e| e.to_string())?;
    design.fit_plot(catalog, false).map_err(|e| e.to_string())?;
    Ok(json!({
        "added": r.added.iter().map(|(m, n)| json!({"macro": m, "count": n})).collect::<Vec<_>>(),
        "moved": r.moved,
        "connectors_removed": r.connectors_removed,
        "connectors_added": r.placement.connectors.len(),
        "placed": r.placement.placed.len(),
        "unplaced": r.placement.unplaced.iter().map(|(m, n)| json!({"macro": m, "count": n})).collect::<Vec<_>>(),
        "broken_up": r.placement.broken_up,
        "docks_blocked": r.placement.docks_blocked,
        "plot": plot_status(design, catalog),
        "analysis": analyze_station(catalog, design, sunlight)?,
    }))
}

/// Turrets and shields players can build, filtered: [{macro, name, kind, size, race, mk, price}].
pub fn list_equipment(catalog: &Catalog, kind: Option<String>, size: Option<String>, race: Option<String>, search: Option<String>) -> Result<Value, String> {
    let wares = catalog.wares().map_err(|e| e.to_string())?;
    let search = search.map(|s| s.to_lowercase());
    let kind = kind.map(|k| if k.ends_with('s') { k } else { format!("{k}s") });
    let list: Vec<Value> = catalog
        .equipment()
        .map_err(|e| e.to_string())?
        .iter()
        .filter(|e| kind.as_ref().is_none_or(|k| e.kind.eq_ignore_ascii_case(k)))
        .filter(|e| size.as_ref().is_none_or(|s| e.size().is_some_and(|x| x.eq_ignore_ascii_case(s))))
        .filter(|e| race.as_ref().is_none_or(|r| e.race.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(r))))
        .filter(|e| search.as_ref().is_none_or(|s| e.name.to_lowercase().contains(s) || e.macro_name.contains(s)))
        .map(|e| {
            let price = wares.ware(&e.ware).and_then(|w| w.average_price);
            json!({"macro": e.macro_name, "name": e.name, "kind": e.kind, "size": e.size(), "race": e.race, "mk": e.mk, "price": price})
        })
        .collect();
    Ok(json!(list))
}

/// A module's equipment groups: per group and kind the slots, size, what is on
/// them and the macros that fit.
pub fn module_equipment(catalog: &Catalog, design: &Design, index: u32) -> Result<Value, String> {
    let e = design.plan().entries.iter().find(|e| e.index == index).ok_or(format!("no entry {index}"))?;
    let module = catalog.module(&e.macro_name).map_err(|e| e.to_string())?;
    let equipment = catalog.equipment().map_err(|e| e.to_string())?;
    let name = |m: &str| equipment.iter().find(|x| x.macro_name.eq_ignore_ascii_case(m)).map(|x| x.name.clone());
    let groups: Vec<Value> = module_groups(equipment, &module, &e.upgrades)
        .into_iter()
        .map(|g| {
            json!({
                "group": g.group,
                "kind": g.kind,
                "size": g.size,
                "slots": g.slots,
                "current": g.current.map(|(m, n)| json!({"macro": m, "name": name(&m), "count": n})),
                "fits": g.fitting.iter().map(|x| x.macro_name.as_str()).collect::<Vec<_>>(),
            })
        })
        .collect();
    Ok(json!({"index": index, "macro": e.macro_name, "groups": groups}))
}

/// Equips modules (picks or a loadout level; all, given, or defence modules only),
/// as one undo step: how many group slots were filled, cleared or skipped.
pub fn equip_modules(catalog: &Catalog, design: &mut Design, args: EquipArgs) -> Result<Value, String> {
    let fill = match (args.picks, args.level.as_deref()) {
        (Some(picks), None) => Fill::Picks(picks),
        (None, Some(level)) => Fill::Level(match level {
            "none" => Level::None,
            "low" => Level::Low,
            "medium" => Level::Medium,
            "high" => Level::High,
            other => return Err(format!("level is none, low, medium or high, not {other}")),
        }),
        _ => return Err("give either picks or level".to_owned()),
    };
    let defence_only = args.defence_only.unwrap_or(false);
    let indices: Vec<u32> = design
        .plan()
        .entries
        .iter()
        .filter(|e| args.modules.as_ref().is_none_or(|m| m.contains(&e.index)))
        .filter(|e| !defence_only || catalog.module(&e.macro_name).is_ok_and(|m| m.class == "defencemodule"))
        .map(|e| e.index)
        .collect();
    let (filled, cleared, skipped) = design.equip_modules(catalog, &indices, &fill).map_err(|e| e.to_string())?;
    Ok(json!({"filled": filled, "cleared": cleared, "skipped": skipped, "modules": indices.len()}))
}

/// Wares some buildable module makes: [{ware, name}], by name.
pub fn list_wares(catalog: &Catalog) -> Result<Value, String> {
    let wares = producible_wares(catalog).map_err(|e| e.to_string())?;
    Ok(json!(wares.into_iter().map(|(ware, name)| json!({"ware": ware, "name": name})).collect::<Vec<_>>()))
}

/// The modules to add to the design for the targets (production chain, habitats,
/// storage, docks), the station's analysis with them, and their build cost.
/// `existing`: plan for a station with these modules instead of the design's
/// (the builder re-plans against the station a plan was made for).
pub fn plan_production(catalog: &Catalog, design: &Design, args: PlanArgs, existing: Option<Vec<ModuleCount>>, blueprints: &Blueprints) -> Result<Value, String> {
    let sunlight = args.sunlight.unwrap_or(100.0);
    let existing = existing.map_or_else(|| module_counts(design.plan()), |m| m.into_iter().map(|m| (m.r#macro, m.count)).collect());
    let p = planner::plan_production(catalog, &existing, &request(args, blueprints)).map_err(|e| e.to_string())?;
    plan_json(catalog, &p, sunlight)
}

/// What the targets' modules make, before planning: per target ware, the module
/// the planner picks, its output per hour and the target's (its module count
/// times that), or `no_blueprint` when only modules without the player's blueprint make it.
pub fn target_rates(catalog: &Catalog, args: PlanArgs, blueprints: &Blueprints) -> Result<Value, String> {
    let req = request(args, blueprints);
    let rates = planner::target_rates(catalog, &req).map_err(|e| e.to_string())?;
    Ok(json!(rates
        .iter()
        .zip(&req.targets)
        .map(|(r, (_, modules))| match &r.producer {
            Some((m, name, per_module)) => json!({"ware": r.ware, "macro": m, "name": name, "per_module": per_module, "per_hour": per_module * f64::from(*modules)}),
            None => json!({"ware": r.ware, "no_blueprint": r.no_blueprint}),
        })
        .collect::<Vec<_>>()))
}

/// Places modules (a list, or what a plan lists) with `layout::auto_place`: the new
/// indices, the connection structures added, what didn't fit, and the plot.
pub fn auto_place(catalog: &Catalog, design: &mut Design, args: AutoPlaceArgs, blueprints: &Blueprints) -> Result<Value, String> {
    let (modules, race, plan, sides) = match (args.modules, args.from_plan) {
        (Some(list), None) => {
            let sides = sides(&list)?;
            (list.into_iter().map(|m| (m.r#macro, m.count)).collect::<Vec<_>>(), args.race, None, sides)
        }
        (None, Some(settings)) => {
            let sides = sides(settings.extra_modules.as_deref().unwrap_or_default())?;
            let sunlight = settings.sunlight.unwrap_or(100.0);
            let req = request(settings, blueprints);
            let p = planner::plan_production(catalog, &module_counts(design.plan()), &req).map_err(|e| e.to_string())?;
            let modules = p.modules.iter().map(|m| (m.macro_name.clone(), m.count)).collect();
            (modules, args.race.or(req.race), Some(plan_json(catalog, &p, sunlight)?), sides)
        }
        _ => return Err("give either modules or from_plan".to_owned()),
    };
    let options = Options {
        grow_plot: args.grow_plot.unwrap_or(true),
        race: race.filter(|r| !r.is_empty() && r != "any"),
        layered: false,
        fewer_connectors: args.fewer_connectors.unwrap_or(false),
        sides,
        owned: blueprints.owned(),
        break_up: args.break_up.unwrap_or(true),
        fill: false,
    };
    let r = layout::auto_place(catalog, design, &modules, &options).map_err(|e| e.to_string())?;
    let mut out = json!({
        "placed": r.placed.iter().map(|(i, m)| json!({"index": i, "macro": m})).collect::<Vec<_>>(),
        "connectors": r.connectors.iter().map(|(i, m)| json!({"index": i, "macro": m})).collect::<Vec<_>>(),
        "unplaced": r.unplaced.iter().map(|(m, n)| json!({"macro": m, "count": n})).collect::<Vec<_>>(),
        "docks_blocked": r.docks_blocked,
        "sides_blocked": r.sides_blocked,
        "broken_up": r.broken_up,
        "plot": plot_status(design, catalog),
    });
    if !out["plot"]["fits"].as_bool().unwrap_or(true) {
        out["next"] = json!("The station outgrew its plot: tell the user, then fit_plot with grow_only=true");
    }
    if let Some(p) = plan {
        out["plan"] = p;
    }
    Ok(out)
}

/// The sides asked for per module: (macro, sides).
fn sides(list: &[ModuleCount]) -> Result<Vec<(String, Vec<Side>)>, String> {
    let parse = |s: &String| Side::parse(s).ok_or_else(|| format!("unknown side {s}: use front, back, left, right, top or bottom"));
    list.iter()
        .filter_map(|m| m.sides.as_ref().filter(|s| !s.is_empty()).map(|s| Ok((m.r#macro.clone(), s.iter().map(parse).collect::<Result<_, _>>()?))))
        .collect()
}

fn request(args: PlanArgs, blueprints: &Blueprints) -> Request {
    Request {
        targets: args.targets.into_iter().map(|t| (t.ware, t.modules)).collect(),
        modules: args.extra_modules.unwrap_or_default().into_iter().map(|m| (m.r#macro, m.count)).collect(),
        storage: args.storage.unwrap_or_default().into_iter().collect(),
        race: args.race.filter(|r| !r.is_empty() && r != "any"),
        workforce: args.workforce.unwrap_or(true),
        storage_hours: args.storage_hours.unwrap_or(6.0),
        buy: args.buy.unwrap_or_default(),
        sunlight: args.sunlight.unwrap_or(100.0) / 100.0,
        method: args.method.unwrap_or_else(|| "default".to_owned()),
        owned: blueprints.owned(),
    }
}

fn plan_json(catalog: &Catalog, p: &ProductionPlan, sunlight: f64) -> Result<Value, String> {
    Ok(json!({
        "modules": p.modules.iter().map(|m| json!({"macro": m.macro_name, "name": m.name, "count": m.count, "role": m.role.as_str(), "ware": m.ware})).collect::<Vec<_>>(),
        "analysis": analysis_json(catalog, &p.analysis, sunlight)?,
        "cost": {
            "lines": p.cost.lines.iter().map(|l| json!({"ware": l.ware, "name": l.name, "amount": l.amount, "value": l.value})).collect::<Vec<_>>(),
            "value": p.cost.value,
            "unknown": p.cost.unknown,
        },
        "unknown_targets": p.unknown_targets,
        "no_blueprint": p.no_blueprint,
    }))
}

fn analysis_json(catalog: &Catalog, a: &Analysis, sunlight: f64) -> Result<Value, String> {
    let housed: u32 = a.housing.iter().map(|h| h.1).sum();
    let mut notes = Vec::new();
    if housed < a.workforce_needed {
        notes.push(format!(
            "Workforce: {housed} of {} housed, so the workforce bonus is at {:.0}%",
            a.workforce_needed,
            a.workforce_fill * 100.0
        ));
    }
    let wares = catalog.wares().map_err(|e| e.to_string())?;
    for s in a.storage.iter().filter(|s| s.capacity == 0.0 && s.volume_per_hour > 0.0) {
        let stored = a.flows.iter().filter(|f| wares.ware(&f.ware).is_some_and(|w| w.transport == s.transport));
        notes.push(format!("No {} storage, needed for {}", s.transport, stored.map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")));
    }
    if !a.flows.is_empty() && a.dock_areas + a.piers == 0 {
        notes.push("No dock area or pier: ships can't trade with the station".to_owned());
    }
    let short: Vec<&str> = a.flows.iter().filter(|f| !f.raw && f.net() < -1e-6).map(|f| f.name.as_str()).collect();
    if !short.is_empty() {
        notes.push(format!("Not self-sufficient: short of {} (make or buy them)", short.join(", ")));
    }
    if !a.unknown.is_empty() {
        notes.push(format!("Not counted (no production data): {}", a.unknown.join(", ")));
    }
    Ok(json!({
        "sunlight": sunlight,
        "flows": a.flows.iter().map(|f| json!({"ware": f.ware, "name": f.name, "produced": f.produced, "used": f.used, "net": f.net(), "raw": f.raw})).collect::<Vec<_>>(),
        "self_sufficient": a.self_sufficient(),
        "workforce": {
            "needed": a.workforce_needed,
            "housed": housed,
            "by_race": a.housing.iter().map(|(race, n)| json!({"race": race, "capacity": n})).collect::<Vec<_>>(),
            "fill": a.workforce_fill,
        },
        "storage": a.storage.iter().map(|s| json!({
            "transport": s.transport,
            "capacity": s.capacity,
            "volume_per_hour": s.volume_per_hour,
            "hours": (s.volume_per_hour > 0.0).then(|| s.capacity / s.volume_per_hour),
        })).collect::<Vec<_>>(),
        "dock_areas": a.dock_areas,
        "piers": a.piers,
        "unknown": a.unknown,
        "notes": notes,
    }))
}

/// After a reorder: the module's new index plus the renumbered design.
fn moved(ctx: &Ctx, moved_to: u32) -> Result<Value, String> {
    let mut out = describe(ctx)?;
    out["moved_to"] = json!(moved_to);
    Ok(out)
}

fn describe(ctx: &Ctx) -> Result<Value, String> {
    let design = &*ctx.design;
    let plan = design.plan();
    let modules: Vec<Value> = plan
        .entries
        .iter()
        .map(|e| {
            let p = design.world(e.index).map(|t| t.position).unwrap_or_default();
            let free: Vec<String> = design.free_snaps(ctx.catalog, e.index).map(|v| v.into_iter().map(|s| s.name).collect()).unwrap_or_default();
            json!({
                "index": e.index,
                "macro": e.macro_name,
                "name": ctx.names.get(&e.macro_name.to_ascii_lowercase()).map(|n| &n.0),
                "position": round3(p),
                "attached": e.predecessor.as_ref().map(|pr| json!({"to": pr.index, "to_snap": pr.connection, "snap": e.connection})),
                "stage_end": e.bookmark.is_some(),
                "free_snaps": free,
            })
        })
        .collect();
    let patches: Vec<&str> = plan.patches.iter().map(|p| p.extension.as_str()).collect();
    let mut out = json!({"id": plan.id, "name": plan.name, "patches": patches, "unsaved_changes": design.is_dirty(), "modules": modules});
    out["plot"] = plot_status(design, ctx.catalog);
    Ok(out)
}

/// Validation issues of the design, including the plot checks when it has a plot,
/// and, with the blueprint limit on, modules the player has no blueprint for.
pub fn issue_list(design: &Design, catalog: &Catalog, blueprints: &Blueprints, names: &ModuleNames) -> Vec<Issue> {
    let mut issues = validate(design.plan(), catalog);
    if let Some(plot) = design.plot() {
        issues.extend(validate_plot(design.plan(), catalog, &plot));
    }
    if let Some(owned) = blueprints.owned() {
        for e in design.plan().entries.iter().filter(|e| !owned.contains(&e.macro_name.to_ascii_lowercase())) {
            let name = names.get(&e.macro_name.to_ascii_lowercase()).map_or_else(|| e.macro_name.clone(), |n| n.0.clone());
            issues.push(Issue::NoBlueprint { index: e.index, name });
        }
    }
    issues
}

/// The plot, the plot the station needs, and whether it fits.
pub fn plot_status(design: &Design, catalog: &Catalog) -> Value {
    let required = required_plot(design.plan(), catalog);
    let plot = design.plot();
    json!({
        "plot": plot.map(plot_json),
        "required": required.map(plot_json),
        "fits": match (plot, required) { (Some(p), Some(r)) => p.contains(&r), _ => true },
        "possible": required.is_none_or(|r| r.is_allowed()),
    })
}

pub fn plot_json(p: Plot) -> Value {
    json!({"pos": p.pos, "neg": p.neg, "size": p.size(), "text": p.to_string()})
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
