//! What a station likely costs the game to run (frame rate): the counts players
//! name as the load. All **hypotheses**, nothing measured in game; see
//! tasks/workflow-fps-load.md.

use crate::catalog::Catalog;
use crate::equipment::count;
use crate::plan::Plan;
use crate::plot::entry_bounds;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Load {
    /// Every module, connection structures included: each is an object the game
    /// draws and checks for collisions.
    pub modules: u32,
    /// Connection structures: modules that make nothing.
    pub connectors: u32,
    /// Turrets on the station: each one aims and fires on its own.
    pub turrets: u32,
    pub shields: u32,
    /// Dock areas and piers: the traffic ships bring.
    pub docks: u32,
    /// Size of the station (part-box bounds) per axis, in km; zero for an empty plan.
    pub size_km: [f64; 3],
    /// Modules not in the catalogue (counted as modules only).
    pub unknown: u32,
}

/// The load of a plan.
pub fn load(plan: &Plan, catalog: &Catalog) -> Load {
    let mut out = Load { modules: plan.entries.len() as u32, ..Default::default() };
    let mut all: Option<([f64; 3], [f64; 3])> = None;
    for e in &plan.entries {
        for u in &e.upgrades {
            match u.kind.as_str() {
                "turrets" => out.turrets += count(u),
                "shields" => out.shields += count(u),
                _ => {}
            }
        }
        let Ok(m) = catalog.module(&e.macro_name) else {
            out.unknown += 1;
            continue;
        };
        match m.class.as_str() {
            "connectionmodule" => out.connectors += 1,
            "dockarea" | "pier" => out.docks += 1,
            _ => {}
        }
        if let Some((lo, hi)) = entry_bounds(plan, catalog, e.index) {
            let (alo, ahi) = all.get_or_insert((lo, hi));
            for i in 0..3 {
                alo[i] = alo[i].min(lo[i]);
                ahi[i] = ahi[i].max(hi[i]);
            }
        }
    }
    if let Some((lo, hi)) = all {
        out.size_km = [0, 1, 2].map(|i| (hi[i] - lo[i]) / 1000.0);
    }
    out
}
