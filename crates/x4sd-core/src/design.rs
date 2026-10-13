//! Editing a plan: the operations the builder, the CLI and MCP share (D-7, D-8).
//!
//! Every operation keeps the plan valid by construction: attached modules are
//! placed with the snap math, moving or twisting a module carries everything
//! attached to it along, and removing a module removes its descendants and
//! renumbers the entries `1..n` like the game writes them.
//!
//! Entry order is the build order: a module always comes after the module it
//! attaches to. `bookmark="1"` on an entry ends a build stage (see
//! docs/x4-construction-plans.md, "Build order and stages").

use std::sync::Arc;

use crate::catalog::{Catalog, Module, Snap};
use crate::geometry::{attach as attach_transform, Transform};
use crate::import::new_plan_id;
use crate::plan::{Entry, Plan, Predecessor};
use crate::plot::{required_plot, Plot};
use crate::{Error, Result};

pub struct Design {
    plan: Plan,
    undo: Vec<Plan>,
    redo: Vec<Plan>,
    /// The plot the station must fit in. Not part of the plan file (the game keeps
    /// plots per station), and not undone with edits.
    plot: Option<Plot>,
}

/// A snap point of a placed module that nothing uses, in the station frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FreeSnap {
    pub name: String,
    pub world: Transform,
}

impl Design {
    /// An empty design on a 2×2×2 km plot.
    pub fn new(name: &str) -> Design {
        let mut design = Design::from_plan(Plan {
            id: new_plan_id(),
            name: name.to_owned(),
            description: Some(String::new()),
            tags: None,
            useplanname: None,
            fixed: None,
            patches: Vec::new(),
            entries: Vec::new(),
        });
        design.plot = Some(Plot::around(1));
        design
    }

    /// A design for an existing plan, without a plot (see `fit_plot`).
    pub fn from_plan(plan: Plan) -> Design {
        Design { plan, undo: Vec::new(), redo: Vec::new(), plot: None }
    }

    pub fn plot(&self) -> Option<Plot> {
        self.plot
    }

    pub fn set_plot(&mut self, plot: Option<Plot>) -> Result<()> {
        if plot.is_some_and(|p| !p.is_allowed()) {
            return Err(Error("a plot is 1 to 20 km along each axis".to_owned()));
        }
        self.plot = plot;
        Ok(())
    }

    /// Sets the plot to the smallest that holds the station, or with `grow_only`
    /// enlarges the current plot just enough. Fails when no plot can hold it.
    pub fn fit_plot(&mut self, catalog: &Catalog, grow_only: bool) -> Result<Plot> {
        let required = required_plot(&self.plan, catalog).unwrap_or(Plot::around(1));
        let plot = match (grow_only, self.plot) {
            (true, Some(current)) => current.union(&required),
            _ => required,
        };
        if !plot.is_allowed() {
            return Err(Error(format!("the station needs {required}; plots are at most 20 km per axis")));
        }
        self.plot = Some(plot);
        Ok(plot)
    }

    pub fn plan(&self) -> &Plan {
        &self.plan
    }

    pub fn rename(&mut self, name: &str) {
        self.checkpoint();
        self.plan.name = name.to_owned();
    }

    /// Places a module on its own (not attached to anything) at `at`.
    pub fn add_root(&mut self, catalog: &Catalog, macro_name: &str, at: Transform) -> Result<u32> {
        let module = catalog.module(macro_name)?;
        self.checkpoint();
        let index = self.next_index();
        self.plan.entries.push(new_entry(index, &module, None, at));
        self.plan.patches = catalog.required_patches(&self.plan);
        Ok(index)
    }

    /// Attaches a new module by its snap `snap` to snap `to_snap` of entry `to`,
    /// twisted by `twist_degrees` around the snap axis.
    pub fn attach(
        &mut self,
        catalog: &Catalog,
        macro_name: &str,
        snap: &str,
        to: u32,
        to_snap: &str,
        twist_degrees: f64,
    ) -> Result<u32> {
        let module = catalog.module(macro_name)?;
        let child_snap = find_snap(&module, snap)?;
        let pred_snap = find_snap(&catalog.module(&self.entry(to)?.macro_name)?, to_snap)?;
        if self.snap_in_use(to, &pred_snap.name) {
            return Err(Error(format!("snap {} of entry {to} is already in use", pred_snap.name)));
        }
        let at = attach_transform(&self.world(to)?, &pred_snap.transform, &child_snap.transform, twist_degrees);
        self.checkpoint();
        let index = self.next_index();
        let link = (child_snap.name.clone(), Predecessor { index: to, connection: pred_snap.name.clone() });
        self.plan.entries.push(new_entry(index, &module, Some(link), at));
        self.plan.patches = catalog.required_patches(&self.plan);
        Ok(index)
    }

    /// Re-attaches entry `index` to its predecessor with a new twist; everything
    /// attached to it moves along.
    pub fn set_twist(&mut self, catalog: &Catalog, index: u32, twist_degrees: f64) -> Result<()> {
        let e = self.entry(index)?;
        let (Some(conn), Some(pred)) = (&e.connection, &e.predecessor) else {
            return Err(Error(format!("entry {index} is not attached to anything; move it instead")));
        };
        let child_snap = find_snap(&catalog.module(&e.macro_name)?, conn)?;
        let pred_snap = find_snap(&catalog.module(&self.entry(pred.index)?.macro_name)?, &pred.connection)?;
        let target = attach_transform(&self.world(pred.index)?, &pred_snap.transform, &child_snap.transform, twist_degrees);
        self.checkpoint();
        self.move_subtree(index, &target);
        Ok(())
    }

    /// Moves an unattached module, and everything attached to it, to `at`.
    pub fn move_root(&mut self, index: u32, at: Transform) -> Result<()> {
        if self.entry(index)?.predecessor.is_some() {
            return Err(Error(format!("entry {index} is attached; change its twist or remove it instead")));
        }
        self.checkpoint();
        self.move_subtree(index, &at);
        Ok(())
    }

    /// Removes entry `index` and every module attached to it, directly or not,
    /// then renumbers the rest `1..n`. Returns how many entries were removed.
    pub fn remove(&mut self, catalog: &Catalog, index: u32) -> Result<usize> {
        self.entry(index)?;
        let doomed = self.subtree(index);
        self.checkpoint();
        self.plan.entries.retain(|e| !doomed.contains(&e.index));
        self.renumber();
        self.plan.patches = catalog.required_patches(&self.plan);
        Ok(doomed.len())
    }

    /// Moves entry `index` to `position` (1-based) in the build order. Fails when
    /// that would put it before the module it attaches to, or after a module
    /// attached to it. Entries are renumbered to the new order; returns the
    /// module's new index.
    pub fn move_in_build_order(&mut self, index: u32, position: usize) -> Result<u32> {
        let from = self.position_of(index)?;
        if position == 0 || position > self.plan.entries.len() {
            return Err(Error(format!("position {position} is outside 1..{}", self.plan.entries.len())));
        }
        let mut order: Vec<u32> = self.plan.entries.iter().map(|e| e.index).collect();
        order.remove(from);
        order.insert(position - 1, index);
        self.apply_order(&order)?;
        Ok(position as u32)
    }

    /// Builds entry `index` as early as possible: it and the chain of modules it
    /// hangs on move to the front, in chain order; the rest keep their order.
    /// Returns the module's new index.
    pub fn build_first(&mut self, index: u32) -> Result<u32> {
        let mut chain = vec![index];
        while let Some(p) = &self.entry(*chain.last().unwrap())?.predecessor {
            chain.push(p.index);
        }
        chain.reverse();
        let rest = self.plan.entries.iter().map(|e| e.index).filter(|i| !chain.contains(i));
        let order: Vec<u32> = chain.iter().copied().chain(rest).collect();
        self.apply_order(&order)?;
        Ok(chain.len() as u32)
    }

    /// Builds entry `index` as late as possible: it and everything attached to
    /// it move to the end, keeping their order; the rest keep theirs. Returns
    /// the module's new index.
    pub fn build_last(&mut self, index: u32) -> Result<u32> {
        self.entry(index)?;
        let moved = self.subtree(index);
        let (tail, head): (Vec<u32>, Vec<u32>) = self.plan.entries.iter().map(|e| e.index).partition(|i| moved.contains(i));
        let new_index = head.len() + tail.iter().position(|&i| i == index).unwrap() + 1;
        self.apply_order(&[head, tail].concat())?;
        Ok(new_index as u32)
    }

    /// Marks entry `index` as the last of a build stage (`bookmark="1"`), or not.
    pub fn set_stage_end(&mut self, index: u32, end: bool) -> Result<()> {
        self.entry(index)?;
        self.checkpoint();
        let e = self.plan.entries.iter_mut().find(|e| e.index == index).unwrap();
        e.bookmark = end.then(|| "1".to_owned());
        Ok(())
    }

    fn position_of(&self, index: u32) -> Result<usize> {
        self.plan.entries.iter().position(|e| e.index == index).ok_or_else(|| Error(format!("no entry {index}")))
    }

    /// Reorders the entries to `order` (current indices) if every module still
    /// comes after its predecessor, then renumbers them `1..n`.
    fn apply_order(&mut self, order: &[u32]) -> Result<()> {
        let rank = |i: u32| order.iter().position(|&o| o == i);
        for e in &self.plan.entries {
            if let Some(p) = &e.predecessor {
                if rank(p.index) > rank(e.index) {
                    return Err(Error(format!("module {} would be built before {}, which it attaches to", e.index, p.index)));
                }
            }
        }
        self.checkpoint();
        let mut entries = std::mem::take(&mut self.plan.entries);
        entries.sort_by_key(|e| rank(e.index));
        self.plan.entries = entries;
        self.renumber();
        Ok(())
    }

    /// Snaps of entry `index` that nothing uses, where new modules can attach.
    pub fn free_snaps(&self, catalog: &Catalog, index: u32) -> Result<Vec<FreeSnap>> {
        let world = self.world(index)?;
        let module = catalog.module(&self.entry(index)?.macro_name)?;
        Ok(module
            .snaps
            .iter()
            .filter(|s| !self.snap_in_use(index, &s.name))
            .map(|s| FreeSnap { name: s.name.clone(), world: world.then(&s.transform) })
            .collect())
    }

    /// Station-frame transform of entry `index`.
    pub fn world(&self, index: u32) -> Result<Transform> {
        Ok(Transform::from_offset(self.entry(index)?.offset.as_ref()))
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) -> bool {
        let Some(previous) = self.undo.pop() else { return false };
        self.redo.push(std::mem::replace(&mut self.plan, previous));
        true
    }

    pub fn redo(&mut self) -> bool {
        let Some(next) = self.redo.pop() else { return false };
        self.undo.push(std::mem::replace(&mut self.plan, next));
        true
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.plan.clone());
        self.redo.clear();
    }

    fn entry(&self, index: u32) -> Result<&Entry> {
        self.plan.entries.iter().find(|e| e.index == index).ok_or_else(|| Error(format!("no entry {index}")))
    }

    fn next_index(&self) -> u32 {
        self.plan.entries.iter().map(|e| e.index).max().unwrap_or(0) + 1
    }

    /// Whether snap `snap` of entry `index` links it to its predecessor or holds a child.
    fn snap_in_use(&self, index: u32, snap: &str) -> bool {
        self.plan.entries.iter().any(|e| {
            (e.index == index && e.connection.as_deref().is_some_and(|c| c.eq_ignore_ascii_case(snap)))
                || e.predecessor.as_ref().is_some_and(|p| p.index == index && p.connection.eq_ignore_ascii_case(snap))
        })
    }

    /// `index` and all entries attached to it, directly or not.
    fn subtree(&self, index: u32) -> Vec<u32> {
        let mut found = vec![index];
        let mut i = 0;
        while i < found.len() {
            let parent = found[i];
            found.extend(self.plan.entries.iter().filter(|e| e.predecessor.as_ref().is_some_and(|p| p.index == parent)).map(|e| e.index));
            i += 1;
        }
        found
    }

    /// Moves entry `index` to `target`, carrying its subtree rigidly. `index` must exist.
    fn move_subtree(&mut self, index: u32, target: &Transform) {
        let current = Transform::from_offset(self.plan.entries.iter().find(|e| e.index == index).and_then(|e| e.offset.as_ref()));
        let delta = target.then(&current.inverse());
        let moved = self.subtree(index);
        for e in self.plan.entries.iter_mut().filter(|e| moved.contains(&e.index)) {
            e.offset = Some(delta.then(&Transform::from_offset(e.offset.as_ref())).to_offset());
        }
    }

    /// Entries become `1..n` in their current order; predecessor links follow.
    fn renumber(&mut self) {
        let map: Vec<(u32, u32)> = self.plan.entries.iter().zip(1..).map(|(e, n)| (e.index, n)).collect();
        let new_index = |old: u32| map.iter().find(|(o, _)| *o == old).map_or(old, |(_, n)| *n);
        for e in &mut self.plan.entries {
            e.index = new_index(e.index);
            if let Some(p) = &mut e.predecessor {
                p.index = new_index(p.index);
            }
        }
    }
}

fn find_snap(module: &Arc<Module>, name: &str) -> Result<Snap> {
    module.snap(name).cloned().ok_or_else(|| Error(format!("{} has no snap {name}", module.macro_name)))
}

fn new_entry(index: u32, module: &Arc<Module>, link: Option<(String, Predecessor)>, at: Transform) -> Entry {
    let (connection, predecessor) = link.map_or((None, None), |(c, p)| (Some(c), Some(p)));
    Entry {
        index,
        macro_name: module.macro_name.clone(),
        connection,
        bookmark: None,
        fixed: None,
        predecessor,
        offset: Some(at.to_offset()),
        upgrades: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{fit, Axis, Quat};

    fn snap(name: &str, position: [f64; 3], yaw: f64) -> Snap {
        Snap { name: name.to_owned(), transform: Transform { position, rotation: Quat::axis_angle(Axis::Y, yaw) } }
    }

    /// A straight 800 m beam (like struct_arg_base_02) and a cross with four side snaps.
    fn catalog() -> Catalog {
        let module = |name: &str, snaps| Module {
            macro_name: name.to_owned(),
            class: "connectionmodule".to_owned(),
            component: name.to_owned(),
            source: None,
            snaps,
            shape: Vec::new(),
            parts: Vec::new(),
        };
        Catalog::from_modules(vec![
            module("beam", vec![snap("s1", [0.0, 0.0, 400.0], 0.0), snap("s2", [0.0, 0.0, -400.0], 180.0)]),
            module("cross", vec![
                snap("n", [0.0, 0.0, 200.0], 0.0),
                snap("s", [0.0, 0.0, -200.0], 180.0),
                snap("e", [200.0, 0.0, 0.0], 90.0),
                snap("w", [-200.0, 0.0, 0.0], -90.0),
            ]),
        ])
    }

    fn close(a: [f64; 3], b: [f64; 3]) -> bool {
        (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3)
    }

    fn assert_attached(d: &Design, c: &Catalog, index: u32) {
        let e = d.plan().entries.iter().find(|e| e.index == index).unwrap();
        let p = e.predecessor.as_ref().unwrap();
        let ps = c.module(&d.plan().entries.iter().find(|x| x.index == p.index).unwrap().macro_name).unwrap();
        let cs = c.module(&e.macro_name).unwrap();
        let f = fit(&d.world(p.index).unwrap(), &ps.snap(&p.connection).unwrap().transform,
                    &d.world(index).unwrap(), &cs.snap(e.connection.as_ref().unwrap()).unwrap().transform);
        assert!(f.gap < 1e-3 && f.misalignment < 1e-3, "entry {index}: {f:?}");
    }

    #[test]
    fn attach_places_modules_on_snaps() {
        let c = catalog();
        let mut d = Design::new("test");
        let root = d.add_root(&c, "beam", Transform::IDENTITY).unwrap();
        let cross = d.attach(&c, "cross", "s", root, "s1", 0.0).unwrap();
        assert!(close(d.world(cross).unwrap().position, [0.0, 0.0, 600.0]));
        let side = d.attach(&c, "beam", "s2", cross, "e", 0.0).unwrap();
        assert!(close(d.world(side).unwrap().position, [600.0, 0.0, 600.0]));
        assert_attached(&d, &c, cross);
        assert_attached(&d, &c, side);
        assert_eq!(d.free_snaps(&c, cross).unwrap().iter().map(|s| s.name.as_str()).collect::<Vec<_>>(), ["n", "w"]);
        assert!(d.attach(&c, "beam", "s2", cross, "e", 0.0).is_err(), "snap e is taken");
    }

    #[test]
    fn twist_carries_descendants() {
        let c = catalog();
        let mut d = Design::new("test");
        let root = d.add_root(&c, "beam", Transform::IDENTITY).unwrap();
        let cross = d.attach(&c, "cross", "s", root, "s1", 0.0).unwrap();
        let side = d.attach(&c, "beam", "s2", cross, "e", 0.0).unwrap();
        d.set_twist(&c, cross, 90.0).unwrap();
        // The cross turned 90° around the beam axis (Z): its east arm, and the side
        // beam on it, moved from (600, 0, 600) to (0, ±600, 600).
        let p = d.world(side).unwrap().position;
        assert!(p[0].abs() < 1e-3 && (p[1].abs() - 600.0).abs() < 1e-3 && (p[2] - 600.0).abs() < 1e-3, "{p:?}");
        assert_attached(&d, &c, cross);
        assert_attached(&d, &c, side);
    }

    #[test]
    fn move_root_carries_everything() {
        let c = catalog();
        let mut d = Design::new("test");
        let root = d.add_root(&c, "beam", Transform::IDENTITY).unwrap();
        let cross = d.attach(&c, "cross", "s", root, "s1", 0.0).unwrap();
        let at = Transform { position: [1000.0, 0.0, 0.0], rotation: Quat::axis_angle(Axis::Y, 90.0) };
        d.move_root(root, at).unwrap();
        assert!(close(d.world(cross).unwrap().position, [1600.0, 0.0, 0.0]));
        assert_attached(&d, &c, cross);
        assert!(d.move_root(cross, at).is_err(), "attached modules move with their root");
    }

    /// 1 beam, 2 cross on it, 3 beam on the cross, 4 beam on beam 1's other end.
    fn small_station(c: &Catalog) -> Design {
        let mut d = Design::new("order");
        let root = d.add_root(c, "beam", Transform::IDENTITY).unwrap();
        let cross = d.attach(c, "cross", "s", root, "s1", 0.0).unwrap();
        d.attach(c, "beam", "s2", cross, "e", 0.0).unwrap();
        d.attach(c, "beam", "s1", root, "s2", 0.0).unwrap();
        d
    }

    fn order(d: &Design) -> Vec<(u32, String, Option<u32>)> {
        d.plan().entries.iter().map(|e| (e.index, e.macro_name.clone(), e.predecessor.as_ref().map(|p| p.index))).collect()
    }

    #[test]
    fn build_order_moves_respect_attachments() {
        let c = catalog();
        let mut d = small_station(&c);
        let placed = |d: &Design, macro_name: &str, pred: Option<u32>| {
            d.plan().entries.iter().find(|e| e.macro_name == macro_name && e.predecessor.as_ref().map(|p| p.index) == pred).map(|e| e.index)
        };
        // Beam 4 hangs on the root: it may move to position 2, before the cross.
        assert_eq!(d.move_in_build_order(4, 2).unwrap(), 2);
        assert_eq!(order(&d)[1], (2, "beam".to_owned(), Some(1)), "{:?}", order(&d));
        assert_eq!(placed(&d, "cross", Some(1)), Some(3));
        // The cross can't go before the root it attaches to.
        assert!(d.move_in_build_order(3, 1).is_err());
        // Build last: the cross and the beam on it go to the end, renumbered.
        assert_eq!(d.build_last(3).unwrap(), 3, "the cross is first of the moved pair");
        assert_eq!(order(&d).iter().map(|o| o.0).collect::<Vec<_>>(), [1, 2, 3, 4]);
        assert_eq!(order(&d)[2].1, "cross");
        assert_eq!(order(&d)[3].2, Some(3), "the side beam still points at the cross");
        // Build first: the side beam and the chain it hangs on (root, cross) come first.
        assert_eq!(d.build_first(4).unwrap(), 3, "root, cross, then the side beam");
        assert_eq!(order(&d).iter().map(|o| o.1.as_str()).collect::<Vec<_>>(), ["beam", "cross", "beam", "beam"]);
        assert_eq!(order(&d)[2].2, Some(2), "the side beam right after its cross");
        assert!(order(&d).iter().all(|(i, _, p)| p.is_none_or(|p| p < *i)), "every module after its predecessor");
        for i in 2..=4 {
            assert_attached(&d, &c, i);
        }
        // Stages and undo.
        d.set_stage_end(2, true).unwrap();
        assert_eq!(d.plan().entries[1].bookmark.as_deref(), Some("1"));
        assert!(d.undo());
        assert_eq!(d.plan().entries[1].bookmark, None);
    }

    #[test]
    fn remove_renumbers_and_undo_restores() {
        let c = catalog();
        let mut d = Design::new("test");
        let root = d.add_root(&c, "beam", Transform::IDENTITY).unwrap();
        let cross = d.attach(&c, "cross", "s", root, "s1", 0.0).unwrap();
        d.attach(&c, "beam", "s2", cross, "e", 0.0).unwrap();
        let other = d.attach(&c, "beam", "s1", root, "s2", 0.0).unwrap();
        assert_eq!(other, 4);
        let before = d.plan().clone();

        assert_eq!(d.remove(&c, cross).unwrap(), 2);
        let indices: Vec<u32> = d.plan().entries.iter().map(|e| e.index).collect();
        assert_eq!(indices, [1, 2]);
        assert_eq!(d.plan().entries[1].predecessor.as_ref().unwrap().index, 1);
        assert_attached(&d, &c, 2);

        assert!(d.undo());
        assert_eq!(d.plan(), &before);
        assert!(d.redo());
        assert_eq!(d.plan().entries.len(), 2);
    }
}
