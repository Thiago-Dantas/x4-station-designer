//! Plan checks the game itself does not do: it accepts plans whose modules float
//! away from their links (verified in game, test C), plans without the
//! `<patches>` they need, (hypothesis) modules that overlap, and (verified on
//! import, 2026-10-14) modules in the way of a pier's approach or a bay's exit.

use std::collections::HashMap;
use std::fmt;

use crate::catalog::Catalog;
use crate::equipment::count;
use crate::geometry::{fit, Transform};
use crate::layout::{blocked_zones, overlaps};
use crate::plan::{Entry, Plan};
use crate::plot::{entry_inside, required_plot, Plot, MAX_AXIS_KM};

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
    /// Equipment for a group (or kind of slot) the module doesn't have.
    EquipmentNoSlots { index: u32, group: String, kind: String },
    /// Equipment that doesn't fit its group's slots.
    EquipmentMisfit { index: u32, group: String, macro_name: String },
    /// More equipment than the group has slots of that kind.
    EquipmentTooMany { index: u32, group: String, kind: String, count: u32, slots: u32 },
    UnknownSnap { index: u32, macro_name: String, snap: String },
    MissingPredecessor { index: u32, predecessor: u32 },
    SnapGap { index: u32, metres: f64 },
    SnapMisaligned { index: u32, degrees: f64 },
    MissingPatch { extension: String },
    /// The module's collision mesh intersects that of an earlier module it isn't
    /// joined to at a snap.
    Overlap { index: u32, other: u32 },
    /// The module reaches into an exclusion zone of `owner` (`layout::blocked_zones`),
    /// whose macro class (`pier`, `buildmodule`, `dockarea`…) is `owner_class`.
    BlocksZone { index: u32, owner: u32, owner_class: String },
    /// The module reaches outside the plot.
    OutsidePlot { index: u32 },
    /// The station needs a bigger plot than it has.
    PlotTooSmall { plot: Plot, required: Plot },
    /// The station is longer along an axis than any plot can be.
    TooBigForAnyPlot { axis: char, km: u32 },
}

impl Issue {
    /// The entry the issue is about; `None` for plan-wide issues.
    pub fn index(&self) -> Option<u32> {
        match self {
            Issue::DuplicateIndex { index }
            | Issue::UnknownMacro { index, .. }
            | Issue::UnknownUpgradeMacro { index, .. }
            | Issue::EquipmentNoSlots { index, .. }
            | Issue::EquipmentMisfit { index, .. }
            | Issue::EquipmentTooMany { index, .. }
            | Issue::UnknownSnap { index, .. }
            | Issue::MissingPredecessor { index, .. }
            | Issue::SnapGap { index, .. }
            | Issue::SnapMisaligned { index, .. }
            | Issue::OutsidePlot { index }
            | Issue::Overlap { index, .. }
            | Issue::BlocksZone { index, .. } => Some(*index),
            Issue::MissingPatch { .. } | Issue::PlotTooSmall { .. } | Issue::TooBigForAnyPlot { .. } => None,
        }
    }
}

impl fmt::Display for Issue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Issue::DuplicateIndex { index } => write!(f, "entry {index}: index used more than once"),
            Issue::UnknownMacro { index, macro_name } => write!(f, "entry {index}: unknown module {macro_name}"),
            Issue::UnknownUpgradeMacro { index, macro_name } => write!(f, "entry {index}: unknown equipment {macro_name}"),
            Issue::EquipmentNoSlots { index, group, kind } => write!(f, "entry {index}: equipment for {kind} in {group}, which the module doesn't have"),
            Issue::EquipmentMisfit { index, group, macro_name } => write!(f, "entry {index}: {macro_name} doesn't fit {group}"),
            Issue::EquipmentTooMany { index, group, kind, count, slots } => write!(f, "entry {index}: {count} {kind} in {group}, which has {slots} slots"),
            Issue::UnknownSnap { index, macro_name, snap } => write!(f, "entry {index}: {macro_name} has no snap {snap}"),
            Issue::MissingPredecessor { index, predecessor } => write!(f, "entry {index}: predecessor {predecessor} does not exist"),
            Issue::SnapGap { index, metres } => write!(f, "entry {index}: {metres:.3} m away from its linked snap"),
            Issue::SnapMisaligned { index, degrees } => write!(f, "entry {index}: tilted {degrees:.1}° off its snap"),
            Issue::MissingPatch { extension } => write!(f, "plan uses {extension} but does not list it in <patches>"),
            Issue::Overlap { index, other } => write!(f, "entry {index}: overlaps entry {other}"),
            Issue::BlocksZone { index, owner, owner_class } => match owner_class.as_str() {
                "pier" => write!(f, "entry {index}: blocks the approach of pier {owner}"),
                "dockarea" => write!(f, "entry {index}: blocks the approach of dock area {owner}"),
                "buildmodule" => write!(f, "entry {index}: blocks the exit of bay {owner}"),
                _ => write!(f, "entry {index}: blocks the clear space of entry {owner}"),
            },
            Issue::OutsidePlot { index } => write!(f, "entry {index}: outside the plot"),
            Issue::PlotTooSmall { plot, required } => write!(f, "the station needs a {required} plot; the plot is {plot}"),
            Issue::TooBigForAnyPlot { axis, km } => write!(f, "the station is {km} km along {axis}; plots are at most {MAX_AXIS_KM} km"),
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
        let equipment = catalog.equipment().ok();
        for u in &e.upgrades {
            let slots: Vec<_> = module.group_slots(&u.group, &u.kind).collect();
            let (index, group) = (e.index, u.group.clone());
            if slots.is_empty() {
                issues.push(Issue::EquipmentNoSlots { index, group, kind: u.kind.clone() });
                continue;
            }
            // Only buildable equipment is checked: what it fits is known for that.
            if let Some(eq) = equipment.and_then(|l| l.iter().find(|x| x.macro_name.eq_ignore_ascii_case(&u.macro_name))) {
                if !slots.iter().all(|s| eq.fits(s)) {
                    issues.push(Issue::EquipmentMisfit { index, group: group.clone(), macro_name: u.macro_name.clone() });
                }
            }
            if count(u) as usize > slots.len() {
                issues.push(Issue::EquipmentTooMany { index, group, kind: u.kind.clone(), count: count(u), slots: slots.len() as u32 });
            }
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

    issues.extend(overlaps(plan, catalog).into_iter().map(|(other, index, _)| Issue::Overlap { index, other }));
    for (index, owner) in blocked_zones(plan, catalog) {
        let owner_class = by_index.get(&owner).and_then(|e| catalog.module(&e.macro_name).ok()).map(|m| m.class.clone()).unwrap_or_default();
        issues.push(Issue::BlocksZone { index, owner, owner_class });
    }

    for needed in catalog.required_patches(plan) {
        if !plan.patches.iter().any(|p| p.extension == needed.extension) {
            issues.push(Issue::MissingPatch { extension: needed.extension });
        }
    }
    issues
}

/// Plot checks: modules outside `plot`, the plot needed, and whether any plot
/// can hold the station (at most 20 km per axis).
pub fn validate_plot(plan: &Plan, catalog: &Catalog, plot: &Plot) -> Vec<Issue> {
    let Some(required) = required_plot(plan, catalog) else { return Vec::new() };
    let mut issues = Vec::new();
    for (axis, km) in ['X', 'Y', 'Z'].into_iter().zip(required.size()) {
        if km > MAX_AXIS_KM {
            issues.push(Issue::TooBigForAnyPlot { axis, km });
        }
    }
    if !plot.contains(&required) {
        issues.push(Issue::PlotTooSmall { plot: *plot, required });
        issues.extend(
            plan.entries.iter().filter(|e| !entry_inside(plan, catalog, e.index, plot)).map(|e| Issue::OutsidePlot { index: e.index }),
        );
    }
    issues
}
