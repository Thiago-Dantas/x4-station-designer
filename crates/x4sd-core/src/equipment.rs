//! Equipping modules: turrets and shields, chosen per group (see
//! docs/x4-construction-plans.md, "Equipment"). `Design` applies the choices.

use crate::catalog::{size_of, Catalog, Equipment, Module};
use crate::plan::Upgrade;
use crate::planner::name_race;
use crate::Result;

/// The kinds of group equipment, as plans name them.
pub const KINDS: [&str; 2] = ["turrets", "shields"];

/// A loadout level, like the game's build-menu slider (our own picks; see `choose`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    None,
    Low,
    Medium,
    High,
}

/// How bulk equipping chooses for each group and kind.
#[derive(Debug, Clone, PartialEq)]
pub enum Fill {
    /// The first of these macros that fits all the slots (one per kind and size, say).
    Picks(Vec<String>),
    Level(Level),
}

/// What to do with one kind of slot in a group.
#[derive(Debug, Clone, PartialEq)]
pub enum Choice {
    /// Fill all the slots with this macro.
    Fill(String),
    /// Remove the equipment.
    Clear,
    /// Nothing fits: leave it as it is.
    Skip,
}

/// One kind of slot in a group of a placed module.
#[derive(Debug, Clone, PartialEq)]
pub struct GroupSlots<'a> {
    pub group: String,
    /// `turrets` or `shields`.
    pub kind: &'static str,
    /// `medium`, `large`, ...
    pub size: Option<String>,
    pub slots: u32,
    /// What is equipped: (macro, count).
    pub current: Option<(String, u32)>,
    /// Buildable equipment that fits every slot.
    pub fitting: Vec<&'a Equipment>,
}

/// Every group and kind of slot a module has, with what `upgrades` (its plan
/// entry's) put there and what fits, in component order.
pub fn module_groups<'a>(equipment: &'a [Equipment], module: &Module, upgrades: &[Upgrade]) -> Vec<GroupSlots<'a>> {
    let mut out = Vec::new();
    for group in module.equipment_groups() {
        for kind in KINDS {
            let slots: Vec<_> = module.group_slots(group, kind).collect();
            if slots.is_empty() {
                continue;
            }
            let current = upgrades
                .iter()
                .find(|u| u.kind == kind && u.group.eq_ignore_ascii_case(group))
                .map(|u| (u.macro_name.clone(), count(u)));
            out.push(GroupSlots {
                group: group.to_owned(),
                kind,
                size: size_of(&slots[0].tags).map(str::to_owned),
                slots: slots.len() as u32,
                current,
                fitting: equipment.iter().filter(|e| slots.iter().all(|s| e.fits(s))).collect(),
            });
        }
    }
    out
}

/// How many slots an upgrade fills (`exact`, else 1).
pub fn count(u: &Upgrade) -> u32 {
    u.exact.as_deref().and_then(|v| v.parse().ok()).unwrap_or(1)
}

/// The `exact` attribute for `count` of `slots` slots: the game writes it for
/// turrets always and for shields unless one of one slot is filled.
pub fn exact(kind: &str, count: u32, slots: u32) -> Option<String> {
    (kind == "turrets" || count != 1 || slots != 1).then(|| count.to_string())
}

/// The choice for one group and kind of a module under `fill`.
///
/// A level takes the fitting equipment without missiles (missile turrets need
/// ammunition supplied), the module's race first, and then the cheapest (low),
/// the middle (medium) or the most expensive (high) by average price. That is our
/// own rule: the game's `GenerateModuleLoadout` may pick differently.
pub fn choose(catalog: &Catalog, module: &Module, slots: &GroupSlots, fill: &Fill) -> Result<Choice> {
    let fits = |m: &str| slots.fitting.iter().any(|e| e.macro_name.eq_ignore_ascii_case(m));
    let level = match fill {
        Fill::Picks(macros) => return Ok(macros.iter().find(|m| fits(m)).map_or(Choice::Skip, |m| Choice::Fill(m.clone()))),
        Fill::Level(Level::None) => return Ok(Choice::Clear),
        Fill::Level(l) => *l,
    };
    let plain: Vec<&Equipment> = slots.fitting.iter().copied().filter(|e| !e.tags.iter().any(|t| t == "missile")).collect();
    let race = name_race(&module.macro_name);
    let own: Vec<&Equipment> = plain.iter().copied().filter(|e| race.is_some() && e.race == race).collect();
    let mut pool = if own.is_empty() { plain } else { own };
    if pool.is_empty() {
        return Ok(Choice::Skip);
    }
    let wares = catalog.wares()?;
    let price = |e: &Equipment| wares.ware(&e.ware).and_then(|w| w.average_price).unwrap_or(0.0);
    pool.sort_by(|a, b| price(a).total_cmp(&price(b)).then(a.macro_name.cmp(&b.macro_name)));
    let pick = match level {
        Level::Low => 0,
        Level::Medium => pool.len() / 2,
        _ => pool.len() - 1,
    };
    Ok(Choice::Fill(pool[pick].macro_name.clone()))
}
