//! Production planner: from target wares to the modules a station needs (the
//! production chain, habitats, storage, a dock area and a pier), on top of what a
//! plan already has. Rules in tasks/workflow-production-planner.md.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use crate::catalog::{Catalog, Module, ModuleInfo};
use crate::economy::{analyze_modules, module_flows, Analysis};
use crate::plan::Plan;
use crate::wares::Cost;
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Request {
    /// Wares to make: (ware, production modules to add).
    pub targets: Vec<(String, u32)>,
    /// Other modules to add: (macro, count), e.g. ship fabrication bays or defence platforms.
    pub modules: Vec<(String, u32)>,
    /// Preferred maker race of the modules; `None`: any.
    pub race: Option<String>,
    /// House the workforce: habitats, and the food and medicine they eat.
    pub workforce: bool,
    /// Hours of production and consumption the storage should hold.
    pub storage_hours: f64,
    /// At least this much storage per cargo type: (transport, m³), e.g. ("liquid", 200000.0).
    pub storage: Vec<(String, f64)>,
    /// Wares to buy instead of making.
    pub buy: Vec<String>,
    /// Sector sunlight, 1.0 = 100%.
    pub sunlight: f64,
    /// Build method for the cost.
    pub method: String,
    /// The station modules the player has blueprints for (lowercase macros);
    /// `None`: all. The planner only chooses these (`extra` modules are used as given).
    pub owned: Option<HashSet<String>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    Target,
    Extra,
    Chain,
    Habitat,
    Storage,
    Dock,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Target => "target",
            Role::Extra => "extra",
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
    /// Wares (targets, or inputs the station is short of) that only modules
    /// without the player's blueprint make.
    pub no_blueprint: Vec<String>,
}

/// A module players can build on their stations (build set `factory`).
pub(crate) struct Candidate {
    pub(crate) info: ModuleInfo,
    pub(crate) module: Arc<Module>,
    /// Maker race, else the race code in the macro name; `None` for generic modules.
    pub(crate) race: Option<String>,
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

/// The modules a plan has: (macro, count), in first-seen order.
pub fn module_counts(plan: &Plan) -> Vec<(String, u32)> {
    let mut out: Vec<(String, u32)> = Vec::new();
    for e in &plan.entries {
        match out.iter_mut().find(|(m, _)| m.eq_ignore_ascii_case(&e.macro_name)) {
            Some((_, n)) => *n += 1,
            None => out.push((e.macro_name.clone(), 1)),
        }
    }
    out
}

/// The modules to add to a station that has `existing` modules (macro, count;
/// see `module_counts`) for the request. With no targets it fills the existing
/// station's shortfalls.
pub fn plan_production(catalog: &Catalog, existing: &[(String, u32)], req: &Request) -> Result<ProductionPlan> {
    let wares = catalog.wares()?;
    let all = candidates(catalog)?;
    let candidates: Vec<&Candidate> = all.iter().filter(|c| owns(req.owned.as_ref(), &c.info.macro_name)).collect();
    let candidates = || candidates.iter().copied();
    let race = req.race.as_deref();
    let producer = |ware: &str| best(candidates().filter(|c| producer_ware(c) == Some(ware)), race, |_| 0.0);
    let anyone_makes = |ware: &str| all.iter().any(|c| producer_ware(c) == Some(ware));
    let habitat = best(candidates().filter(|c| c.module.economy.housing.is_some()), race, |c| {
        c.module.economy.housing.as_ref().map_or(0.0, |h| f64::from(h.1))
    });
    let stores = |transport: String| {
        candidates().filter(move |c| matches!(c.module.economy.cargo.as_slice(), [(_, tags)] if tags.len() == 1 && tags[0] == transport))
    };
    let capacity = |c: &Candidate| c.module.economy.cargo[0].0;
    let dock = |class: &str| {
        best(
            candidates().filter(|c| c.info.class == class && !["tradestation", "venturer"].iter().any(|k| c.info.macro_name.contains(k))),
            race,
            |_| 0.0,
        )
    };

    let mut added: BTreeMap<String, Planned> = BTreeMap::new();
    let mut unknown_targets = Vec::new();
    let mut no_blueprint = Vec::new();
    for (ware, count) in &req.targets {
        match producer(ware) {
            Some(c) => add(&mut added, c, *count, Role::Target),
            None if anyone_makes(ware) => no_blueprint.push(ware.clone()),
            None => unknown_targets.push(ware.clone()),
        }
    }
    for (macro_name, count) in &req.modules {
        match all.iter().find(|c| c.info.macro_name.eq_ignore_ascii_case(macro_name)) {
            Some(c) => add(&mut added, c, *count, Role::Extra),
            None => {
                let info = catalog
                    .station_modules()?
                    .into_iter()
                    .find(|i| i.macro_name.eq_ignore_ascii_case(macro_name))
                    .ok_or_else(|| Error(format!("not a station module: {macro_name}")))?;
                let module = catalog.module(&info.macro_name)?;
                add(&mut added, &Candidate { race: info.race.clone(), info, module }, *count, Role::Extra);
            }
        }
    }
    let analyze = |added: &BTreeMap<String, Planned>| {
        let existing = existing.iter().map(|(m, n)| (m.as_str(), *n));
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
            let Some(p) = producer(&f.ware) else {
                if anyone_makes(&f.ware) && !no_blueprint.contains(&f.ware) {
                    no_blueprint.push(f.ware.clone());
                }
                continue;
            };
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

    // Storage per type: the larger of the hours rule and the minimum asked for. The
    // largest module covers the bulk, the smallest that covers it the rest.
    let a = analyze(&added)?;
    let mut needs: Vec<(String, f64, f64)> = a.storage.iter().map(|s| (s.transport.clone(), s.volume_per_hour * req.storage_hours, s.capacity)).collect();
    for (transport, min) in &req.storage {
        match needs.iter_mut().find(|n| n.0 == *transport) {
            Some(n) => n.1 = n.1.max(*min),
            None => needs.push((transport.clone(), *min, 0.0)),
        }
    }
    for (transport, need, have) in needs {
        let missing = need - have;
        let Some(big) = best(stores(transport.clone()), race, capacity).filter(|_| missing > 0.5) else { continue };
        let bulk = (missing / capacity(big)).floor();
        if bulk > 0.0 {
            add(&mut added, big, bulk as u32, Role::Storage);
        }
        let rest = missing - bulk * capacity(big);
        let small = stores(transport)
            .filter(|c| c.race == big.race && capacity(c) >= rest)
            .min_by(|a, b| capacity(a).total_cmp(&capacity(b)).then(a.info.macro_name.cmp(&b.info.macro_name)));
        if let Some(small) = small.filter(|_| rest > 0.5) {
            add(&mut added, small, 1, Role::Storage);
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
    Ok(ProductionPlan { modules, analysis, cost, unknown_targets, no_blueprint })
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

pub(crate) fn candidates(catalog: &Catalog) -> Result<Vec<Candidate>> {
    Ok(catalog
        .station_modules()?
        .into_iter()
        .filter(|i| i.sets.iter().any(|s| s == "factory"))
        .filter_map(|info| {
            let module = catalog.module(&info.macro_name).ok()?;
            Some(Candidate { race: info.race.clone(), info, module })
        })
        .collect())
}

/// Whether the player has the blueprint of a module (`owned`: lowercase macros; `None`: all).
pub(crate) fn owns(owned: Option<&HashSet<String>>, macro_name: &str) -> bool {
    owned.is_none_or(|o| o.contains(&macro_name.to_ascii_lowercase()))
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
pub(crate) fn best<'a>(candidates: impl Iterator<Item = &'a Candidate>, race: Option<&str>, size: impl Fn(&Candidate) -> f64) -> Option<&'a Candidate> {
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
