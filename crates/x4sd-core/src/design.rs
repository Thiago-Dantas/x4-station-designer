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
use crate::equipment::{choose, exact, module_groups, Choice, Fill};
use crate::geometry::{attach as attach_transform, Axis, Quat, Transform};
use crate::import::new_plan_id;
use crate::plan::{Entry, Plan, Predecessor, Upgrade};
use crate::plot::{required_plot, Plot};
use crate::{Error, Result};

pub struct Design {
    plan: Plan,
    undo: Vec<Plan>,
    redo: Vec<Plan>,
    /// The plot the station must fit in. Not part of the plan file (the game keeps
    /// plots per station), and not undone with edits.
    plot: Option<Plot>,
    /// The plan as last saved, exported or opened: unsaved changes are differences from it.
    saved: Plan,
}

/// A snap point of a placed module that nothing uses, in the station frame.
#[derive(Debug, Clone, PartialEq)]
pub struct FreeSnap {
    pub name: String,
    pub world: Transform,
}

impl Design {
    /// An empty design on a plot reaching 2 km on every side (4×4×4 km), the game's default.
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
        design.plot = Some(Plot::around(2));
        design
    }

    /// A design for an existing plan, without a plot (see `fit_plot`).
    pub fn from_plan(plan: Plan) -> Design {
        Design { saved: plan.clone(), plan, undo: Vec::new(), redo: Vec::new(), plot: None }
    }

    /// Whether the plan differs from what was last saved, exported or opened
    /// (undoing back to that state counts as unchanged).
    pub fn is_dirty(&self) -> bool {
        self.plan != self.saved
    }

    /// Call after writing the plan somewhere (plan file or the game's import folder).
    pub fn mark_saved(&mut self) {
        self.saved = self.plan.clone();
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
    /// attached to it moves along. A tilt (see `rotate_attached`) is kept.
    pub fn set_twist(&mut self, catalog: &Catalog, index: u32, twist_degrees: f64) -> Result<()> {
        let (joint, child_snap) = self.joint(catalog, index)?;
        let tilt = tilt_of(&joint, &self.world(index)?, &child_snap);
        let target = at_joint(&joint, tilt, &child_snap, twist_degrees);
        self.checkpoint();
        self.move_subtree(index, &target);
        Ok(())
    }

    /// Turns attached entry `index`, with everything attached to it, about its
    /// joint (where its snap meets its predecessor's) to the station-frame
    /// `rotation`. It stays linked: a turn around the snap axis is a twist, any
    /// other turn tilts it off the snap axis, which the game's build menu never does.
    pub fn rotate_attached(&mut self, catalog: &Catalog, index: u32, rotation: Quat) -> Result<()> {
        let (joint, child_snap) = self.joint(catalog, index)?;
        let rotation = rotation.normalized();
        let offset = rotation.rotate(child_snap.position);
        let target = Transform { position: [0, 1, 2].map(|i| joint.position[i] - offset[i]), rotation };
        self.checkpoint();
        self.move_subtree(index, &target);
        Ok(())
    }

    /// Takes the tilt off attached entry `index` (its snap axis lines up with its
    /// predecessor's again), keeping its twist; everything attached to it moves along.
    pub fn straighten(&mut self, catalog: &Catalog, index: u32) -> Result<()> {
        let (joint, child_snap) = self.joint(catalog, index)?;
        let twist = crate::geometry::fit(&joint, &Transform::IDENTITY, &self.world(index)?, &child_snap).twist;
        let target = at_joint(&joint, Quat::IDENTITY, &child_snap, twist);
        self.checkpoint();
        self.move_subtree(index, &target);
        Ok(())
    }

    /// Where attached entry `index` joins its predecessor: the predecessor's snap
    /// frame in the station frame, and the entry's own snap (module frame).
    fn joint(&self, catalog: &Catalog, index: u32) -> Result<(Transform, Transform)> {
        let e = self.entry(index)?;
        let (Some(conn), Some(pred)) = (&e.connection, &e.predecessor) else {
            return Err(Error(format!("entry {index} is not attached to anything; move it instead")));
        };
        let child_snap = find_snap(&catalog.module(&e.macro_name)?, conn)?;
        let pred_snap = find_snap(&catalog.module(&self.entry(pred.index)?.macro_name)?, &pred.connection)?;
        Ok((self.world(pred.index)?.then(&pred_snap.transform), child_snap.transform))
    }

    /// Moves an attached or free-standing module, with everything attached to it,
    /// so its snap `snap` connects to snap `to_snap` of entry `to`, twisted by
    /// `twist_degrees`. The target can't be the module itself or attached to it.
    /// If `to` comes later in the build order, the moved modules are re-sequenced
    /// right after it. Returns the module's (possibly new) index.
    pub fn reattach(&mut self, catalog: &Catalog, index: u32, snap: &str, to: u32, to_snap: &str, twist_degrees: f64) -> Result<u32> {
        let moved = self.subtree(index);
        if moved.contains(&to) {
            return Err(Error(format!("module {index} can't attach to itself or to a module attached to it")));
        }
        let entry = self.entry(index)?;
        let child_snap = find_snap(&catalog.module(&entry.macro_name)?, snap)?;
        let pred_snap = find_snap(&catalog.module(&self.entry(to)?.macro_name)?, to_snap)?;
        let current_link = entry.predecessor.as_ref().is_some_and(|p| p.index == to && p.connection.eq_ignore_ascii_case(&pred_snap.name));
        if self.snap_in_use(to, &pred_snap.name) && !current_link {
            return Err(Error(format!("snap {} of entry {to} is already in use", pred_snap.name)));
        }
        let used_by_children = self.plan.entries.iter().any(|e| {
            e.predecessor.as_ref().is_some_and(|p| p.index == index && p.connection.eq_ignore_ascii_case(&child_snap.name))
        });
        if used_by_children {
            return Err(Error(format!("snap {} of module {index} holds an attached module", child_snap.name)));
        }
        let target = attach_transform(&self.world(to)?, &pred_snap.transform, &child_snap.transform, twist_degrees);
        self.checkpoint();
        self.move_subtree(index, &target);
        let e = self.plan.entries.iter_mut().find(|e| e.index == index).unwrap();
        e.connection = Some(child_snap.name.clone());
        e.predecessor = Some(Predecessor { index: to, connection: pred_snap.name.clone() });
        // Keep the build order valid: the moved modules come after their new parent.
        let order = self.order_after(&moved, to);
        let new_index = order.iter().position(|&i| i == index).unwrap() as u32 + 1;
        self.sort_and_renumber(&order);
        Ok(new_index)
    }

    /// Makes an attached module free-standing where it is (its link is dropped;
    /// whatever is attached to it stays attached).
    pub fn detach(&mut self, index: u32) -> Result<()> {
        if self.entry(index)?.predecessor.is_none() {
            return Err(Error(format!("module {index} is not attached to anything")));
        }
        self.checkpoint();
        let e = self.plan.entries.iter_mut().find(|e| e.index == index).unwrap();
        e.connection = None;
        e.predecessor = None;
        Ok(())
    }

    /// The current order with `moved` placed right after `after` when they came
    /// before it; otherwise unchanged.
    fn order_after(&self, moved: &[u32], after: u32) -> Vec<u32> {
        let current: Vec<u32> = self.plan.entries.iter().map(|e| e.index).collect();
        let first_moved = current.iter().position(|i| moved.contains(i)).unwrap_or(0);
        let parent = current.iter().position(|&i| i == after).unwrap_or(0);
        if parent < first_moved {
            return current;
        }
        let rest: Vec<u32> = current.iter().copied().filter(|i| !moved.contains(i)).collect();
        let cut = rest.iter().position(|&i| i == after).map_or(rest.len(), |p| p + 1);
        let block: Vec<u32> = current.iter().copied().filter(|i| moved.contains(i)).collect();
        [&rest[..cut], &block[..], &rest[cut..]].concat()
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

    /// Equips one kind of slot (`turrets` or `shields`) in a group of entry `index`:
    /// `Some((macro, count))` puts `count` of that equipment on its slots (it must fit
    /// all of them), `None` removes what is there.
    pub fn set_equipment(&mut self, catalog: &Catalog, index: u32, group: &str, kind: &str, equip: Option<(&str, u32)>) -> Result<()> {
        let module = catalog.module(&self.entry(index)?.macro_name)?;
        let slots: Vec<_> = module.group_slots(group, kind).collect();
        if slots.is_empty() {
            return Err(Error(format!("module {index} ({}) has no {kind} in group {group}", module.macro_name)));
        }
        let group = slots[0].group.clone();
        let choice = match equip {
            Some((macro_name, count)) => {
                let e = catalog
                    .equipment()?
                    .iter()
                    .find(|e| e.macro_name.eq_ignore_ascii_case(macro_name))
                    .ok_or_else(|| Error(format!("{macro_name} is not buildable equipment")))?;
                if !slots.iter().all(|s| e.fits(s)) {
                    return Err(Error(format!("{macro_name} doesn't fit the {kind} of group {group}")));
                }
                if count == 0 || count as usize > slots.len() {
                    return Err(Error(format!("group {group} has {} {kind} slots; {count} can't go there", slots.len())));
                }
                Some((e.macro_name.clone(), count, slots.len() as u32))
            }
            None => None,
        };
        self.checkpoint();
        put_upgrade(&mut self.entry_mut(index).upgrades, &group, kind, choice);
        Ok(())
    }

    /// Equips every group of the given entries as `fill` chooses, as one undo step.
    /// Returns how many group slots of a kind were filled, cleared and skipped
    /// (nothing fits).
    pub fn equip_modules(&mut self, catalog: &Catalog, indices: &[u32], fill: &Fill) -> Result<(usize, usize, usize)> {
        let equipment = catalog.equipment()?;
        let mut changes = Vec::new();
        let (mut filled, mut cleared, mut skipped) = (0, 0, 0);
        for &index in indices {
            let e = self.entry(index)?;
            let module = catalog.module(&e.macro_name)?;
            for slots in module_groups(equipment, &module, &e.upgrades) {
                match choose(catalog, &module, &slots, fill)? {
                    Choice::Fill(m) => {
                        filled += 1;
                        changes.push((index, slots.group, slots.kind, Some((m, slots.slots, slots.slots))));
                    }
                    Choice::Clear => {
                        cleared += 1;
                        changes.push((index, slots.group, slots.kind, None));
                    }
                    Choice::Skip => skipped += 1,
                }
            }
        }
        if !changes.is_empty() {
            self.checkpoint();
            for (index, group, kind, choice) in changes {
                put_upgrade(&mut self.entry_mut(index).upgrades, &group, kind, choice);
            }
        }
        Ok((filled, cleared, skipped))
    }

    /// Gives every other module of the same type the equipment of entry `index`, as
    /// one undo step. Returns how many modules changed.
    pub fn copy_equipment(&mut self, index: u32) -> Result<usize> {
        let source = self.entry(index)?.clone();
        let targets: Vec<u32> = self
            .plan
            .entries
            .iter()
            .filter(|e| e.index != index && e.macro_name.eq_ignore_ascii_case(&source.macro_name) && e.upgrades != source.upgrades)
            .map(|e| e.index)
            .collect();
        if !targets.is_empty() {
            self.checkpoint();
            for t in &targets {
                self.entry_mut(*t).upgrades = source.upgrades.clone();
            }
        }
        Ok(targets.len())
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
        self.sort_and_renumber(order);
        Ok(())
    }

    /// Puts the entries in `order` (current indices) and renumbers them `1..n`.
    fn sort_and_renumber(&mut self, order: &[u32]) {
        let rank = |i: u32| order.iter().position(|&o| o == i);
        let mut entries = std::mem::take(&mut self.plan.entries);
        entries.sort_by_key(|e| rank(e.index));
        self.plan.entries = entries;
        self.renumber();
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

    /// How many edits can be undone.
    pub fn history_len(&self) -> usize {
        self.undo.len()
    }

    /// Turns the edits made since the history had `since` steps into one undo step.
    pub fn squash_history(&mut self, since: usize) {
        self.undo.truncate(since + 1);
    }

    fn checkpoint(&mut self) {
        self.undo.push(self.plan.clone());
        self.redo.clear();
    }

    fn entry(&self, index: u32) -> Result<&Entry> {
        self.plan.entries.iter().find(|e| e.index == index).ok_or_else(|| Error(format!("no entry {index}")))
    }

    /// Gives entry `index` this equipment as it is (no checks, no undo step of its
    /// own): for auto-placement, which puts a lifted dock back with its equipment.
    pub(crate) fn restore_upgrades(&mut self, index: u32, upgrades: Vec<Upgrade>) {
        self.entry_mut(index).upgrades = upgrades;
    }

    /// Entry `index`, which the caller has checked exists.
    fn entry_mut(&mut self, index: u32) -> &mut Entry {
        self.plan.entries.iter_mut().find(|e| e.index == index).unwrap()
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

/// Sets (or with `None` removes) the equipment of one kind in a group: (macro,
/// count, slots). A replaced upgrade keeps its place and `path`.
fn put_upgrade(upgrades: &mut Vec<Upgrade>, group: &str, kind: &str, choice: Option<(String, u32, u32)>) {
    let at = upgrades.iter().position(|u| u.kind == kind && u.group.eq_ignore_ascii_case(group));
    let old = at.map(|i| upgrades.remove(i));
    if let Some((macro_name, count, slots)) = choice {
        let new = Upgrade {
            kind: kind.to_owned(),
            macro_name,
            path: old.and_then(|o| o.path),
            group: group.to_owned(),
            exact: exact(kind, count, slots),
        };
        upgrades.insert(at.unwrap_or(upgrades.len()), new);
    }
}

/// A module at a joint (its predecessor's snap frame): its snap meets the joint's
/// point, facing it, tilted by `tilt` (in the joint's frame) and twisted around
/// its own snap axis. With no tilt this is `geometry::attach`.
fn at_joint(joint: &Transform, tilt: Quat, child_snap: &Transform, twist_degrees: f64) -> Transform {
    let snap = joint.rotation * tilt * Quat::axis_angle(Axis::Y, 180.0) * Quat::axis_angle(Axis::Z, twist_degrees);
    let rotation = snap * child_snap.rotation.conj();
    let offset = rotation.rotate(child_snap.position);
    Transform { position: [0, 1, 2].map(|i| joint.position[i] - offset[i]), rotation }
}

/// The tilt of a module at `world` attached by `child_snap` at `joint`: the part of
/// its turn (in the joint's frame) that isn't around the snap axis (swing-twist
/// decomposition about Z). Identity for a straight joint.
fn tilt_of(joint: &Transform, world: &Transform, child_snap: &Transform) -> Quat {
    let snap = world.then(child_snap).rotation;
    let q = joint.rotation.conj() * snap * Quat::axis_angle(Axis::Y, 180.0).conj();
    let n = (q.w * q.w + q.z * q.z).sqrt();
    if n < 1e-9 {
        return q; // flipped right over: all tilt
    }
    q * Quat { w: q.w / n, x: 0.0, y: 0.0, z: q.z / n }.conj()
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
            collision: Vec::new(),
            equipment_slots: Vec::new(),
            economy: Default::default(),
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
    fn tilt_keeps_the_joint_and_survives_twists() {
        let c = catalog();
        let mut d = Design::new("test");
        let root = d.add_root(&c, "beam", Transform::IDENTITY).unwrap();
        let cross = d.attach(&c, "cross", "s", root, "s1", 0.0).unwrap();
        let side = d.attach(&c, "beam", "s2", cross, "e", 0.0).unwrap();
        let measure = |d: &Design| {
            let s = c.module("cross").unwrap().snap("s").unwrap().transform;
            fit(&d.world(root).unwrap(), &c.module("beam").unwrap().snap("s1").unwrap().transform, &d.world(cross).unwrap(), &s)
        };
        // Tilt the cross 30° about x: its snap point stays on the beam's, the axis is 30° off
        // (within f32 rounding of the stored offsets).
        let tilted = Quat::axis_angle(Axis::X, 30.0) * d.world(cross).unwrap().rotation;
        d.rotate_attached(&c, cross, tilted).unwrap();
        let f = measure(&d);
        assert!(f.gap < 1e-3 && (f.misalignment - 30.0).abs() < 1e-3, "{f:?}");
        assert_attached(&d, &c, side); // what hangs on it moved along
        // A twist keeps the tilt; straightening keeps the twist.
        d.set_twist(&c, cross, 90.0).unwrap();
        let f = measure(&d);
        assert!(f.gap < 1e-3 && (f.misalignment - 30.0).abs() < 1e-3 && (f.twist - 90.0).abs() < 1e-3, "{f:?}");
        d.straighten(&c, cross).unwrap();
        let f = measure(&d);
        assert!(f.gap < 1e-3 && f.misalignment < 1e-3 && (f.twist - 90.0).abs() < 1e-3, "{f:?}");
        assert_attached(&d, &c, cross);
        assert_attached(&d, &c, side);
        assert!(d.rotate_attached(&c, root, Quat::IDENTITY).is_err(), "the root isn't attached");
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
    fn reattach_moves_subtree_and_keeps_order_valid() {
        let c = catalog();
        // 1 beam, 2 cross on it, 3 beam on the cross's east arm, 4 beam on beam 1's other end.
        let mut d = small_station(&c);
        // Move the cross (with beam 3 on it) to the end of beam 4: beam 4 comes later
        // in the order, so the cross and beam 3 are re-sequenced after it.
        let free_end = d.free_snaps(&c, 4).unwrap()[0].name.clone();
        let new_index = d.reattach(&c, 2, "s", 4, &free_end, 0.0).unwrap();
        assert_eq!(new_index, 3);
        assert_eq!(order(&d).iter().map(|o| (o.1.as_str(), o.2)).collect::<Vec<_>>(),
                   [("beam", None), ("beam", Some(1)), ("cross", Some(2)), ("beam", Some(3))]);
        for i in 2..=4 {
            assert_attached(&d, &c, i);
        }
        // Not onto itself or something attached to it.
        assert!(d.reattach(&c, 3, "s", 4, "w", 0.0).is_err());
        // Detach: the module stays where it is, free-standing.
        let at = d.world(3).unwrap();
        d.detach(3).unwrap();
        assert!(d.plan().entries[2].predecessor.is_none() && d.world(3).unwrap() == at);
        assert_attached(&d, &c, 4);
        // Undo twice: back to the original station.
        assert!(d.undo() && d.undo());
        assert_eq!(order(&d), order(&small_station(&c)));
    }

    #[test]
    fn dirty_tracks_changes_since_saved() {
        let c = catalog();
        let mut d = Design::new("dirty");
        assert!(!d.is_dirty());
        d.add_root(&c, "beam", Transform::IDENTITY).unwrap();
        assert!(d.is_dirty());
        d.mark_saved();
        assert!(!d.is_dirty());
        d.rename("other");
        assert!(d.is_dirty());
        d.undo();
        assert!(!d.is_dirty(), "undoing back to the saved state counts as unchanged");
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
