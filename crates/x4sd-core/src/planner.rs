//! Production planner: from target wares to the modules a station needs (the
//! production chain, habitats, storage, a dock area and a pier), on top of what a
//! plan already has. Rules in tasks/workflow-production-planner.md.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::catalog::{Catalog, Module, ModuleInfo};
use crate::economy::{analyze_modules, module_flows, Analysis};
use crate::plan::Plan;
use crate::wares::Cost;
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Wares to make: (ware, production modules to add).
    pub targets: Vec<(String, u32)>,
    /// Preferred maker race of the modules; `None`: any.
    pub race: Option<String>,
    /// House the workforce: habitats, and the food and medicine they eat.
    pub workforce: bool,
    /// Storage for this many hours of everything the station makes and uses.
    pub storage_hours: f64,
    /// Wares to buy instead of making.
    pub buy: Vec<String>,
    /// Sector sunlight, 1.0 = 100%.
    pub sunlight: f64,
    /// Build method for the cost.
    pub method: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Target,
    Chain,
    Habitat,
    Storage,
    Dock,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Target => "target",
            Role::Chain => "chain",
            Role::Habitat => "habitat",
            Role::Storage => "storage",
            Role::Dock => "dock",
        }
    }
}

/// Modules of one macro to add.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    pub macro_name: String,
    pub name: String,
    pub count: u32,
    pub role: Role,
    /// The ware it makes, for production modules.
    pub ware: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProductionPlan {
    /// Modules to add, by role, then name.
    pub modules: Vec<Planned>,
    /// The station with them. What stays short has to be bought or mined.
    pub analysis: Analysis,
    /// Resources to build the added modules.
    pub cost: Cost,
    /// Target wares no buildable module makes.
    pub unknown_targets: Vec<String>,
}

/// A module players can build on their stations (build set `factory`).
struct Candidate {
    info: ModuleInfo,
    module: Arc<Module>,
    /// Maker race, else the race code in the macro name; `None` for generic modules.
    race: Option<String>,
}

/// Wares some buildable module makes: (ware id, name), by name.
pub fn producible_wares(catalog: &Catalog) -> Result<Vec<(String, String)>> {
    let wares = catalog.wares()?;
    let mut out: Vec<(String, String)> = candidates(catalog)?
        .iter()
        .filter_map(|c| producer_ware(c).map(str::to_owned))
        .map(|w| (w.clone(), wares.ware(&w).map_or_else(|| w.clone(), |x| x.name.clone())))
        .collect();
    out.sort_by(|a, b| a.1.cmp(&b.1));
    out.dedup();
    Ok(out)
}

/// The modules to add to `existing` for the request. With no targets it fills the
/// existing station's shortfalls.
pub fn plan_production(catalog: &Catalog, existing: &Plan, req: &Request) -> Result<ProductionPlan> {
    let wares = catalog.wares()?;
    let candidates = candidates(catalog)?;
    let race = req.race.as_deref();
    let producer = |ware: &str| best(candidates.iter().filter(|c| producer_ware(c) == Some(ware)), race, |_| 0.0);
    let habitat = best(candidates.iter().filter(|c| c.module.economy.housing.is_some()), race, |c| {
        c.module.economy.housing.as_ref().map_or(0.0, |h| f64::from(h.1))
    });
    let storage = |transport: &str| {
        best(
            candidates.iter().filter(|c| matches!(c.module.economy.cargo.as_slice(), [(_, tags)] if tags.len() == 1 && tags[0] == transport)),
            race,
            |c| c.module.economy.cargo[0].0,
        )
    };
    let dock = |class: &str| {
        best(
            candidates.iter().filter(|c| c.info.class == class && !["tradestation", "venturer"].iter().any(|k| c.info.macro_name.contains(k))),
            race,
            |_| 0.0,
        )
    };

    let mut added: BTreeMap<String, Planned> = BTreeMap::new();
    let mut unknown_targets = Vec::new();
    for (ware, count) in &req.targets {
        match producer(ware) {
            Some(c) => add(&mut added, c, *count, Role::Target),
            None => unknown_targets.push(ware.clone()),
        }
    }
    let analyze = |added: &BTreeMap<String, Planned>| {
        let existing = existing.entries.iter().map(|e| (e.macro_name.as_str(), 1));
        analyze_modules(catalog, existing.chain(added.values().map(|p| (p.macro_name.as_str(), p.count))), req.sunlight)
    };

    // Each pass houses the workforce, then covers every shortfall it can; new
    // modules need inputs and workers of their own, so repeat until nothing changes.
    for _ in 0..100 {
        let mut a = analyze(&added)?;
        let mut changed = false;
        if let Some(h) = habitat.filter(|_| req.workforce) {
            let housed: u32 = a.housing.iter().map(|h| h.1).sum();
            let capacity = h.module.economy.housing.as_ref().map_or(1, |h| h.1.max(1));
            if housed < a.workforce_needed {
                add(&mut added, h, (a.workforce_needed - housed).div_ceil(capacity), Role::Habitat);
                changed = true;
                a = analyze(&added)?;
            }
        }
        for f in a.flows.iter().filter(|f| f.net() < -1e-6 && !req.buy.contains(&f.ware)) {
            let Some(p) = producer(&f.ware) else { continue };
            let per_module: f64 = module_flows(wares, &p.module, a.workforce_fill, req.sunlight)
                .into_iter()
                .flatten()
                .filter(|(w, ..)| *w == f.ware)
                .map(|(_, produced, used)| produced - used)
                .sum();
            if per_module > 0.0 {
                add(&mut added, p, (-f.net() / per_module - 1e-9).ceil() as u32, Role::Chain);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }

    let a = analyze(&added)?;
    for s in &a.storage {
        let missing = s.volume_per_hour * req.storage_hours - s.capacity;
        if let Some(st) = storage(&s.transport).filter(|_| missing > 0.0) {
            add(&mut added, st, (missing / st.module.economy.cargo[0].0 - 1e-9).ceil() as u32, Role::Storage);
        }
    }
    if !a.flows.is_empty() {
        for (class, have) in [("dockarea", a.dock_areas), ("pier", a.piers)] {
            if let Some(d) = dock(class).filter(|_| have == 0) {
                add(&mut added, d, 1, Role::Dock);
            }
        }
    }

    let analysis = analyze(&added)?;
    let cost = wares.cost(added.values().map(|p| (p.macro_name.as_str(), f64::from(p.count))), &req.method);
    let mut modules: Vec<Planned> = added.into_values().collect();
    modules.sort_by(|a, b| (a.role, &a.name).cmp(&(b.role, &b.name)));
    Ok(ProductionPlan { modules, analysis, cost, unknown_targets })
}

/// Adds `count` modules of a candidate (a macro keeps the role it was first added with).
fn add(added: &mut BTreeMap<String, Planned>, c: &Candidate, count: u32, role: Role) {
    let p = added.entry(c.info.macro_name.clone()).or_insert_with(|| Planned {
        macro_name: c.info.macro_name.clone(),
        name: c.info.name.clone(),
        count: 0,
        role,
        ware: producer_ware(c).map(str::to_owned),
    });
    p.count += count;
}

fn candidates(catalog: &Catalog) -> Result<Vec<Candidate>> {
    Ok(catalog
        .station_modules()?
        .into_iter()
        .filter(|i| i.sets.iter().any(|s| s == "factory"))
        .filter_map(|info| {
            let module = catalog.module(&info.macro_name).ok()?;
            let race = info.race.clone().or_else(|| name_race(&info.macro_name));
            Some(Candidate { info, module, race })
        })
        .collect())
}

/// The ware of a production module with a single, regular (not recycling) recipe.
fn producer_ware(c: &Candidate) -> Option<&str> {
    match c.module.economy.production.as_slice() {
        [(ware, method)] if c.info.class == "production" && method != "recycling" => Some(ware),
        _ => None,
    }
}

/// The best candidate: the chosen race's, then generic ones (or, with any race,
/// all alike), then other races'; then the largest `size`; then the shortest macro
/// name (the plain variant, e.g. a standard dock area before a luxury one); then by name.
fn best<'a>(candidates: impl Iterator<Item = &'a Candidate>, race: Option<&str>, size: impl Fn(&Candidate) -> f64) -> Option<&'a Candidate> {
    let rank = |c: &Candidate| match (race, c.race.as_deref()) {
        (Some(r), Some(x)) if r == x => 0,
        (None, _) | (_, None) => 1,
        _ => 2,
    };
    candidates.min_by(|a, b| {
        let name = |c: &Candidate| (c.info.macro_name.len(), c.info.macro_name.clone());
        rank(a).cmp(&rank(b)).then(size(b).total_cmp(&size(a))).then(name(a).cmp(&name(b)))
    })
}

/// Race code in a macro name such as `dockarea_arg_m_station_01_macro`.
fn name_race(macro_name: &str) -> Option<String> {
    let race = match macro_name.split('_').nth(1)? {
        "arg" => "argon",
        "bor" => "boron",
        "par" => "paranid",
        "spl" => "split",
        "tel" => "teladi",
        "ter" => "terran",
        _ => return None,
    };
    Some(race.to_owned())
}
