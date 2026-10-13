//! The design operations agents use (D-7). `x4sd mcp` runs them on its own
//! design, or sends them to a running builder (`link.rs`), which runs them on
//! the design open there. Either way they end up here.

use std::collections::HashMap;
use std::path::Path;

use rmcp::schemars;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use x4sd_core::catalog::Catalog;
use x4sd_core::design::Design;
use x4sd_core::economy::{analyze, Analysis};
use x4sd_core::geometry::{Quat, Transform};
use x4sd_core::import::{new_plan_id, write_import_file};
use x4sd_core::plan::{parse_plans, write_plans};
use x4sd_core::planner::{self, producible_wares, Request};
use x4sd_core::plot::{required_plot, Plot};
use x4sd_core::validate::{validate, validate_plot};

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
    /// Preferred race of the modules: argon, boron, paranid, split, teladi, terran; omit for any
    pub race: Option<String>,
    /// Add habitats for the workforce and make their food and medicine (default true)
    pub workforce: Option<bool>,
    /// Storage for this many hours of everything the station makes and uses (default 6)
    pub storage_hours: Option<f64>,
    /// Ware ids to buy instead of making
    pub buy: Option<Vec<String>>,
    /// The sector's sunlight in percent (default 100)
    pub sunlight: Option<f64>,
    /// Build method for the cost (default: default)
    pub method: Option<String>,
}

/// Lowercase macro -> (display name, class) of every station module.
pub type ModuleNames = HashMap<String, (String, String)>;

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
    /// Wares some buildable module makes.
    ListWares,
    /// The modules to add for target wares (or to fix the station's shortfalls).
    PlanProduction(PlanArgs),
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
                | Op::ListWares
                | Op::PlanProduction(_)
        )
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
            let modules: Vec<Value> = c
                .station_modules()
                .map_err(err)?
                .into_iter()
                .filter(|m| class.as_ref().is_none_or(|x| m.class.eq_ignore_ascii_case(x)))
                .filter(|m| race.as_ref().is_none_or(|r| m.race.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(r))))
                .filter(|m| search.as_ref().is_none_or(|s| m.name.to_lowercase().contains(s) || m.macro_name.to_lowercase().contains(s)))
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
        Op::ValidateDesign => Ok(json!({"issues": issues(ctx.design, c)})),
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
        Op::ListWares => list_wares(c),
        Op::PlanProduction(args) => plan_production(c, ctx.design, args),
        Op::SaveDesign { file } => {
            std::fs::write(&file, write_plans(std::slice::from_ref(ctx.design.plan()))).map_err(|e| format!("{file}: {e}"))?;
            ctx.design.mark_saved();
            Ok(json!({"saved": file}))
        }
        Op::ExportDesign { new_id, overwrite } => {
            let profile = ctx.profile_dir.ok_or("player profile not found")?;
            let mut plan = ctx.design.plan().clone();
            if new_id.unwrap_or(false) {
                plan.id = new_plan_id();
            }
            let path = write_import_file(profile, &plan, overwrite.unwrap_or(false)).map_err(err)?;
            ctx.design.mark_saved();
            Ok(json!({"file": path, "id": plan.id, "next": "In game: station build menu > Import > select the plan"}))
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

/// Wares some buildable module makes: [{ware, name}], by name.
pub fn list_wares(catalog: &Catalog) -> Result<Value, String> {
    let wares = producible_wares(catalog).map_err(|e| e.to_string())?;
    Ok(json!(wares.into_iter().map(|(ware, name)| json!({"ware": ware, "name": name})).collect::<Vec<_>>()))
}

/// The modules to add to the design for the targets (production chain, habitats,
/// storage, docks), the station's analysis with them, and their build cost.
pub fn plan_production(catalog: &Catalog, design: &Design, args: PlanArgs) -> Result<Value, String> {
    let sunlight = args.sunlight.unwrap_or(100.0);
    let req = Request {
        targets: args.targets.into_iter().map(|t| (t.ware, t.modules)).collect(),
        race: args.race.filter(|r| !r.is_empty() && r != "any"),
        workforce: args.workforce.unwrap_or(true),
        storage_hours: args.storage_hours.unwrap_or(6.0),
        buy: args.buy.unwrap_or_default(),
        sunlight: sunlight / 100.0,
        method: args.method.unwrap_or_else(|| "default".to_owned()),
    };
    let p = planner::plan_production(catalog, design.plan(), &req).map_err(|e| e.to_string())?;
    Ok(json!({
        "modules": p.modules.iter().map(|m| json!({"macro": m.macro_name, "name": m.name, "count": m.count, "role": m.role.as_str(), "ware": m.ware})).collect::<Vec<_>>(),
        "analysis": analysis_json(catalog, &p.analysis, sunlight)?,
        "cost": {
            "lines": p.cost.lines.iter().map(|l| json!({"ware": l.ware, "name": l.name, "amount": l.amount, "value": l.value})).collect::<Vec<_>>(),
            "value": p.cost.value,
            "unknown": p.cost.unknown,
        },
        "unknown_targets": p.unknown_targets,
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
    if !a.unknown.is_empty() {
        notes.push(format!("Not counted (no production data): {}", a.unknown.join(", ")));
    }
    Ok(json!({
        "sunlight": sunlight,
        "flows": a.flows.iter().map(|f| json!({"ware": f.ware, "name": f.name, "produced": f.produced, "used": f.used, "net": f.net()})).collect::<Vec<_>>(),
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

/// Validation issues of the design, including the plot checks when it has a plot.
pub fn issues(design: &Design, catalog: &Catalog) -> Vec<String> {
    let mut issues = validate(design.plan(), catalog);
    if let Some(plot) = design.plot() {
        issues.extend(validate_plot(design.plan(), catalog, &plot));
    }
    issues.iter().map(|i| i.to_string()).collect()
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
