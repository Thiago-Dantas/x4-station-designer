//! Plan checks the game itself does not do: it accepts plans whose modules float
//! away from their links (verified in game, test C) and plans without the
//! `<patches>` they need.

use std::collections::HashMap;
use std::fmt;

use crate::catalog::Catalog;
use crate::geometry::{fit, Transform};
use crate::plan::{Entry, Plan};

/// Allowed distance between linked snap points. The game rounds positions to
/// millimetres, and its own hand-made plans are off by up to 3.5 cm
/// (`arg_wharf`); real misplacements are hundreds of metres.
pub const GAP_TOLERANCE_M: f64 = 0.05;
/// Allowed angle between linked snap axes.
pub const ANGLE_TOLERANCE_DEG: f64 = 0.5;

#[derive(Debug, Clone, PartialEq)]
pub enum Issue {
    DuplicateIndex { index: u32 },
    UnknownMacro { index: u32, macro_name: String },
    UnknownUpgradeMacro { index: u32, macro_name: String },
    UnknownSnap { index: u32, macro_name: String, snap: String },
    MissingPredecessor { index: u32, predecessor: u32 },
    SnapGap { index: u32, metres: f64 },
    SnapMisaligned { index: u32, degrees: f64 },
    MissingPatch { extension: String },
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Issue::DuplicateIndex { index } => write!(f, "entry {index}: index used more than once"),
            Issue::UnknownMacro { index, macro_name } => write!(f, "entry {index}: unknown module {macro_name}"),
            Issue::UnknownUpgradeMacro { index, macro_name } => write!(f, "entry {index}: unknown equipment {macro_name}"),
            Issue::UnknownSnap { index, macro_name, snap } => write!(f, "entry {index}: {macro_name} has no snap {snap}"),
            Issue::MissingPredecessor { index, predecessor } => write!(f, "entry {index}: predecessor {predecessor} does not exist"),
            Issue::SnapGap { index, metres } => write!(f, "entry {index}: {metres:.3} m away from its linked snap"),
            Issue::SnapMisaligned { index, degrees } => write!(f, "entry {index}: snap axis off by {degrees:.2}°"),
            Issue::MissingPatch { extension } => write!(f, "plan uses {extension} but does not list it in <patches>"),
        }
    }
}

pub fn validate(plan: &Plan, catalog: &Catalog) -> Vec<Issue> {
    let mut issues = Vec::new();
    let mut by_index: HashMap<u32, &Entry> = HashMap::new();
    for e in &plan.entries {
        if by_index.insert(e.index, e).is_some() {
            issues.push(Issue::DuplicateIndex { index: e.index });
        }
    }

    for e in &plan.entries {
        let module = match catalog.module(&e.macro_name) {
            Ok(m) => m,
            Err(_) => {
                issues.push(Issue::UnknownMacro { index: e.index, macro_name: e.macro_name.clone() });
                continue;
            }
        };
        for u in e.upgrades.iter().filter(|u| !catalog.has_macro(&u.macro_name)) {
            issues.push(Issue::UnknownUpgradeMacro { index: e.index, macro_name: u.macro_name.clone() });
        }
        let (Some(conn), Some(pred)) = (&e.connection, &e.predecessor) else { continue };
        let Some(child_snap) = module.snap(conn) else {
            issues.push(Issue::UnknownSnap { index: e.index, macro_name: e.macro_name.clone(), snap: conn.clone() });
            continue;
        };
        let Some(pred_entry) = by_index.get(&pred.index) else {
            issues.push(Issue::MissingPredecessor { index: e.index, predecessor: pred.index });
            continue;
        };
        // An unknown predecessor macro is reported on its own entry.
        let Ok(pred_module) = catalog.module(&pred_entry.macro_name) else { continue };
        let Some(pred_snap) = pred_module.snap(&pred.connection) else {
            issues.push(Issue::UnknownSnap {
                index: e.index,
                macro_name: pred_entry.macro_name.clone(),
                snap: pred.connection.clone(),
            });
            continue;
        };
        let f = fit(
            &Transform::from_offset(pred_entry.offset.as_ref()),
            &pred_snap.transform,
            &Transform::from_offset(e.offset.as_ref()),
            &child_snap.transform,
        );
        if f.gap > GAP_TOLERANCE_M {
            issues.push(Issue::SnapGap { index: e.index, metres: f.gap });
        } else if f.misalignment > ANGLE_TOLERANCE_DEG {
            issues.push(Issue::SnapMisaligned { index: e.index, degrees: f.misalignment });
        }
    }

    for needed in catalog.required_patches(plan) {
        if !plan.patches.iter().any(|p| p.extension == needed.extension) {
            issues.push(Issue::MissingPatch { extension: needed.extension });
        }
    }
    issues
}
