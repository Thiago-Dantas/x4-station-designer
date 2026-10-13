//! Station economy: what a plan makes and uses per hour, its workforce and its
//! storage. The rules and where they come from are in
//! tasks/workflow-production-planner.md; the maths are hypotheses until checked
//! against the game's station overview.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::catalog::{Catalog, Module};
use crate::plan::Plan;
use crate::Result;

/// The ware whose recipe per race says what workers eat.
const WORKFORCE_WARE: &str = "workunit_busy";

/// One ware's flow, per hour.
#[derive(Debug, Clone, PartialEq)]
pub struct Flow {
    pub ware: String,
    pub name: String,
    pub produced: f64,
    pub used: f64,
}

impl Flow {
    pub fn net(&self) -> f64 {
        self.produced - self.used
    }
}

/// Cargo storage of one type.
#[derive(Debug, Clone, PartialEq)]
pub struct Storage {
    /// `container`, `solid`, `liquid`, ...
    pub transport: String,
    /// m³.
    pub capacity: f64,
    /// m³ per hour of wares stored this way that the station makes and uses.
    pub volume_per_hour: f64,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Analysis {
    /// Sorted by net flow, largest surplus first.
    pub flows: Vec<Flow>,
    /// Workers the modules need at full staff.
    pub workforce_needed: u32,
    /// Workers the habitats house, per race (assumed full).
    pub housing: Vec<(String, u32)>,
    /// Share of the needed workforce that is housed, 0..1. The work bonus scales with it.
    pub workforce_fill: f64,
    pub storage: Vec<Storage>,
    pub dock_areas: u32,
    pub piers: u32,
    /// Modules whose macro or production data is missing (left out).
    pub unknown: Vec<String>,
}

/// Analyses a plan in a sector with `sunlight` (1.0 = 100%; solar output follows it).
pub fn analyze(catalog: &Catalog, plan: &Plan, sunlight: f64) -> Result<Analysis> {
    let wares = catalog.wares()?;
    let mut out = Analysis::default();
    let mut housing: BTreeMap<String, u32> = BTreeMap::new();
    let mut cargo: BTreeMap<String, f64> = BTreeMap::new();
    let mut producers: Vec<Arc<Module>> = Vec::new();
    for e in &plan.entries {
        let Ok(m) = catalog.module(&e.macro_name) else {
            out.unknown.push(e.macro_name.clone());
            continue;
        };
        out.workforce_needed += m.economy.workforce;
        if let Some((race, capacity)) = &m.economy.housing {
            *housing.entry(race.clone()).or_default() += capacity;
        }
        for (max, tags) in &m.economy.cargo {
            tags.iter().for_each(|t| *cargo.entry(t.clone()).or_default() += max);
        }
        match m.class.as_str() {
            "dockarea" => out.dock_areas += 1,
            "pier" => out.piers += 1,
            _ => {}
        }
        if !m.economy.production.is_empty() {
            producers.push(m);
        }
    }
    let housed: u32 = housing.values().sum();
    out.workforce_fill = if out.workforce_needed == 0 { 1.0 } else { (f64::from(housed) / f64::from(out.workforce_needed)).min(1.0) };

    // (produced, used) per ware and hour.
    let mut flows: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for m in &producers {
        let recipes: Option<Vec<_>> =
            m.economy.production.iter().map(|(ware, method)| Some((ware, wares.recipe(wares.ware(ware)?, method)?))).collect();
        let cycle: f64 = recipes.iter().flatten().map(|(_, r)| r.time).sum();
        let (Some(recipes), true) = (recipes, cycle > 0.0) else {
            out.unknown.push(m.macro_name.clone());
            continue;
        };
        // Queue items run in turn; the work and sunlight bonuses add output, not inputs.
        let cycles_per_hour = 3600.0 / cycle;
        for (ware, r) in recipes {
            let bonus = (1.0 + r.work * out.workforce_fill) * (1.0 + (sunlight - 1.0) * r.sunlight);
            flows.entry(ware.clone()).or_default().0 += r.amount * cycles_per_hour * bonus;
            for (input, amount) in &r.resources {
                flows.entry(input.clone()).or_default().1 += amount * cycles_per_hour;
            }
        }
    }
    // Workers eat their race's upkeep recipe: its resources feed `amount` workers per cycle.
    if let Some(upkeep) = wares.ware(WORKFORCE_WARE) {
        for (race, workers) in &housing {
            let Some(r) = wares.recipe(upkeep, race).filter(|r| r.amount > 0.0 && r.time > 0.0) else { continue };
            for (input, amount) in &r.resources {
                flows.entry(input.clone()).or_default().1 += amount * f64::from(*workers) / r.amount * 3600.0 / r.time;
            }
        }
    }

    let mut volume: BTreeMap<String, f64> = cargo.keys().map(|t| (t.clone(), 0.0)).collect();
    for (ware, (produced, used)) in &flows {
        if let Some(w) = wares.ware(ware).filter(|w| !w.transport.is_empty()) {
            *volume.entry(w.transport.clone()).or_default() += (produced + used) * w.volume;
        }
    }
    out.storage = volume
        .into_iter()
        .map(|(transport, volume_per_hour)| Storage { capacity: cargo.get(&transport).copied().unwrap_or(0.0), transport, volume_per_hour })
        .collect();
    out.flows = flows
        .into_iter()
        .map(|(ware, (produced, used))| Flow { name: wares.ware(&ware).map_or_else(|| ware.clone(), |w| w.name.clone()), ware, produced, used })
        .collect();
    out.flows.sort_by(|a, b| b.net().total_cmp(&a.net()));
    out.housing = housing.into_iter().collect();
    out.unknown.sort();
    out.unknown.dedup();
    Ok(out)
}
