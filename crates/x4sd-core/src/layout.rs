//! Overlap checks between placed modules, and automatic placement of a module
//! list onto free snaps (like the x4ep_planner's "Build This Design").
//!
//! Overlaps use each module's collision boxes (`Catalog::collision_boxes`): two
//! modules overlap when a pair of their boxes intersects deeper than
//! `OVERLAP_TOLERANCE_M`, unless they are joined at a snap (modules interlock
//! there by design). The boxes enclose the collision meshes loosely (curves,
//! diagonals), so `overlaps` (what `validate` reports) confirms each pair on the
//! meshes themselves (`meshes_intersect`); auto-placement keeps the stricter box
//! test. Calibration in docs/x4-construction-plans.md, "Overlaps".

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use crate::catalog::{Catalog, CollisionMesh, MeshNode, Module, ShapeBox, Snap};
use crate::design::Design;
use crate::geometry::{attach, Quat, Transform};
use crate::plan::{Plan, Upgrade};
use crate::planner::{best, candidates, Candidate};
use crate::plot::{bounds_plot, module_bounds, Plot};
use crate::{Error, Result};

/// How deep the boxes of two modules may intersect before they overlap: room for
/// rounding where faces touch (0.4 m at most where Terran modules meet at a snap
/// in the player's plans); real overlaps are tens of metres.
pub const OVERLAP_TOLERANCE_M: f64 = 1.0;

/// Placements stay this far (metres) inside the plot, so rounding positions to
/// what the plan file stores can't push a module out.
const PLOT_MARGIN_M: f64 = 1.0;

/// Dock areas and piers want open space: nothing within this distance (metres)
/// on the line from the station origin out through their centre.
const DOCK_CLEARANCE_M: f64 = 3_000.0;

/// Snap points closer than this (metres) join two modules (plans store positions
/// in millimetres; the game's own hand-made plans are off by up to 3.5 cm).
const JOINED_M: f64 = 0.1;

/// Twists tried when placing a module on a snap.
const TWISTS: [f64; 4] = [0.0, 90.0, 180.0, 270.0];

/// A module goes onto a new hub instead of a free snap when that puts it this
/// much nearer the origin. Tried on "100 solar, staffed": 0 m adds 124
/// connection structures (4.3% of the cost, 7x8x7 km plot), 2 km 81 (2.9%,
/// 8x8x8 km), 4 km 37 (1.4%, 11x12x12 km).
const HUB_PENALTY_M: f64 = 2000.0;

/// Axis-aligned bounds: (min, max) corner.
type Bounds = ([f64; 3], [f64; 3]);

/// A module part box in the station frame.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldBox {
    pub centre: [f64; 3],
    /// The box's own unit axes.
    pub axes: [[f64; 3]; 3],
    pub half_extents: [f64; 3],
}

impl WorldBox {
    pub fn new(world: &Transform, b: &ShapeBox) -> WorldBox {
        let frame = world.then(&b.transform);
        let axes = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]].map(|a| frame.rotation.rotate(a));
        WorldBox { centre: frame.position, axes, half_extents: b.half_extents }
    }

    /// Axis-aligned bounds: (min, max) corner.
    pub fn bounds(&self) -> Bounds {
        let reach = [0, 1, 2].map(|i| (0..3).map(|k| self.half_extents[k] * self.axes[k][i].abs()).sum::<f64>());
        ([0, 1, 2].map(|i| self.centre[i] - reach[i]), [0, 1, 2].map(|i| self.centre[i] + reach[i]))
    }

    /// Half the box's extent along unit vector `l`.
    fn radius(&self, l: [f64; 3]) -> f64 {
        (0..3).map(|k| self.half_extents[k] * dot(self.axes[k], l).abs()).sum()
    }
}

/// How deep two boxes intersect, in metres: the shortest distance one has to move
/// to come free (separating axis theorem over the 15 candidate axes). Negative
/// when they are apart (then at least that far apart).
pub fn penetration(a: &WorldBox, b: &WorldBox) -> f64 {
    let d = sub(b.centre, a.centre);
    let mut depth = f64::INFINITY;
    let mut test = |l: [f64; 3]| {
        let n = dot(l, l).sqrt();
        if n < 1e-9 {
            return; // parallel edges: covered by the face axes
        }
        let l = l.map(|v| v / n);
        depth = depth.min(a.radius(l) + b.radius(l) - dot(d, l).abs());
    };
    for i in 0..3 {
        test(a.axes[i]);
        test(b.axes[i]);
        for j in 0..3 {
            test(cross(a.axes[i], b.axes[j]));
        }
    }
    depth
}

/// A module's collision boxes and snap points in the station frame.
#[derive(Debug, Clone)]
pub struct Body {
    boxes: Vec<WorldBox>,
    box_bounds: Vec<Bounds>,
    bounds: Bounds,
    snaps: Vec<[f64; 3]>,
}

impl Body {
    /// A module with boxes `boxes` (`Catalog::collision_boxes`) and snaps `snaps`,
    /// both in its own frame, placed at `world`.
    pub fn new(boxes: &[ShapeBox], snaps: &[Snap], world: &Transform) -> Body {
        let boxes: Vec<WorldBox> = boxes.iter().map(|b| WorldBox::new(world, b)).collect();
        let box_bounds: Vec<Bounds> = boxes.iter().map(WorldBox::bounds).collect();
        let bounds = box_bounds.iter().copied().reduce(union).unwrap_or(([0.0; 3], [0.0; 3]));
        Body { boxes, box_bounds, bounds, snaps: snaps.iter().map(|s| world.then(&s.transform).position).collect() }
    }

    /// Axis-aligned bounds of the boxes: (min, max) corner.
    pub fn bounds(&self) -> ([f64; 3], [f64; 3]) {
        self.bounds
    }

    /// Whether `point` lies inside one of its boxes.
    pub fn contains(&self, point: [f64; 3]) -> bool {
        self.boxes.iter().any(|b| {
            let d = sub(point, b.centre);
            (0..3).all(|k| dot(d, b.axes[k]).abs() <= b.half_extents[k])
        })
    }

    /// Whether one of its boxes reaches more than the tolerance into one of `area`'s boxes.
    fn enters(&self, area: &[Bounds]) -> bool {
        let tol = OVERLAP_TOLERANCE_M;
        area.iter().any(|a| intersect(&self.bounds, a, tol) && self.box_bounds.iter().any(|b| intersect(b, a, tol)))
    }

    /// Whether the two modules meet at a snap (linked, or touching where snaps coincide).
    pub fn joined(&self, other: &Body) -> bool {
        self.snaps.iter().any(|a| other.snaps.iter().any(|b| dot(sub(*a, *b), sub(*a, *b)) < JOINED_M * JOINED_M))
    }
}

/// How deep two modules intersect (their deepest pair of boxes), if deeper than
/// `OVERLAP_TOLERANCE_M` and they aren't joined at a snap.
pub fn overlap(a: &Body, b: &Body) -> Option<f64> {
    if a.joined(b) {
        return None;
    }
    intersection_depth(a, b, OVERLAP_TOLERANCE_M)
}

/// How deep two modules' boxes intersect (their deepest pair), if deeper than `min_depth`.
pub fn intersection_depth(a: &Body, b: &Body, min_depth: f64) -> Option<f64> {
    // Boxes intersecting by d overlap by at least d along every direction, so each
    // box of such a pair reaches more than `min_depth` into the other module's bounds.
    if !intersect(&a.bounds, &b.bounds, min_depth) {
        return None;
    }
    let near_b: Vec<usize> = (0..b.boxes.len()).filter(|&j| intersect(&b.box_bounds[j], &a.bounds, min_depth)).collect();
    let mut deepest: Option<f64> = None;
    for i in (0..a.boxes.len()).filter(|&i| intersect(&a.box_bounds[i], &b.bounds, min_depth)) {
        for &j in near_b.iter().filter(|&&j| intersect(&a.box_bounds[i], &b.box_bounds[j], min_depth)) {
            let d = penetration(&a.boxes[i], &b.boxes[j]);
            if d > min_depth && deepest.is_none_or(|x| d > x) {
                deepest = Some(d);
            }
        }
    }
    deepest
}

/// Pairs of entries whose modules overlap: their boxes intersect (`overlap`) and
/// so do their collision meshes (`meshes_intersect`; the box result stands for
/// modules without meshes). (earlier index, later index, box depth in metres), in
/// plan order. Unknown modules are left out.
pub fn overlaps(plan: &Plan, catalog: &Catalog) -> Vec<(u32, u32, f64)> {
    let bodies: Vec<(u32, &str, Transform, Body)> = plan
        .entries
        .iter()
        .filter_map(|e| {
            let (module, boxes) = (catalog.module(&e.macro_name).ok()?, catalog.collision_boxes(&e.macro_name).ok()?);
            let at = Transform::from_offset(e.offset.as_ref());
            Some((e.index, e.macro_name.as_str(), at, Body::new(&boxes, &module.snaps, &at)))
        })
        .collect();
    let mut out = Vec::new();
    for (i, (a, macro_a, at_a, body_a)) in bodies.iter().enumerate() {
        for (b, macro_b, at_b, body_b) in &bodies[i + 1..] {
            let Some(depth) = overlap(body_a, body_b) else { continue };
            let confirmed = match (catalog.collision_mesh(macro_a), catalog.collision_mesh(macro_b)) {
                (Ok(Some(mesh_a)), Ok(Some(mesh_b))) => meshes_intersect(&mesh_a, at_a, &mesh_b, at_b),
                _ => true,
            };
            if confirmed {
                out.push((*a, *b, depth));
            }
        }
    }
    out
}

/// Whether two placed modules' collision meshes intersect: an edge of a triangle
/// of one passes through the inside of a triangle of the other. Faces or edges
/// that only touch don't count. Walks both bounding-volume trees together.
pub fn meshes_intersect(a: &CollisionMesh, at_a: &Transform, b: &CollisionMesh, at_b: &Transform) -> bool {
    // Work in a's frame: b's tree boxes become oriented boxes there.
    let rel = at_a.inverse().then(at_b);
    let node_box = |n: &MeshNode, frame: &Transform| {
        let centre = [0, 1, 2].map(|k| (f64::from(n.lo[k]) + f64::from(n.hi[k])) / 2.0);
        let half_extents = [0, 1, 2].map(|k| (f64::from(n.hi[k]) - f64::from(n.lo[k])) / 2.0);
        WorldBox::new(frame, &ShapeBox { transform: Transform { position: centre, rotation: Quat::IDENTITY }, half_extents })
    };
    let size = |n: &MeshNode| (0..3).map(|k| n.hi[k] - n.lo[k]).sum::<f32>();
    let mut stack = vec![(0u32, 0u32)];
    while let Some((i, j)) = stack.pop() {
        let (na, nb) = (&a.nodes[i as usize], &b.nodes[j as usize]);
        if penetration(&node_box(na, &Transform::IDENTITY), &node_box(nb, &rel)) <= 0.0 {
            continue;
        }
        match (na.children, nb.children) {
            (None, None) => {
                if leaves_intersect(a, na, b, nb, &rel) {
                    return true;
                }
            }
            (Some((l, r)), None) => stack.extend([(l, j), (r, j)]),
            (None, Some((l, r))) => stack.extend([(i, l), (i, r)]),
            (Some((l, r)), Some(_)) if size(na) >= size(nb) => stack.extend([(l, j), (r, j)]),
            (_, Some((l, r))) => stack.extend([(i, l), (i, r)]),
        }
    }
    false
}

/// Whether any triangle of leaf `na` of `a` crosses one of leaf `nb` of `b`
/// (`rel` takes b's frame into a's).
fn leaves_intersect(a: &CollisionMesh, na: &MeshNode, b: &CollisionMesh, nb: &MeshNode, rel: &Transform) -> bool {
    let corners = |m: &CollisionMesh, n: &MeshNode, frame: Option<&Transform>| -> Vec<[[f64; 3]; 3]> {
        m.triangles[n.first as usize..(n.first + n.count) as usize]
            .iter()
            .map(|t| {
                t.map(|i| {
                    let p = m.points[i as usize].map(f64::from);
                    frame.map_or(p, |f| f.then(&Transform { position: p, rotation: Quat::IDENTITY }).position)
                })
            })
            .collect()
    };
    let ta = corners(a, na, None);
    let tb = corners(b, nb, Some(rel));
    ta.iter().any(|x| tb.iter().any(|y| triangles_cross(x, y)))
}

/// Whether two triangles cross: an edge of one passes through the inside of the other.
fn triangles_cross(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> bool {
    (0..3).any(|k| segment_through(a[k], a[(k + 1) % 3], b)) || (0..3).any(|k| segment_through(b[k], b[(k + 1) % 3], a))
}

/// Whether the segment `p`-`q` passes through the inside of triangle `t`
/// (Möller-Trumbore); touching an edge or lying in its plane doesn't count.
fn segment_through(p: [f64; 3], q: [f64; 3], t: &[[f64; 3]; 3]) -> bool {
    const EPS: f64 = 1e-9;
    let (e1, e2, d) = (sub(t[1], t[0]), sub(t[2], t[0]), sub(q, p));
    let h = cross(d, e2);
    let det = dot(e1, h);
    if det.abs() <= 1e-12 * (dot(e1, e1) * dot(d, d) * dot(e2, e2)).sqrt() {
        return false; // parallel to the triangle's plane
    }
    let s = sub(p, t[0]);
    let u = dot(s, h) / det;
    if u <= EPS || u >= 1.0 - EPS {
        return false;
    }
    let r = cross(s, e1);
    let v = dot(d, r) / det;
    if v <= EPS || u + v >= 1.0 - EPS {
        return false;
    }
    let along = dot(e2, r) / det;
    along > EPS && along < 1.0 - EPS
}

/// What `auto_place` did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Placement {
    /// New entries for the requested modules: (index, macro), in placing order.
    pub placed: Vec<(u32, String)>,
    /// Connection structures added to hold them: (index, macro).
    pub connectors: Vec<(u32, String)>,
    /// Modules that found no place: (macro, count).
    pub unplaced: Vec<(String, u32)>,
    /// Dock areas and piers already on the station that were placed again, outside.
    pub docks_moved: u32,
    /// Dock areas and piers (new or moved) left without a clear corridor: no such spot.
    pub docks_blocked: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Options {
    /// Let the station outgrow its plot, up to the largest plot (20 km per axis).
    pub grow_plot: bool,
    /// Race of the connection structures; `None`: the most common race of the modules.
    pub race: Option<String>,
}

/// Attaches the modules (macro, count) to the open design at exact snap
/// positions, clear of each other and inside the plot, as one undo step.
///
/// Greedy: an empty design starts with a connection hub (the race's connection
/// structure with the most snaps) at the origin. Modules go biggest first, dock
/// areas and piers last. Each goes onto the free snap nearest the origin where it
/// fits, with whichever of its own snaps and twists keeps it most compact; or,
/// when that is `HUB_PENALTY_M` nearer, onto a new hub (or a straight connection
/// structure and a hub) on a free snap.
///
/// Docks stay reachable: every run first lifts off the dock areas and piers that
/// have nothing attached and places them again last, where their corridor (see
/// `corridor`) is clear; no module may enter a placed dock's corridor. A lifted
/// dock keeps its equipment, and goes back where it was if it fits nowhere.
pub fn auto_place(catalog: &Catalog, design: &mut Design, modules: &[(String, u32)], options: &Options) -> Result<Placement> {
    let history = design.history_len();
    let mut order: Vec<ToPlace> = Vec::new();
    let mut out = Placement::default();
    let size = |m: &Module| module_bounds(m, &Transform::IDENTITY).map_or(0.0, |(lo, hi)| (0..3).map(|i| hi[i] - lo[i]).product());
    for (macro_name, count) in modules {
        match catalog.module(macro_name) {
            Ok(m) if !m.snaps.is_empty() => {
                let s = size(&m);
                order.extend(std::iter::repeat_n((m, s, None), *count as usize));
            }
            _ => out.unplaced.push((macro_name.clone(), *count)),
        }
    }
    if order.is_empty() {
        return Ok(out);
    }
    while let Some(index) = lift_candidate(catalog, design) {
        let e = design.plan().entries.iter().find(|e| e.index == index).unwrap();
        let (m, at, upgrades) = (catalog.module(&e.macro_name)?, design.world(index)?, e.upgrades.clone());
        let s = size(&m);
        order.push((m, s, Some((at, upgrades))));
        design.remove(catalog, index)?;
    }
    order.sort_by(|(a, sa, _), (b, sb, _)| docking(a).cmp(&docking(b)).then(sb.total_cmp(sa)).then(a.macro_name.cmp(&b.macro_name)));

    let all = candidates(catalog)?;
    let race = options.race.clone().or_else(|| common_race(&all, order.iter().map(|o| o.0.macro_name.as_str())));
    let connectors = Connectors::choose(catalog, &all, race.as_deref())?;
    let mut placer = Placer::new(catalog, design, if options.grow_plot { None } else { design.plot() })?;
    if design.plan().entries.is_empty() {
        let hub = design.add_root(catalog, &connectors.hub.module.macro_name, Transform::IDENTITY)?;
        placer.add(design, hub)?;
        out.connectors.push((hub, connectors.hub.module.macro_name.clone()));
    }

    let mut unplaced: BTreeMap<String, u32> = BTreeMap::new();
    for (module, _, lifted) in &order {
        match (placer.place(design, &connectors, module)?, lifted) {
            (Some((index, added)), None) => {
                out.placed.push((index, module.macro_name.clone()));
                out.connectors.extend(added);
            }
            (Some((index, added)), Some((_, upgrades))) => {
                design.restore_upgrades(index, upgrades.clone());
                out.docks_moved += 1;
                out.connectors.extend(added);
            }
            (None, None) => *unplaced.entry(module.macro_name.clone()).or_default() += 1,
            (None, Some((at, upgrades))) => {
                // Nowhere else: back where it was (no longer attached; the snap may be taken).
                let index = design.add_root(catalog, &module.macro_name, *at)?;
                design.restore_upgrades(index, upgrades.clone());
                placer.add(design, index)?;
                placer.blocked += 1;
            }
        }
    }
    out.unplaced.extend(unplaced);
    out.docks_blocked = placer.blocked;
    design.squash_history(history);
    Ok(out)
}

/// A module to place: (module, size, for a lifted dock: where it was and its equipment).
type ToPlace = (Arc<Module>, f64, Option<(Transform, Vec<Upgrade>)>);

/// A dock area or pier with nothing attached to it, which a placement run lifts
/// off to place again outside.
fn lift_candidate(catalog: &Catalog, design: &Design) -> Option<u32> {
    let entries = &design.plan().entries;
    entries
        .iter()
        .filter(|e| catalog.module(&e.macro_name).is_ok_and(|m| docking(&m)))
        .find(|e| !entries.iter().any(|x| x.predecessor.as_ref().is_some_and(|p| p.index == e.index)))
        .map(|e| e.index)
}

/// A dock's clearance corridor: its box (`bounds`) swept `DOCK_CLEARANCE_M` straight
/// out from the station's centre, as overlapping boxes. `None` at the centre.
fn corridor(bounds: &Bounds, centre: [f64; 3]) -> Option<Vec<Bounds>> {
    let mid = [0, 1, 2].map(|i| (bounds.0[i] + bounds.1[i]) / 2.0);
    let out = sub(mid, centre);
    let length = dot(out, out).sqrt();
    if length < 1.0 {
        return None;
    }
    let dir = out.map(|v| v / length);
    let smallest = (0..3).map(|i| bounds.1[i] - bounds.0[i]).fold(f64::MAX, f64::min);
    let step = (smallest / 2.0).max(25.0);
    let steps = (DOCK_CLEARANCE_M / step).ceil() as usize;
    Some((1..=steps).map(|k| { let s = k as f64 * step; ([0, 1, 2].map(|i| bounds.0[i] + dir[i] * s), [0, 1, 2].map(|i| bounds.1[i] + dir[i] * s)) }).collect())
}

/// Dock areas and piers: ships have to reach them.
fn docking(m: &Module) -> bool {
    m.class == "dockarea" || m.class == "pier"
}

/// The most common maker race among the modules (`None` if none has one).
fn common_race<'a>(all: &[Candidate], macros: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut count: BTreeMap<&str, usize> = BTreeMap::new();
    for m in macros {
        if let Some(race) = all.iter().find(|c| c.info.macro_name.eq_ignore_ascii_case(m)).and_then(|c| c.race.as_deref()) {
            *count.entry(race).or_default() += 1;
        }
    }
    count.into_iter().max_by_key(|(_, n)| *n).map(|(r, _)| r.to_owned())
}

/// A connection structure with its collision boxes.
struct Piece {
    module: Arc<Module>,
    boxes: Arc<Vec<ShapeBox>>,
}

/// The connection structures the placer adds: a hub, and straight ones (two
/// opposite snaps) to put a hub further out, shortest first.
struct Connectors {
    hub: Piece,
    struts: Vec<Piece>,
}

impl Connectors {
    fn choose(catalog: &Catalog, all: &[Candidate], race: Option<&str>) -> Result<Connectors> {
        let piece = |c: &Candidate| -> Result<Piece> { Ok(Piece { module: c.module.clone(), boxes: catalog.collision_boxes(&c.info.macro_name)? }) };
        let connection = || all.iter().filter(|c| c.info.class == "connectionmodule");
        let hub = best(connection().filter(|c| c.module.snaps.len() >= 3), race, |c| c.module.snaps.len() as f64)
            .ok_or_else(|| Error("no connection structure with three or more snaps".to_owned()))?;
        let mut struts: Vec<(f64, &Candidate)> = connection()
            .filter(|c| c.race == hub.race)
            .filter_map(|c| match c.module.snaps.as_slice() {
                [a, b] => {
                    let z = [0.0, 0.0, 1.0];
                    let facing = dot(a.transform.rotation.rotate(z), b.transform.rotation.rotate(z));
                    let gap = sub(a.transform.position, b.transform.position);
                    (facing < -0.999).then(|| (dot(gap, gap).sqrt(), c))
                }
                _ => None,
            })
            .collect();
        struts.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.info.macro_name.cmp(&b.1.info.macro_name)));
        // One per length (a vertical structure is a base structure on its side).
        struts.dedup_by(|b, a| (a.0 - b.0).abs() < 1.0);
        Ok(Connectors { hub: piece(hub)?, struts: struts.into_iter().map(|(_, c)| piece(c)).collect::<Result<_>>()? })
    }
}

/// A snap nothing uses yet, in the station frame.
#[derive(Debug, Clone)]
struct FreeSnap {
    entry: u32,
    snap: String,
    world: Transform,
}

/// A module at a place where it fits: its body and part-box bounds.
struct Fit {
    body: Body,
    bounds: Bounds,
}

/// The best way to attach a module to one snap: (own snap, twist, fit).
type Choice = (String, f64, Fit);

/// A placed module's index, and the connection structures added for it: (index, macro).
type Placed = (u32, Vec<(u32, String)>);

struct Placer<'a> {
    catalog: &'a Catalog,
    /// The plot to stay inside; `None`: any plot the game allows.
    plot: Option<Plot>,
    bodies: Vec<Body>,
    /// Part-box bounds of the whole station (what the plot must hold).
    bounds: Option<Bounds>,
    free: Vec<FreeSnap>,
    /// The entry each body belongs to.
    owners: Vec<u32>,
    /// (entry, snap, macro, through a new hub, outward) that didn't fit. Placing
    /// only adds obstacles, so what failed once fails again.
    failed: HashSet<(u32, String, String, bool, bool)>,
    /// Dock areas and piers now need a clear corridor (see `corridor`).
    outward: bool,
    /// Corridors of docks on the station that have one: no module may enter them.
    corridors: Vec<Vec<Bounds>>,
    /// Docks placed (or put back) without a clear corridor.
    blocked: u32,
}

impl<'a> Placer<'a> {
    fn new(catalog: &'a Catalog, design: &Design, plot: Option<Plot>) -> Result<Placer<'a>> {
        let mut placer = Placer {
            catalog,
            plot,
            bodies: Vec::new(),
            bounds: None,
            free: Vec::new(),
            owners: Vec::new(),
            failed: HashSet::new(),
            outward: false,
            corridors: Vec::new(),
            blocked: 0,
        };
        for e in &design.plan().entries {
            if catalog.module(&e.macro_name).is_ok() {
                placer.add(design, e.index)?;
            }
        }
        // Docks that stay (something is attached to them) keep their corridor clear.
        for e in &design.plan().entries {
            if catalog.module(&e.macro_name).is_ok_and(|m| docking(&m)) {
                placer.protect(design, e.index)?;
            }
        }
        Ok(placer)
    }

    /// The centre of the station's bounds.
    fn centre(&self) -> [f64; 3] {
        self.bounds.map_or([0.0; 3], |(lo, hi)| [0, 1, 2].map(|i| (lo[i] + hi[i]) / 2.0))
    }

    /// Keeps the corridor of dock `index` clear from now on, if it is clear now.
    fn protect(&mut self, design: &Design, index: u32) -> Result<()> {
        let e = design.plan().entries.iter().find(|e| e.index == index).ok_or_else(|| Error(format!("no entry {index}")))?;
        let module = self.catalog.module(&e.macro_name)?;
        let Some(bounds) = module_bounds(&module, &design.world(index)?) else { return Ok(()) };
        let Some(lane) = corridor(&bounds, self.centre()) else { return Ok(()) };
        let clear = self.bodies.iter().zip(&self.owners).all(|(b, &owner)| owner == index || !b.enters(&lane));
        if clear {
            self.corridors.push(lane);
        }
        Ok(())
    }

    /// Takes a placed entry into account: its body, bounds and free snaps.
    fn add(&mut self, design: &Design, index: u32) -> Result<()> {
        let e = design.plan().entries.iter().find(|e| e.index == index).ok_or_else(|| Error(format!("no entry {index}")))?;
        let module = self.catalog.module(&e.macro_name)?;
        let world = design.world(index)?;
        self.bodies.push(Body::new(&self.catalog.collision_boxes(&e.macro_name)?, &module.snaps, &world));
        self.owners.push(index);
        if let Some(b) = module_bounds(&module, &world) {
            self.bounds = Some(self.bounds.map_or(b, |s| union(s, b)));
        }
        self.free.extend(design.free_snaps(self.catalog, index)?.into_iter().map(|s| FreeSnap { entry: index, snap: s.name, world: s.world }));
        Ok(())
    }

    fn take(&mut self, entry: u32, snap: &str) {
        self.free.retain(|f| !(f.entry == entry && f.snap == snap));
    }

    /// Whether the module fits at `at`: inside the plot and clear of every placed
    /// module and of `extra` (pieces about to be placed).
    fn fits(&self, module: &Module, boxes: &[ShapeBox], at: &Transform, extra: &[&Fit]) -> Option<Fit> {
        let bounds = module_bounds(module, at)?;
        let m = PLOT_MARGIN_M;
        let inside = match self.plot {
            Some(p) => (0..3).all(|i| bounds.1[i] + m <= p.pos[i] as f64 * 1000.0 && m - bounds.0[i] <= p.neg[i] as f64 * 1000.0),
            None => {
                let (lo, hi) = extra.iter().fold(self.bounds.map_or(bounds, |s| union(s, bounds)), |s, f| union(s, f.bounds));
                bounds_plot((lo.map(|v| v - m), hi.map(|v| v + m))).is_allowed()
            }
        };
        if !inside {
            return None;
        }
        let body = Body::new(boxes, &module.snaps, at);
        let clear = self.bodies.iter().chain(extra.iter().map(|f| &f.body)).all(|b| overlap(b, &body).is_none());
        // Nothing enters a dock's corridor; a dock (while `outward`) needs a clear one.
        let lanes_free = !self.corridors.iter().any(|lane| body.enters(lane));
        let open = !self.outward || !docking(module) || self.exposed(&bounds, extra);
        (clear && lanes_free && open).then_some(Fit { body, bounds })
    }

    /// Whether the corridor of a dock at `bounds` is clear of everything placed
    /// (and of `extra`).
    fn exposed(&self, bounds: &Bounds, extra: &[&Fit]) -> bool {
        let Some(lane) = corridor(bounds, self.centre()) else { return false };
        !self.bodies.iter().chain(extra.iter().map(|f| &f.body)).any(|b| b.enters(&lane))
    }

    /// The most compact way (module centre nearest the origin) to attach the
    /// module to a snap at `snap` that fits.
    fn best_at(&self, module: &Module, boxes: &[ShapeBox], snap: &Transform, extra: &[&Fit]) -> Option<Choice> {
        let mut options: Vec<(f64, &str, f64, Transform)> = Vec::new();
        for s in &module.snaps {
            for twist in TWISTS {
                let at = attach(snap, &Transform::IDENTITY, &s.transform, twist);
                let Some((lo, hi)) = module_bounds(module, &at) else { continue };
                let centre = [0, 1, 2].map(|i| (lo[i] + hi[i]) / 2.0);
                options.push((dot(centre, centre), &s.name, twist, at));
            }
        }
        options.sort_by(|a, b| a.0.total_cmp(&b.0));
        options.into_iter().find_map(|(_, name, twist, at)| self.fits(module, boxes, &at, extra).map(|f| (name.to_owned(), twist, f)))
    }

    /// Places one module; returns its index and the connection structures added for it.
    /// Dock areas and piers go where they face open space if they can.
    fn place(&mut self, design: &mut Design, conn: &Connectors, module: &Module) -> Result<Option<Placed>> {
        for &outward in if docking(module) { &[true, false][..] } else { &[false] } {
            self.outward = outward;
            let placed = self.place_with(design, conn, module)?;
            self.outward = false;
            if let Some((index, _)) = &placed {
                if outward {
                    self.protect(design, *index)?;
                } else if docking(module) {
                    self.blocked += 1;
                }
                return Ok(placed);
            }
        }
        Ok(None)
    }

    fn place_with(&mut self, design: &mut Design, conn: &Connectors, module: &Module) -> Result<Option<Placed>> {
        let boxes = self.catalog.collision_boxes(&module.macro_name)?;
        let name = &module.macro_name;
        // Ways to place it, nearest the origin first: directly on a free snap, or
        // through a new hub there (counted `HUB_PENALTY_M` further out).
        let mut options: Vec<(f64, &FreeSnap, bool)> = Vec::new();
        for f in &self.free {
            let d = dot(f.world.position, f.world.position).sqrt();
            options.push((d, f, false));
            options.push((d + HUB_PENALTY_M, f, true));
        }
        options.sort_by(|a, b| a.0.total_cmp(&b.0));
        let options: Vec<(FreeSnap, bool)> = options.into_iter().map(|(_, f, x)| (f.clone(), x)).collect();
        for (f, through_hub) in options {
            let key = (f.entry, f.snap.clone(), name.clone(), through_hub, self.outward);
            if self.failed.contains(&key) {
                continue;
            }
            if !through_hub {
                if let Some((snap, twist, _)) = self.best_at(module, &boxes, &f.world, &[]) {
                    let index = design.attach(self.catalog, name, &snap, f.entry, &f.snap, twist)?;
                    self.take(f.entry, &f.snap);
                    self.add(design, index)?;
                    return Ok(Some((index, Vec::new())));
                }
            } else {
                for strut in std::iter::once(None).chain(conn.struts.iter().map(Some)) {
                    if let Some(unit) = self.unit_at(&f, strut, &conn.hub, module, &boxes) {
                        return self.commit(design, &f, strut, &conn.hub, module, unit).map(Some);
                    }
                }
            }
            self.failed.insert(key);
        }
        Ok(None)
    }

    /// A hub at free snap `f` (on a strut, if given) that the module fits on:
    /// (hub snap, hub twist, hub snap the module takes, the module's choice).
    fn unit_at(&self, f: &FreeSnap, strut: Option<&Piece>, hub: &Piece, module: &Module, boxes: &[ShapeBox]) -> Option<(String, f64, String, Choice)> {
        let mut base = f.world;
        let strut_fit = match strut {
            Some(s) => {
                let at = attach(&f.world, &Transform::IDENTITY, &s.module.snaps[0].transform, 0.0);
                base = at.then(&s.module.snaps[1].transform);
                Some(self.fits(&s.module, &s.boxes, &at, &[])?)
            }
            None => None,
        };
        let extra: Vec<&Fit> = strut_fit.iter().collect();
        // Hub placements that put its snaps in the same places are the same for the module.
        let mut seen: Vec<Vec<[i64; 3]>> = Vec::new();
        for hs in &hub.module.snaps {
            for twist in [0.0, 90.0] {
                let at = attach(&base, &Transform::IDENTITY, &hs.transform, twist);
                let mut key: Vec<[i64; 3]> = hub.module.snaps.iter().map(|s| at.then(&s.transform).position.map(|v| v.round() as i64)).collect();
                key.sort();
                if seen.contains(&key) {
                    continue;
                }
                seen.push(key);
                let Some(hub_fit) = self.fits(&hub.module, &hub.boxes, &at, &extra) else { continue };
                let extra: Vec<&Fit> = extra.iter().copied().chain(std::iter::once(&hub_fit)).collect();
                let best = hub
                    .module
                    .snaps
                    .iter()
                    .filter(|o| o.name != hs.name)
                    .filter_map(|o| self.best_at(module, boxes, &at.then(&o.transform), &extra).map(|c| (o.name.clone(), c)))
                    .min_by(|a, b| distance(&a.1 .2.bounds).total_cmp(&distance(&b.1 .2.bounds)));
                if let Some((on, choice)) = best {
                    return Some((hs.name.clone(), twist, on, choice));
                }
            }
        }
        None
    }

    fn commit(
        &mut self,
        design: &mut Design,
        f: &FreeSnap,
        strut: Option<&Piece>,
        hub: &Piece,
        module: &Module,
        (hub_snap, hub_twist, on, (snap, twist, _)): (String, f64, String, Choice),
    ) -> Result<Placed> {
        let mut added = Vec::new();
        let (mut to, mut to_snap) = (f.entry, f.snap.clone());
        self.take(f.entry, &f.snap);
        if let Some(s) = strut {
            let (a, b) = (&s.module.snaps[0].name, &s.module.snaps[1].name);
            let index = design.attach(self.catalog, &s.module.macro_name, a, to, &to_snap, 0.0)?;
            self.add(design, index)?;
            added.push((index, s.module.macro_name.clone()));
            (to, to_snap) = (index, b.clone());
            self.take(to, &to_snap);
        }
        let index = design.attach(self.catalog, &hub.module.macro_name, &hub_snap, to, &to_snap, hub_twist)?;
        self.add(design, index)?;
        added.push((index, hub.module.macro_name.clone()));
        let placed = design.attach(self.catalog, &module.macro_name, &snap, index, &on, twist)?;
        self.take(index, &on);
        self.add(design, placed)?;
        Ok((placed, added))
    }
}

/// Squared distance of the bounds' centre from the origin.
fn distance((lo, hi): &Bounds) -> f64 {
    let c = [0, 1, 2].map(|i| (lo[i] + hi[i]) / 2.0);
    dot(c, c)
}

/// Whether the bounds overlap by more than `margin` along every axis.
fn intersect(a: &Bounds, b: &Bounds, margin: f64) -> bool {
    (0..3).all(|i| a.1[i] - b.0[i] > margin && b.1[i] - a.0[i] > margin)
}

fn union(a: Bounds, b: Bounds) -> Bounds {
    ([0, 1, 2].map(|i| a.0[i].min(b.0[i])), [0, 1, 2].map(|i| a.1[i].max(b.1[i])))
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Axis, Quat};

    fn cube(position: [f64; 3], yaw: f64, half: f64) -> WorldBox {
        let world = Transform { position, rotation: Quat::axis_angle(Axis::Y, yaw) };
        WorldBox::new(&world, &ShapeBox { transform: Transform::IDENTITY, half_extents: [half; 3] })
    }

    #[test]
    fn penetration_of_boxes() {
        let a = cube([0.0; 3], 0.0, 100.0);
        // Faces 10 m into each other, touching, and 50 m apart.
        assert!((penetration(&a, &cube([190.0, 0.0, 0.0], 0.0, 100.0)) - 10.0).abs() < 1e-9);
        assert!(penetration(&a, &cube([200.0, 0.0, 0.0], 0.0, 100.0)).abs() < 1e-9);
        assert!((penetration(&a, &cube([250.0, 0.0, 0.0], 0.0, 100.0)) + 50.0).abs() < 1e-9);
        // A cube turned 45° reaches 141 m along x: at 240 m its edge is 1.4 m in.
        let turned = cube([240.0, 0.0, 0.0], 45.0, 100.0);
        assert!((penetration(&a, &turned) - (100.0 * 2f64.sqrt() - 140.0)).abs() < 1e-6);
        assert!((turned.bounds().0[0] - (240.0 - 100.0 * 2f64.sqrt())).abs() < 1e-6);
        // Diagonally apart: the bounds overlap along x and z, the turned cube's face separates them.
        let far = cube([175.0, 0.0, 175.0], 45.0, 100.0);
        assert!((penetration(&a, &far) - (100.0 * 2f64.sqrt() + 100.0 - 175.0 * 2f64.sqrt())).abs() < 1e-6);
    }

    fn shape(position: [f64; 3], half_extents: [f64; 3]) -> ShapeBox {
        ShapeBox { transform: Transform { position, rotation: Quat::IDENTITY }, half_extents }
    }

    #[test]
    fn triangles_cross_only_through_their_insides() {
        let t = [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]];
        // A triangle standing through t's middle crosses it; one beside it, or only
        // touching its edge, or lying in its plane, doesn't.
        assert!(triangles_cross(&t, &[[2.0, 2.0, -5.0], [2.0, 2.0, 5.0], [4.0, 2.0, 5.0]]));
        assert!(!triangles_cross(&t, &[[20.0, 2.0, -5.0], [20.0, 2.0, 5.0], [22.0, 2.0, 5.0]]));
        assert!(!triangles_cross(&t, &[[2.0, 2.0, 0.0], [2.0, 2.0, 5.0], [4.0, 2.0, 5.0]]), "a corner on t: touching");
        assert!(!triangles_cross(&t, &[[1.0, 1.0, 0.0], [3.0, 1.0, 0.0], [1.0, 3.0, 0.0]]), "coplanar");
    }

    #[test]
    fn module_overlap_uses_the_deepest_box_pair() {
        // A 200 m block with a thin spire on top (y 100..500), and a block with an arm.
        let spired = [shape([0.0; 3], [100.0; 3]), shape([0.0, 300.0, 0.0], [10.0, 200.0, 10.0])];
        let armed = [shape([0.0; 3], [100.0; 3]), shape([-230.0, 300.0, 0.0], [40.0, 10.0, 10.0])];
        let at = |boxes: &[ShapeBox], x: f64| Body::new(boxes, &[], &Transform { position: [x, 0.0, 0.0], ..Transform::IDENTITY });
        // Side by side, touching: no overlap; 0.5 m in: within the tolerance; 30 m in: overlap.
        assert_eq!(overlap(&at(&spired, 0.0), &at(&spired, 200.0)), None);
        assert_eq!(overlap(&at(&spired, 0.0), &at(&spired, 199.5)), None);
        assert!((overlap(&at(&spired, 0.0), &at(&spired, 170.0)).unwrap() - 30.0).abs() < 1e-9);
        // 250 m apart the blocks are clear, but the arm (x -20..60) crosses the spire (x -10..10).
        assert!((overlap(&at(&spired, 0.0), &at(&armed, 250.0)).unwrap() - 20.0).abs() < 1e-9);
        assert_eq!(overlap(&at(&spired, 0.0), &at(&armed, 290.0)), None, "the arm ends at x 20, clear of the spire");
        // Joined at a snap, modules may interlock.
        let snap = |x: f64| Snap { name: "s".to_owned(), transform: Transform { position: [x, 0.0, 0.0], rotation: Quat::IDENTITY } };
        let joined = |x: f64, s: f64| Body::new(&spired, &[snap(s)], &Transform { position: [x, 0.0, 0.0], ..Transform::IDENTITY });
        assert_eq!(overlap(&joined(0.0, 85.0), &joined(170.0, -85.0)), None);
        assert!(overlap(&joined(0.0, 85.0), &joined(170.0, -80.0)).is_some(), "5 m apart is not joined");
    }
}
