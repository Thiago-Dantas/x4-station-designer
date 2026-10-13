//! Station module catalogue: macro -> component -> snap connections, across the
//! base game, DLCs and enabled mods.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use crate::game::{find_extensions, load_vfs, Extension};
use crate::geometry::{Quat, Transform};
use crate::plan::{Patch, Plan};
use crate::text::Texts;
use crate::vfs::{normalize, Vfs};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Snap {
    /// Lowercase connection name, e.g. `connectionsnap001` (names vary: `connectionsnap_04`, `connectionsnap03`).
    pub name: String,
    /// Offset in the module's local frame.
    pub transform: Transform,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub macro_name: String,
    /// Macro class, e.g. `connectionmodule`, `production`, `dockarea`.
    pub class: String,
    pub component: String,
    /// Id of the extension whose index defines the macro; `None` for the base game.
    pub source: Option<String>,
    pub snaps: Vec<Snap>,
}

impl Module {
    pub fn snap(&self, name: &str) -> Option<&Snap> {
        self.snaps.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }
}

/// Macro classes of buildable station modules (the others under `assets/structures`
/// are landmarks, props and asteroids).
pub const STATION_MODULE_CLASSES: &[&str] = &[
    "connectionmodule", "storage", "production", "dockarea", "defencemodule", "habitation",
    "buildmodule", "pier", "welfaremodule", "processingmodule", "ventureplatform", "radar",
];

/// A buildable station module, for listings (palette, `list-modules`).
#[derive(Debug, Clone, PartialEq)]
pub struct ModuleInfo {
    pub macro_name: String,
    pub class: String,
    /// Display name (English).
    pub name: String,
    pub race: Option<String>,
    /// Id of the extension that defines it; `None` for the base game.
    pub source: Option<String>,
}

struct IndexEntry {
    path: String,
    source: Option<usize>,
}

pub struct Catalog {
    vfs: Vfs,
    extensions: Vec<Extension>,
    macros: HashMap<String, IndexEntry>,
    components: HashMap<String, IndexEntry>,
    texts: Texts,
    cache: Mutex<HashMap<String, Arc<Module>>>,
}

impl Catalog {
    /// Reads the base game, all extensions enabled in the profile, and their indexes.
    pub fn load(game_dir: &Path, profile_dir: Option<&Path>) -> Result<Catalog> {
        let extensions: Vec<Extension> =
            find_extensions(game_dir, profile_dir)?.into_iter().filter(|e| e.enabled).collect();
        let vfs = load_vfs(game_dir, &extensions)?;
        let mut catalog = Catalog {
            vfs,
            extensions,
            macros: HashMap::new(),
            components: HashMap::new(),
            texts: Texts::default(),
            cache: Mutex::new(HashMap::new()),
        };
        let text_files = std::iter::once("t/0001-l044.xml".to_owned())
            .chain(catalog.extensions.iter().map(|e| format!("extensions/{}/t/0001-l044.xml", e.folder)));
        for path in text_files.filter(|p| catalog.vfs.contains(p)).collect::<Vec<_>>() {
            let xml = catalog.vfs.read_text(&path)?;
            catalog.texts.merge(&xml).map_err(|e| Error(format!("{path}: {e}")))?;
        }
        for (kind, source) in [("macros", None), ("components", None)]
            .into_iter()
            .chain((0..catalog.extensions.len()).flat_map(|i| [("macros", Some(i)), ("components", Some(i))]))
        {
            let path = match source {
                None => format!("index/{kind}.xml"),
                Some(i) => format!("extensions/{}/index/{kind}.xml", catalog.extensions[i].folder),
            };
            if !catalog.vfs.contains(&path) {
                continue;
            }
            let xml = catalog.vfs.read_text(&path)?;
            let map = if kind == "macros" { &mut catalog.macros } else { &mut catalog.components };
            merge_index(map, &xml, source).map_err(|e| Error(format!("{path}: {e}")))?;
        }
        Ok(catalog)
    }

    /// A catalogue of just these modules, without game files: for tests and
    /// offline use. Index-based queries (`has_macro`, patches, listings) are empty.
    pub fn from_modules(modules: Vec<Module>) -> Catalog {
        let cache = modules.into_iter().map(|m| (m.macro_name.to_ascii_lowercase(), Arc::new(m))).collect();
        Catalog {
            vfs: Vfs::default(),
            extensions: Vec::new(),
            macros: HashMap::new(),
            components: HashMap::new(),
            texts: Texts::default(),
            cache: Mutex::new(cache),
        }
    }

    /// Enabled extensions, in load order.
    pub fn extensions(&self) -> &[Extension] {
        &self.extensions
    }

    /// Game texts (English), for resolving `{page,id}` names.
    pub fn texts(&self) -> &Texts {
        &self.texts
    }

    /// Every buildable station module: a structure macro of a station-module class
    /// that belongs to at least one build set. Sorted by macro name.
    pub fn station_modules(&self) -> Result<Vec<ModuleInfo>> {
        let mut out = Vec::new();
        for (key, entry) in &self.macros {
            let path = format!("{}.xml", entry.path);
            if !entry.path.contains("assets/structures/") || !self.vfs.contains(&path) {
                continue;
            }
            let xml = self.vfs.read_text(&path)?;
            let doc = roxmltree::Document::parse(&xml).map_err(|e| Error(format!("{path}: {e}")))?;
            let Some(m) = doc.descendants().find(|n| n.has_tag_name("macro")) else { continue };
            let class = m.attribute("class").unwrap_or_default();
            let props = m.children().find(|n| n.has_tag_name("properties"));
            if !STATION_MODULE_CLASSES.contains(&class) || child(child(props, "build"), "sets").is_none() {
                continue;
            }
            let ident = child(props, "identification");
            out.push(ModuleInfo {
                macro_name: m.attribute("name").unwrap_or(key).to_owned(),
                class: class.to_owned(),
                name: ident.and_then(|i| i.attribute("name")).map_or_else(|| key.clone(), |n| self.texts.resolve(n)),
                race: ident.and_then(|i| i.attribute("makerrace")).map(str::to_owned),
                source: entry.source.map(|i| self.extensions[i].id.clone()),
            });
        }
        out.sort_by(|a, b| a.macro_name.cmp(&b.macro_name));
        Ok(out)
    }

    pub fn module(&self, macro_name: &str) -> Result<Arc<Module>> {
        let key = macro_name.to_ascii_lowercase();
        if let Some(m) = self.cache.lock().unwrap().get(&key) {
            return Ok(m.clone());
        }
        let module = Arc::new(self.read_module(&key)?);
        self.cache.lock().unwrap().insert(key, module.clone());
        Ok(module)
    }

    /// Whether the index knows this macro (modules, equipment, anything).
    pub fn has_macro(&self, macro_name: &str) -> bool {
        self.macros.contains_key(&macro_name.to_ascii_lowercase())
    }

    /// The `<patches>` a plan needs: every extension that defines one of its
    /// module or equipment macros and that savegames depend on (`save`), in load order.
    pub fn required_patches(&self, plan: &Plan) -> Vec<Patch> {
        let mut used = vec![false; self.extensions.len()];
        let macros = plan.entries.iter().flat_map(|e| {
            std::iter::once(&e.macro_name).chain(e.upgrades.iter().map(|u| &u.macro_name))
        });
        for m in macros {
            if let Some(IndexEntry { source: Some(i), .. }) = self.macros.get(&m.to_ascii_lowercase()) {
                used[*i] = true;
            }
        }
        self.extensions
            .iter()
            .zip(used)
            .filter(|(e, used)| *used && e.save)
            .map(|(e, _)| Patch { extension: e.id.clone(), version: e.version.clone(), name: e.name.clone() })
            .collect()
    }

    fn read_module(&self, key: &str) -> Result<Module> {
        let entry = self.macros.get(key).ok_or_else(|| Error(format!("unknown macro: {key}")))?;
        let xml = self.vfs.read_text(&format!("{}.xml", entry.path))?;
        let doc = roxmltree::Document::parse(&xml)?;
        let macro_el = doc
            .descendants()
            .find(|n| n.has_tag_name("macro") && n.attribute("name").is_some_and(|v| v.eq_ignore_ascii_case(key)))
            .ok_or_else(|| Error(format!("{}: no <macro name=\"{key}\">", entry.path)))?;
        let component = macro_el
            .children()
            .find(|n| n.has_tag_name("component"))
            .and_then(|n| n.attribute("ref"))
            .ok_or_else(|| Error(format!("{key}: macro has no <component ref>")))?;

        let centry = self
            .components
            .get(&component.to_ascii_lowercase())
            .ok_or_else(|| Error(format!("{key}: unknown component {component}")))?;
        let cxml = self.vfs.read_text(&format!("{}.xml", centry.path))?;
        let cdoc = roxmltree::Document::parse(&cxml)?;
        let comp = cdoc
            .descendants()
            .find(|n| n.has_tag_name("component") && n.attribute("name").is_some_and(|v| v.eq_ignore_ascii_case(component)))
            .ok_or_else(|| Error(format!("{}: no <component name=\"{component}\">", centry.path)))?;
        let snaps = comp
            .descendants()
            .filter(|n| n.has_tag_name("connection"))
            .filter(|n| n.attribute("tags").is_some_and(|t| t.split_whitespace().any(|t| t == "snap")))
            .filter_map(|n| {
                let name = n.attribute("name")?.to_ascii_lowercase();
                let offset = n.children().find(|c| c.has_tag_name("offset"));
                Some(Snap { name, transform: offset.map_or(Transform::IDENTITY, parse_offset) })
            })
            .collect();

        Ok(Module {
            macro_name: macro_el.attribute("name").unwrap_or(key).to_owned(),
            class: macro_el.attribute("class").unwrap_or_default().to_owned(),
            component: component.to_owned(),
            source: entry.source.map(|i| self.extensions[i].id.clone()),
            snaps,
        })
    }
}

/// Adds the entries of an index file: a full `<index>`, or a `<diff>` that adds to
/// `/index`. Other diff operations (replace/remove of single entries) are not
/// used by any installed mod yet and are ignored.
fn merge_index(map: &mut HashMap<String, IndexEntry>, xml: &str, source: Option<usize>) -> Result<()> {
    let doc = roxmltree::Document::parse(xml)?;
    let root = doc.root_element();
    let parents: Vec<_> = match root.tag_name().name() {
        "index" => vec![root],
        "diff" => root
            .children()
            .filter(|n| n.has_tag_name("add") && n.attribute("sel").map(str::trim) == Some("/index"))
            .collect(),
        other => return Err(Error(format!("unexpected root <{other}>"))),
    };
    for entry in parents.iter().flat_map(|p| p.children()).filter(|n| n.has_tag_name("entry")) {
        if let (Some(name), Some(value)) = (entry.attribute("name"), entry.attribute("value")) {
            map.insert(name.to_ascii_lowercase(), IndexEntry { path: normalize(value), source });
        }
    }
    Ok(())
}

fn child<'a, 'i>(node: Option<roxmltree::Node<'a, 'i>>, tag: &str) -> Option<roxmltree::Node<'a, 'i>> {
    node.and_then(|n| n.children().find(|c| c.has_tag_name(tag)))
}

/// A component `<offset>`: position plus quaternion (or yaw/pitch/roll), missing values 0.
fn parse_offset(offset: roxmltree::Node) -> Transform {
    let num = |n: roxmltree::Node, a: &str| n.attribute(a).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let mut t = Transform::IDENTITY;
    for c in offset.children().filter(|c| c.is_element()) {
        match c.tag_name().name() {
            "position" => t.position = [num(c, "x"), num(c, "y"), num(c, "z")],
            "quaternion" => t.rotation = Quat { w: num(c, "qw"), x: num(c, "qx"), y: num(c, "qy"), z: num(c, "qz") },
            "rotation" => t.rotation = Quat::from_euler(num(c, "yaw"), num(c, "pitch"), num(c, "roll")),
            _ => {}
        }
    }
    t
}
