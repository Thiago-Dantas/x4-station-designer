//! Wares (`libraries/wares.xml`): what modules and equipment cost to build.
//!
//! A module or equipment ware names its macro in `<component ref>` and has one
//! recipe (`<production>`) per build method, e.g. `default` and `terran`.
//! Extensions add wares and recipes through `<diff><add>` (see
//! tasks/workflow-build-cost.md). Their `<replace>`/`<remove>` patches (mods
//! changing prices or recipes) are not applied, and extension files that are
//! not valid XML are skipped (one installed mod ships a broken `wares.xml`).

use std::collections::{BTreeMap, HashMap};

use crate::plan::Plan;
use crate::text::Texts;
use crate::vfs::Vfs;
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct Recipe {
    pub method: String,
    /// Resources per build: (ware id, amount).
    pub resources: Vec<(String, f64)>,
    /// Seconds per production cycle.
    pub time: f64,
    /// Wares made per cycle.
    pub amount: f64,
    /// Output bonus with a full workforce (`<effect type="work" product>`), e.g. 0.43.
    pub work: f64,
    /// How much output follows the sector's sunlight (`<effect type="sunlight">`), 0..1.
    pub sunlight: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Ware {
    pub id: String,
    /// Display name (English).
    pub name: String,
    /// Average price in credits, if the ware is traded.
    pub average_price: Option<f64>,
    pub recipes: Vec<Recipe>,
    /// Cargo type it is stored as: `container`, `solid`, `liquid`, ...
    pub transport: String,
    /// m³ per unit.
    pub volume: f64,
    /// A raw material, brought in rather than made: mined (`tags="minable"`: ore, ice,
    /// hydrogen, ...) or salvaged scrap (`tags="recycling"`: scrap metal, raw scrap).
    pub raw: bool,
    /// `tags` attribute, e.g. `equipment turret`, `equipment noplayerblueprint shield`.
    pub tags: Vec<String>,
    /// The macro it builds (`<component ref>`), for modules and equipment.
    pub component: Option<String>,
}

/// A build method players can choose (the game's "Preferred Build Method").
#[derive(Debug, Clone, PartialEq)]
pub struct BuildMethod {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Default)]
pub struct Wares {
    wares: HashMap<String, Ware>,
    /// Lowercase macro -> ware id.
    by_macro: HashMap<String, String>,
    methods: Vec<BuildMethod>,
}

/// One resource of a cost.
#[derive(Debug, Clone, PartialEq)]
pub struct CostLine {
    pub ware: String,
    pub name: String,
    pub amount: f64,
    /// amount × average price, if the resource has a price.
    pub value: Option<f64>,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cost {
    /// Sorted by value, largest first.
    pub lines: Vec<CostLine>,
    /// Sum of the line values (average prices).
    pub value: f64,
    /// Macros without a known ware or recipe (left out of the totals).
    pub unknown: Vec<String>,
}

impl Wares {
    /// Reads `libraries/wares.xml` and the given extension folders' patches.
    pub fn load(vfs: &Vfs, extension_folders: &[String], texts: &Texts) -> Result<Wares> {
        let mut out = Wares::default();
        let mut tags: HashMap<String, String> = HashMap::new();
        let files = std::iter::once("libraries/wares.xml".to_owned())
            .chain(extension_folders.iter().map(|f| format!("extensions/{f}/libraries/wares.xml")));
        for file in files.filter(|f| vfs.contains(f)) {
            let xml = vfs.read_text(&file)?;
            let doc = match roxmltree::Document::parse(&xml) {
                Ok(doc) => doc,
                Err(_) if file.starts_with("extensions/") => continue,
                Err(e) => return Err(crate::Error(format!("{file}: {e}"))),
            };
            let root = doc.root_element();
            if root.has_tag_name("wares") {
                out.merge_wares(root, texts);
                if let Some(p) = root.children().find(|c| c.has_tag_name("production")) {
                    out.merge_methods(p, texts, &mut tags);
                }
                continue;
            }
            for add in root.children().filter(|c| c.has_tag_name("add")) {
                let sel = add.attribute("sel").unwrap_or_default().trim();
                if sel == "/wares" {
                    out.merge_wares(add, texts);
                } else if sel == "/wares/production" {
                    out.merge_methods(add, texts, &mut tags);
                } else if let Some(id) = sel.strip_prefix("/wares/ware[@id='").and_then(|r| r.strip_suffix("']")) {
                    if let Some(ware) = out.wares.get_mut(id) {
                        ware.recipes.extend(add.children().filter(|c| c.has_tag_name("production")).map(recipe));
                    }
                }
            }
        }
        out.methods.retain(|m| !tags.get(&m.id).is_some_and(|t| t.split_whitespace().any(|t| t == "noplayerbuild")));
        Ok(out)
    }

    fn merge_wares(&mut self, parent: roxmltree::Node, texts: &Texts) {
        for w in parent.children().filter(|c| c.has_tag_name("ware")) {
            let Some(id) = w.attribute("id") else { continue };
            let child = |tag: &str| w.children().find(|c| c.has_tag_name(tag));
            let component = child("component").and_then(|c| c.attribute("ref"));
            if let Some(m) = component {
                self.by_macro.insert(m.to_ascii_lowercase(), id.to_owned());
            }
            let tags: Vec<String> = w.attribute("tags").unwrap_or_default().split_whitespace().map(str::to_owned).collect();
            let ware = Ware {
                id: id.to_owned(),
                name: w.attribute("name").map_or_else(|| id.to_owned(), |n| texts.resolve(n)),
                average_price: child("price").and_then(|p| p.attribute("average")).and_then(|v| v.parse().ok()),
                recipes: w.children().filter(|c| c.has_tag_name("production")).map(recipe).collect(),
                transport: w.attribute("transport").unwrap_or_default().to_owned(),
                volume: w.attribute("volume").and_then(|v| v.parse().ok()).unwrap_or(0.0),
                raw: tags.iter().any(|t| t == "minable" || t == "recycling"),
                tags,
                component: component.map(str::to_owned),
            };
            self.wares.insert(id.to_owned(), ware);
        }
    }

    fn merge_methods(&mut self, parent: roxmltree::Node, texts: &Texts, tags: &mut HashMap<String, String>) {
        for m in parent.children().filter(|c| c.has_tag_name("method")) {
            let Some(id) = m.attribute("id") else { continue };
            tags.insert(id.to_owned(), m.attribute("tags").unwrap_or_default().to_owned());
            let name = m.attribute("name").map_or_else(|| id.to_owned(), |n| texts.resolve(n));
            self.methods.retain(|x| x.id != id);
            self.methods.push(BuildMethod { id: id.to_owned(), name });
        }
    }

    /// Build methods players can choose.
    pub fn methods(&self) -> &[BuildMethod] {
        &self.methods
    }

    pub fn ware(&self, id: &str) -> Option<&Ware> {
        self.wares.get(id)
    }

    /// Every ware.
    pub fn all(&self) -> impl Iterator<Item = &Ware> {
        self.wares.values()
    }

    /// The ware that builds this module or equipment macro.
    pub fn for_macro(&self, macro_name: &str) -> Option<&Ware> {
        self.wares.get(self.by_macro.get(&macro_name.to_ascii_lowercase())?)
    }

    /// The recipe for `method`, else the `default` one, else the first.
    pub fn recipe<'a>(&self, ware: &'a Ware, method: &str) -> Option<&'a Recipe> {
        let find = |m: &str| ware.recipes.iter().find(|r| r.method == m);
        find(method).or_else(|| find("default")).or(ware.recipes.first())
    }

    /// Resources to build `items` (macro, count) with `method`, summed.
    pub fn cost<'a>(&self, items: impl IntoIterator<Item = (&'a str, f64)>, method: &str) -> Cost {
        let mut totals: BTreeMap<String, f64> = BTreeMap::new();
        let mut unknown = Vec::new();
        for (macro_name, count) in items {
            match self.for_macro(macro_name).and_then(|w| self.recipe(w, method)) {
                Some(r) => r.resources.iter().for_each(|(ware, amount)| *totals.entry(ware.clone()).or_default() += amount * count),
                None => unknown.push(macro_name.to_owned()),
            }
        }
        let mut lines: Vec<CostLine> = totals
            .into_iter()
            .map(|(ware, amount)| {
                let known = self.wares.get(&ware);
                CostLine {
                    name: known.map_or_else(|| ware.clone(), |w| w.name.clone()),
                    value: known.and_then(|w| w.average_price).map(|p| p * amount),
                    ware,
                    amount,
                }
            })
            .collect();
        lines.sort_by(|a, b| b.value.unwrap_or(0.0).total_cmp(&a.value.unwrap_or(0.0)));
        unknown.sort();
        unknown.dedup();
        Cost { value: lines.iter().filter_map(|l| l.value).sum(), lines, unknown }
    }

    /// What a plan costs to build, or just entry `index` (module + its equipment).
    pub fn plan_cost(&self, plan: &Plan, index: Option<u32>, method: &str) -> Cost {
        let entries = plan.entries.iter().filter(|e| index.is_none_or(|i| e.index == i));
        let items = entries.flat_map(|e| {
            let equipment = e.upgrades.iter().map(|u| (u.macro_name.as_str(), u.exact.as_deref().and_then(|x| x.parse().ok()).unwrap_or(1.0)));
            std::iter::once((e.macro_name.as_str(), 1.0)).chain(equipment)
        });
        self.cost(items, method)
    }
}

fn recipe(p: roxmltree::Node) -> Recipe {
    let num = |n: roxmltree::Node, a: &str| n.attribute(a).and_then(|v| v.parse().ok()).unwrap_or(0.0);
    let effect = |kind: &str| {
        p.descendants().find(|e| e.has_tag_name("effect") && e.attribute("type") == Some(kind)).map_or(0.0, |e| num(e, "product"))
    };
    Recipe {
        method: p.attribute("method").unwrap_or("default").to_owned(),
        resources: p
            .descendants()
            .filter(|w| w.has_tag_name("ware"))
            .filter_map(|w| Some((w.attribute("ware")?.to_owned(), w.attribute("amount")?.parse().ok()?)))
            .collect(),
        time: num(p, "time"),
        amount: num(p, "amount"),
        work: effect("work"),
        sunlight: effect("sunlight"),
    }
}
