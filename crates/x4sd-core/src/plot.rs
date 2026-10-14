//! Station plots: the box a station must fit in (see tasks/workflow-plot-size.md).
//!
//! The game measures a plot per side from the station origin in whole
//! kilometres (+X, −X, +Y, −Y, +Z, −Z), at most 20 km per axis, and requires
//! the smallest such box that contains the station's modules.

use std::fmt;

use crate::catalog::{Catalog, Module};
use crate::geometry::Transform;
use crate::plan::Plan;

/// The game's largest plot along one axis (`maxPlotSize` in the build menu).
pub const MAX_AXIS_KM: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plot {
    /// Reach on the positive side of x, y, z, in km.
    pub pos: [u32; 3],
    /// Reach on the negative side of x, y, z, in km.
    pub neg: [u32; 3],
}

impl Plot {
    /// A plot reaching `km` on every side (2·`km` per axis).
    pub fn around(km: u32) -> Plot {
        Plot { pos: [km; 3], neg: [km; 3] }
    }

    /// Size per axis in km.
    pub fn size(&self) -> [u32; 3] {
        [0, 1, 2].map(|i| self.pos[i] + self.neg[i])
    }

    /// Whether the game allows this plot: 1..=20 km per axis.
    pub fn is_allowed(&self) -> bool {
        self.size().iter().all(|&s| (1..=MAX_AXIS_KM).contains(&s))
    }

    /// Whether this plot reaches at least as far as `other` on every side.
    pub fn contains(&self, other: &Plot) -> bool {
        (0..3).all(|i| self.pos[i] >= other.pos[i] && self.neg[i] >= other.neg[i])
    }

    /// The smallest plot reaching as far as both on every side.
    pub fn union(&self, other: &Plot) -> Plot {
        Plot { pos: [0, 1, 2].map(|i| self.pos[i].max(other.pos[i])), neg: [0, 1, 2].map(|i| self.neg[i].max(other.neg[i])) }
    }
}

impl fmt::Display for Plot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [x, y, z] = self.size();
        // Plain ASCII: this text goes to consoles, logs and agents.
        write!(f, "{x}x{y}x{z} km (+X {} -X {}, +Y {} -Y {}, +Z {} -Z {})", self.pos[0], self.neg[0], self.pos[1], self.neg[1], self.pos[2], self.neg[2])
    }
}

/// Axis-aligned bounds (min, max corner, metres, station frame) of one entry's
/// module, from its part boxes; `None` for unknown modules.
pub fn entry_bounds(plan: &Plan, catalog: &Catalog, index: u32) -> Option<([f64; 3], [f64; 3])> {
    let entry = plan.entries.iter().find(|e| e.index == index)?;
    let module = catalog.module(&entry.macro_name).ok()?;
    module_bounds(&module, &Transform::from_offset(entry.offset.as_ref()))
}

/// Axis-aligned bounds of a module placed at `world`, from its part boxes.
pub fn module_bounds(module: &Module, world: &Transform) -> Option<([f64; 3], [f64; 3])> {
    let mut bounds: Option<([f64; 3], [f64; 3])> = None;
    for b in &module.shape {
        let frame = world.then(&b.transform);
        for corner in 0..8 {
            let local = [0, 1, 2].map(|i| if corner >> i & 1 == 1 { b.half_extents[i] } else { -b.half_extents[i] });
            let p = frame.then(&Transform { position: local, rotation: crate::geometry::Quat::IDENTITY }).position;
            let (lo, hi) = bounds.get_or_insert((p, p));
            for i in 0..3 {
                lo[i] = lo[i].min(p[i]);
                hi[i] = hi[i].max(p[i]);
            }
        }
    }
    bounds
}

/// The smallest whole-km plot containing all modules (the game's minimum plot);
/// `None` for an empty design. Sides with nothing on them still get 0 km, and an
/// axis is at least 1 km in total.
pub fn required_plot(plan: &Plan, catalog: &Catalog) -> Option<Plot> {
    let mut all: Option<([f64; 3], [f64; 3])> = None;
    for e in &plan.entries {
        let Some((lo, hi)) = entry_bounds(plan, catalog, e.index) else { continue };
        let (alo, ahi) = all.get_or_insert((lo, hi));
        for i in 0..3 {
            alo[i] = alo[i].min(lo[i]);
            ahi[i] = ahi[i].max(hi[i]);
        }
    }
    all.map(bounds_plot)
}

/// The smallest whole-km plot containing the bounds (min, max corner, metres).
pub fn bounds_plot((lo, hi): ([f64; 3], [f64; 3])) -> Plot {
    let mut p = Plot { pos: hi.map(km_out), neg: lo.map(|v| km_out(-v)) };
    for i in 0..3 {
        if p.pos[i] + p.neg[i] == 0 {
            p.pos[i] = 1;
        }
    }
    p
}

/// Whole km needed to reach `metres` from the origin (rounded up; nothing for ≤ 0).
fn km_out(metres: f64) -> u32 {
    if metres <= 0.0 { 0 } else { (metres / 1000.0).ceil() as u32 }
}

/// Whether one entry's module lies inside `plot`.
pub fn entry_inside(plan: &Plan, catalog: &Catalog, index: u32, plot: &Plot) -> bool {
    let Some((lo, hi)) = entry_bounds(plan, catalog, index) else { return true };
    (0..3).all(|i| hi[i] <= plot.pos[i] as f64 * 1000.0 && -lo[i] <= plot.neg[i] as f64 * 1000.0)
}
