//! The design operations agents use (D-7). `x4sd mcp` runs them on its own
//! design, or sends them to a running builder (`link.rs`), which runs them on
//! the design open there. Either way they end up here.

use std::collections::HashMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use x4sd_core::catalog::Catalog;
use x4sd_core::design::Design;
use x4sd_core::geometry::{Quat, Transform};
use x4sd_core::import::{new_plan_id, write_import_file};
use x4sd_core::plan::{parse_plans, write_plans};
use x4sd_core::validate::validate;

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
}

impl Op {
    /// Whether the open design may change (the builder then redraws).
    pub fn changes_design(&self) -> bool {
        !matches!(
            self,
            Op::ListModules { .. } | Op::ModuleSnaps { .. } | Op::DescribeDesign | Op::FreeSnaps { .. }
                | Op::ValidateDesign | Op::SaveDesign { .. } | Op::ExportDesign { .. }
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
        Op::ValidateDesign => {
            let issues: Vec<String> = validate(ctx.design.plan(), c).iter().map(|i| i.to_string()).collect();
            Ok(json!({"issues": issues}))
        }
        Op::SaveDesign { file } => {
            std::fs::write(&file, write_plans(std::slice::from_ref(ctx.design.plan()))).map_err(|e| format!("{file}: {e}"))?;
            Ok(json!({"saved": file}))
        }
        Op::ExportDesign { new_id, overwrite } => {
            let profile = ctx.profile_dir.ok_or("player profile not found")?;
            let mut plan = ctx.design.plan().clone();
            if new_id.unwrap_or(false) {
                plan.id = new_plan_id();
            }
            let path = write_import_file(profile, &plan, overwrite.unwrap_or(false)).map_err(err)?;
            Ok(json!({"file": path, "id": plan.id, "next": "In game: station build menu > Import > select the plan"}))
        }
    }
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
                "free_snaps": free,
            })
        })
        .collect();
    let patches: Vec<&str> = plan.patches.iter().map(|p| p.extension.as_str()).collect();
    Ok(json!({"id": plan.id, "name": plan.name, "patches": patches, "modules": modules}))
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
