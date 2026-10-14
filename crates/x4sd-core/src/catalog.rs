//! Station module catalogue: macro -> component -> snap connections, across the
//! base game, DLCs and enabled mods.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use crate::game::{find_extensions, load_vfs, Extension};
use crate::geometry::{Quat, Transform};
use crate::mesh::{self, Mesh};
use crate::plan::{Patch, Plan};
use crate::text::Texts;
use crate::texture::{self, Texture};
use crate::wares::Wares;
use crate::vfs::{normalize, Vfs};
use crate::{Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub struct Snap {
    /// Lowercase connection name, e.g. `connectionsnap001` (names vary: `connectionsnap_04`, `connectionsnap03`).
    pub name: String,
    /// Offset in the module's local frame.
    pub transform: Transform,
}

/// An oriented box approximating part of a module, in the module's local frame.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeBox {
    /// Box centre and orientation.
    pub transform: Transform,
    /// Half the box size along its own x, y, z.
    pub half_extents: [f64; 3],
}

/// A module's collision meshes in its own frame, with a bounding-volume tree
/// for exact intersection tests (`layout::meshes_intersect`).
#[derive(Debug, Clone, PartialEq)]
pub struct CollisionMesh {
    pub points: Vec<[f32; 3]>,
    /// Ordered so that each tree leaf holds a contiguous run.
    pub triangles: Vec<[u32; 3]>,
    /// The tree, root first.
    pub nodes: Vec<MeshNode>,
}

/// A node of a `CollisionMesh` tree: bounds in the module frame, and either two
/// children or (a leaf) `count` triangles from `first`.
#[derive(Debug, Clone, PartialEq)]
pub struct MeshNode {
    pub lo: [f32; 3],
    pub hi: [f32; 3],
    pub children: Option<(u32, u32)>,
    pub first: u32,
    pub count: u32,
}

/// One part's collision mesh in the module frame: (points, triangles).
type MeshPart = (Vec<[f64; 3]>, Vec<[u32; 3]>);

/// A turret or shield slot of a module: a component connection with a `group`.
/// Plans equip slots by group (`<upgrades><groups>`), never one by one.
#[derive(Debug, Clone, PartialEq)]
pub struct EquipmentSlot {
    /// `turrets` or `shields`, as the plan's element names them.
    pub kind: String,
    /// Group name, e.g. `group01`.
    pub group: String,
    /// Connection tags, e.g. `turret medium standard missile hittable combat`.
    pub tags: Vec<String>,
    /// Placement in the module's frame.
    pub transform: Transform,
}

/// Space a module keeps free for ships: a component connection tagged
/// `exclusionzone <ship class>` (piers, fabrication bays, some dock areas). Its
/// size per ship class is `Catalog::exclusion_size`.
#[derive(Debug, Clone, PartialEq)]
pub struct Clearance {
    /// Ship class tag, e.g. `ship_xl`.
    pub tag: String,
    /// The zone's frame in the module's frame: ships come and go along its +Z.
    pub transform: Transform,
}

/// Equipment players can build for station modules: a turret or shield.
#[derive(Debug, Clone, PartialEq)]
pub struct Equipment {
    pub macro_name: String,
    pub ware: String,
    /// Display name (English).
    pub name: String,
    /// `turrets` or `shields`.
    pub kind: String,
    /// Tags a slot needs for it to fit (its component's `component` connection,
    /// without `component`), e.g. `turret medium standard hittable combat`.
    pub tags: Vec<String>,
    pub race: Option<String>,
    /// Mark (1, 2, ...), if given.
    pub mk: Option<u32>,
}

impl Equipment {
    /// Whether it fits a slot: same kind, and the slot has every tag it needs.
    pub fn fits(&self, slot: &EquipmentSlot) -> bool {
        self.kind == slot.kind && self.tags.iter().all(|t| slot.tags.contains(t))
    }

    /// Slot size from the tags: `small`, `medium`, `large` or `extralarge`.
    pub fn size(&self) -> Option<&str> {
        size_of(&self.tags)
    }
}

/// The size tag among `tags`.
pub fn size_of(tags: &[String]) -> Option<&str> {
    tags.iter().map(String::as_str).find(|t| ["small", "medium", "large", "extralarge"].contains(t))
}

/// A visible part of a module with its mesh files.
#[derive(Debug, Clone, PartialEq)]
pub struct Part {
    pub name: String,
    /// Path of the mesh files without the `-lodN.xmf` suffix, e.g.
    /// `assets/structures/connectionmodules/struct_arg_base_02_data/part_main`.
    pub mesh: String,
    /// Levels of detail that exist (0 = finest).
    pub lods: Vec<u8>,
    /// How small the part's features are: 0 main hull, 1 `detail_l`, 2 `detail_m`,
    /// 3 `detail_s`, 4 `detail_xs` (the game shows smaller details only up close).
    /// Interiors count as 3.
    pub detail: u8,
    /// Placement in the module's frame (parent parts composed).
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
    /// Rough shape for display: the module's main parts as boxes.
    pub shape: Vec<ShapeBox>,
    /// Visible parts with meshes (see `Catalog::module_model`).
    pub parts: Vec<Part>,
    /// Parts that collide: (collision mesh path without `-collision.xmf`, placement
    /// in the module's frame). See `Catalog::collision_boxes`.
    pub collision: Vec<(String, Transform)>,
    /// Turret and shield slots, in component order.
    pub equipment_slots: Vec<EquipmentSlot>,
    /// Exclusion zones: space kept free for ships to dock or leave.
    pub clearance: Vec<Clearance>,
    /// What it produces, needs and holds (see `economy`).
    pub economy: ModuleEconomy,
}

/// A module's part in the station economy, from its macro `<properties>`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ModuleEconomy {
    /// Production queue: (ware, recipe method), run in turn.
    pub production: Vec<(String, String)>,
    /// Workers it needs at full staff (`<workforce max>`).
    pub workforce: u32,
    /// Workers it houses: (race, capacity).
    pub housing: Option<(String, u32)>,
    /// Cargo storage: (capacity in m³, cargo types such as `container`).
    pub cargo: Vec<(f64, Vec<String>)>,
}

impl Module {
    pub fn snap(&self, name: &str) -> Option<&Snap> {
        self.snaps.iter().find(|s| s.name.eq_ignore_ascii_case(name))
    }

    /// Equipment group names, in the order they first appear.
    pub fn equipment_groups(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for s in &self.equipment_slots {
            if !out.contains(&s.group.as_str()) {
                out.push(&s.group);
            }
        }
        out
    }

    /// The slots of one kind (`turrets` or `shields`) in a group.
    pub fn group_slots<'a>(&'a self, group: &'a str, kind: &'a str) -> impl Iterator<Item = &'a EquipmentSlot> {
        self.equipment_slots.iter().filter(move |s| s.group.eq_ignore_ascii_case(group) && s.kind == kind)
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
    /// Build sets it belongs to, e.g. `factory` (player stations), `tradestation_argon`.
    pub sets: Vec<String>,
}

/// Texture maps of a material (`libraries/material_library.xml`), as game paths
/// without extension.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MaterialMaps {
    pub diffuse: Option<String>,
    pub normal: Option<String>,
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
    /// Lowercase macro -> collision boxes; built on first use.
    collision_cache: Mutex<HashMap<String, Arc<Vec<ShapeBox>>>>,
    /// Lowercase macro -> collision mesh (`None`: it has none); built on first use.
    mesh_cache: Mutex<HashMap<String, Option<Arc<CollisionMesh>>>>,
    /// `collection.material` (lowercase) -> maps; read on first use.
    materials: OnceLock<HashMap<String, MaterialMaps>>,
    /// Wares and recipes (build costs); read on first use.
    wares: OnceLock<Result<Wares>>,
    /// Buildable turrets and shields; read on first use.
    equipment: OnceLock<Result<Vec<Equipment>>>,
    /// Ship class tag -> exclusion zone size (x, y, z) in metres; read on first use.
    exclusion_sizes: OnceLock<HashMap<String, [f64; 3]>>,
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
            collision_cache: Mutex::new(HashMap::new()),
            mesh_cache: Mutex::new(HashMap::new()),
            materials: OnceLock::new(),
            wares: OnceLock::new(),
            equipment: OnceLock::new(),
            exclusion_sizes: OnceLock::new(),
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
            collision_cache: Mutex::new(HashMap::new()),
            mesh_cache: Mutex::new(HashMap::new()),
            materials: OnceLock::new(),
            wares: OnceLock::new(),
            equipment: OnceLock::new(),
            exclusion_sizes: OnceLock::new(),
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
            let sets = child(child(props, "build"), "sets").into_iter().flat_map(|s| s.children());
            out.push(ModuleInfo {
                macro_name: m.attribute("name").unwrap_or(key).to_owned(),
                class: class.to_owned(),
                name: ident.and_then(|i| i.attribute("name")).map_or_else(|| key.clone(), |n| self.texts.resolve(n)),
                race: ident.and_then(|i| i.attribute("makerrace")).map(str::to_owned),
                source: entry.source.map(|i| self.extensions[i].id.clone()),
                sets: sets.filter_map(|s| s.attribute("ref")).map(str::to_owned).collect(),
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

    /// Wares with their build recipes (base game and enabled extensions), for costs.
    pub fn wares(&self) -> Result<&Wares> {
        let folders: Vec<String> = self.extensions.iter().map(|e| e.folder.clone()).collect();
        self.wares.get_or_init(|| Wares::load(&self.vfs, &folders, &self.texts)).as_ref().map_err(Clone::clone)
    }

    /// Turrets and shields players can build for their stations: a recipe, not
    /// `noplayerblueprint`, and fitting a slot of some module players build (ship
    /// variants such as `turret_arg_m_beam_01_mk1` fit none; Xenon equipment
    /// carries a `xenon` tag no player module has). By kind, size, race, name, macro.
    pub fn equipment(&self) -> Result<&[Equipment]> {
        self.equipment.get_or_init(|| self.read_equipment()).as_deref().map_err(Clone::clone)
    }

    fn read_equipment(&self) -> Result<Vec<Equipment>> {
        let mut out = Vec::new();
        for w in self.wares()?.all() {
            // By tags, not ware group: the Boron DLC files its turrets under `weapons`.
            let has = |tag: &str| w.tags.iter().any(|t| t == tag);
            let kind = match (has("equipment"), has("turret"), has("shield")) {
                (true, true, _) => "turrets",
                (true, _, true) => "shields",
                _ => continue,
            };
            let Some(macro_name) = w.component.as_deref() else { continue };
            if w.recipes.is_empty() || w.tags.iter().any(|t| t == "noplayerblueprint") {
                continue;
            }
            let Some(entry) = self.macros.get(&macro_name.to_ascii_lowercase()) else { continue };
            let xml = self.vfs.read_text(&format!("{}.xml", entry.path))?;
            let doc = roxmltree::Document::parse(&xml).map_err(|e| Error(format!("{}: {e}", entry.path)))?;
            let Some(m) = doc.descendants().find(|n| n.has_tag_name("macro")) else { continue };
            let ident = child(child(Some(m), "properties"), "identification");
            let Some(component) = m.children().find(|n| n.has_tag_name("component")).and_then(|c| c.attribute("ref")) else { continue };
            let Some(centry) = self.components.get(&component.to_ascii_lowercase()) else { continue };
            let cxml = self.vfs.read_text(&format!("{}.xml", centry.path))?;
            let cdoc = roxmltree::Document::parse(&cxml).map_err(|e| Error(format!("{}: {e}", centry.path)))?;
            let tags = cdoc
                .descendants()
                .filter(|n| n.has_tag_name("connection"))
                .map(|n| n.attribute("tags").unwrap_or_default().split_whitespace().map(str::to_owned).collect::<Vec<String>>())
                .find(|t| t.iter().any(|t| t == "component"));
            let Some(mut tags) = tags else { continue };
            tags.retain(|t| t != "component");
            out.push(Equipment {
                macro_name: m.attribute("name").unwrap_or(macro_name).to_owned(),
                ware: w.id.clone(),
                name: w.name.clone(),
                kind: kind.to_owned(),
                tags,
                race: ident.and_then(|i| i.attribute("makerrace")).map(str::to_owned),
                mk: ident.and_then(|i| i.attribute("mk")).and_then(|v| v.parse().ok()),
            });
        }
        let size_rank = |e: &Equipment| ["small", "medium", "large", "extralarge"].iter().position(|s| Some(*s) == e.size()).unwrap_or(4);
        let mut slots = Vec::new();
        for m in self.station_modules()?.iter().filter(|m| m.sets.iter().any(|s| s == "factory")) {
            slots.extend(self.module(&m.macro_name)?.equipment_slots.iter().cloned());
        }
        out.retain(|e| slots.iter().any(|s| e.fits(s)));
        out.sort_by(|a, b| (&a.kind, size_rank(a), &a.race, &a.name, &a.macro_name).cmp(&(&b.kind, size_rank(b), &b.race, &b.name, &b.macro_name)));
        Ok(out)
    }

    /// Size (x, y, z in metres) of an exclusion zone of a ship class tag such as
    /// `ship_xl`, from `libraries/parameters.xml` (`<exclusionzones>`).
    pub fn exclusion_size(&self, tag: &str) -> Option<[f64; 3]> {
        self.exclusion_sizes.get_or_init(|| self.read_exclusion_sizes()).get(&tag.to_ascii_lowercase()).copied()
    }

    fn read_exclusion_sizes(&self) -> HashMap<String, [f64; 3]> {
        let mut out = HashMap::new();
        let Ok(xml) = self.vfs.read_text("libraries/parameters.xml") else { return out };
        let Ok(doc) = roxmltree::Document::parse(&xml) else { return out };
        let zones = doc.descendants().filter(|n| n.has_tag_name("exclusionzone") && n.parent().is_some_and(|p| p.has_tag_name("exclusionzones")));
        for z in zones {
            let num = |a: &str| z.attribute(a).and_then(|v| v.parse::<f64>().ok());
            if let (Some(tag), Some(x), Some(y), Some(l)) = (z.attribute("tag"), num("x"), num("y"), num("z")) {
                out.insert(tag.to_ascii_lowercase(), [x, y, l]);
            }
        }
        out
    }

    /// Texture maps of a mesh material such as `generic.generic_p1_hulltexture`.
    pub fn material(&self, name: &str) -> Option<MaterialMaps> {
        self.materials.get_or_init(|| self.read_materials()).get(&name.to_ascii_lowercase()).cloned()
    }

    /// A texture by game path without extension (`assets/textures/...`), from its
    /// `.gz` or `.dds` file, with mip levels down to `max_size` pixels.
    pub fn texture(&self, path: &str, max_size: u32) -> Result<Texture> {
        let file = [".gz", ".dds"].iter().map(|ext| format!("{path}{ext}")).find(|f| self.vfs.contains(f));
        let file = file.ok_or_else(|| Error(format!("texture not found: {path}")))?;
        texture::parse_dds(&texture::gunzip(&self.vfs.read(&file)?)?, max_size).map_err(|e| Error(format!("{file}: {e}")))
    }

    /// The material libraries (`material_library.xml` and `material_library_1.xml`,
    /// which holds the newer `p2*` materials), of the base game and of extensions
    /// (whose files add materials through `<diff><add>`).
    fn read_materials(&self) -> HashMap<String, MaterialMaps> {
        let roots = std::iter::once(String::new()).chain(self.extensions.iter().map(|e| format!("extensions/{}/", e.folder)));
        let files = roots.flat_map(|root| {
            std::iter::once(format!("{root}libraries/material_library.xml"))
                .chain((1..10).map(move |n| format!("{root}libraries/material_library_{n}.xml")))
        });
        let mut out = HashMap::new();
        for file in files.filter(|f| self.vfs.contains(f)) {
            let Ok(xml) = self.vfs.read_text(&file) else { continue };
            let Ok(doc) = roxmltree::Document::parse(&xml) else { continue };
            for m in doc.descendants().filter(|n| n.has_tag_name("material")) {
                let collection = m.ancestors().find(|a| a.has_tag_name("collection")).and_then(|c| c.attribute("name"));
                let (Some(collection), Some(name)) = (collection, m.attribute("name")) else { continue };
                let map = |kind: &str| {
                    m.descendants()
                        .find(|p| p.has_tag_name("property") && p.attribute("type") == Some("BitMap") && p.attribute("name") == Some(kind))
                        .and_then(|p| p.attribute("value"))
                        .map(normalize)
                };
                let maps = MaterialMaps { diffuse: map("diffuse_map"), normal: map("normal_map") };
                out.insert(format!("{collection}.{name}").to_ascii_lowercase(), maps);
            }
        }
        out
    }

    /// A macro's display name (English) from its `<identification name>`, for
    /// modules outside `station_modules` (e.g. a mod's module without build sets).
    pub fn display_name(&self, macro_name: &str) -> Option<String> {
        let entry = self.macros.get(&macro_name.to_ascii_lowercase())?;
        let xml = self.vfs.read_text(&format!("{}.xml", entry.path)).ok()?;
        let doc = roxmltree::Document::parse(&xml).ok()?;
        let m = doc.descendants().find(|n| n.has_tag_name("macro"))?;
        let name = child(child(Some(m), "properties"), "identification")?.attribute("name")?;
        Some(self.texts.resolve(name)).filter(|n| !n.is_empty() && !n.starts_with('{'))
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
        let snaps: Vec<Snap> = comp
            .descendants()
            .filter(|n| n.has_tag_name("connection"))
            .filter(|n| n.attribute("tags").is_some_and(|t| t.split_whitespace().any(|t| t == "snap")))
            .filter_map(|n| {
                let name = n.attribute("name")?.to_ascii_lowercase();
                let offset = n.children().find(|c| c.has_tag_name("offset"));
                Some(Snap { name, transform: offset.map_or(Transform::IDENTITY, parse_offset) })
            })
            .collect();

        let (parts, collision) = self.mesh_parts(comp, &centry.path);
        let equipment_slots = comp
            .descendants()
            .filter(|n| n.has_tag_name("connection"))
            .filter_map(|n| {
                let group = n.attribute("group")?.trim().to_ascii_lowercase();
                let tags: Vec<String> = n.attribute("tags").unwrap_or_default().split_whitespace().map(str::to_owned).collect();
                let kind = if tags.iter().any(|t| t == "turret") {
                    "turrets"
                } else if tags.iter().any(|t| t == "shield") {
                    "shields"
                } else {
                    return None;
                };
                let transform = n.children().find(|c| c.has_tag_name("offset")).map_or(Transform::IDENTITY, parse_offset);
                Some(EquipmentSlot { kind: kind.to_owned(), group, tags, transform })
            })
            .collect();
        let clearance = comp
            .descendants()
            .filter(|n| n.has_tag_name("connection"))
            .filter_map(|n| {
                let mut tags = n.attribute("tags")?.split_whitespace();
                tags.clone().any(|t| t == "exclusionzone").then_some(())?;
                let tag = tags.find(|t| *t != "exclusionzone")?.to_ascii_lowercase();
                let transform = n.children().find(|c| c.has_tag_name("offset")).map_or(Transform::IDENTITY, parse_offset);
                Some(Clearance { tag, transform })
            })
            .collect();
        let mut shape = shape_boxes(comp);
        if shape.is_empty() {
            shape.extend(snap_bounds(&snaps));
        }
        Ok(Module {
            macro_name: macro_el.attribute("name").unwrap_or(key).to_owned(),
            class: macro_el.attribute("class").unwrap_or_default().to_owned(),
            component: component.to_owned(),
            source: entry.source.map(|i| self.extensions[i].id.clone()),
            snaps,
            shape,
            parts,
            collision,
            equipment_slots,
            clearance,
            economy: economy(macro_el.children().find(|n| n.has_tag_name("properties"))),
        })
    }

    /// The module's look: its visible parts down to detail level `details` (see
    /// `Part::detail`), merged into one mesh in the module's frame, at level of
    /// detail `lod` where a part has it (else its nearest). Effect materials
    /// (glows, light cones, decals) are left out.
    pub fn module_model(&self, macro_name: &str, lod: u8, details: u8) -> Result<Mesh> {
        let module = self.module(macro_name)?;
        let mut model = Mesh::default();
        for part in module.parts.iter().filter(|p| p.detail <= details) {
            let Some(&level) = part.lods.iter().filter(|&&l| l <= lod).max().or(part.lods.first()) else { continue };
            let path = format!("{}-lod{level}.xmf", part.mesh);
            let mesh = mesh::parse(&self.vfs.read(&path)?).map_err(|e| Error(format!("{path}: {e}")))?;
            let base = model.positions.len() as u32;
            model.positions.extend(mesh.positions.iter().map(|p| {
                let w = part.transform.then(&Transform { position: p.map(f64::from), rotation: Quat::IDENTITY }).position;
                w.map(|v| v as f32)
            }));
            model.normals.extend(mesh.normals.iter().map(|n| part.transform.rotation.rotate(n.map(f64::from)).map(|v| v as f32)));
            model.uvs.extend(&mesh.uvs);
            for (material, first, count) in mesh.groups.iter().filter(|g| !is_effect_material(&g.0)) {
                let start = model.indices.len() as u32;
                let range = *first as usize..(*first + *count) as usize;
                model.indices.extend(mesh.indices.get(range).unwrap_or_default().iter().map(|i| i + base));
                model.groups.push((material.clone(), start, model.indices.len() as u32 - start));
            }
        }
        Ok(model)
    }

    /// Visible parts of a component: every `<connection tags="part">` except
    /// collision-only and invisible ones. A part may reference a part of another
    /// component (`ref="xref_dockingbay_gen_xl_pier_01.part_main"`) and may sit on
    /// a parent part (`parent="part_main"`), whose placement it inherits.
    /// Also returns the parts that collide (not tagged `nocollision`, collision-only
    /// boxes included) with a collision mesh: (mesh path without the
    /// `-collision.xmf` suffix, placement in the module's frame).
    fn mesh_parts(&self, comp: roxmltree::Node, component_path: &str) -> (Vec<Part>, Vec<(String, Transform)>) {
        struct Raw<'a> {
            name: &'a str,
            reference: Option<&'a str>,
            detail: u8,
            offset: Transform,
            parent: Option<&'a str>,
            visible: bool,
            collides: bool,
        }
        let raw: Vec<Raw> = comp
            .descendants()
            .filter(|n| n.has_tag_name("connection"))
            .filter_map(|conn| {
                let tags: Vec<&str> = conn.attribute("tags").unwrap_or_default().split_whitespace().collect();
                let part = child(child(Some(conn), "parts"), "part")?;
                if !tags.contains(&"part") {
                    return None;
                }
                let name = part.attribute("name")?;
                let detail = ["detail_l", "detail_m", "detail_s", "detail_xs"]
                    .iter()
                    .position(|d| tags.contains(d))
                    .map_or(if name.contains("interior") { 3 } else { 0 }, |i| i as u8 + 1);
                Some(Raw {
                    name,
                    reference: part.attribute("ref"),
                    detail,
                    offset: conn.children().find(|c| c.has_tag_name("offset")).map_or(Transform::IDENTITY, parse_offset),
                    parent: conn.attribute("parent"),
                    visible: !tags.iter().any(|t| *t == "collisiononly" || *t == "geometryinvisible"),
                    collides: !tags.contains(&"nocollision"),
                })
            })
            .collect();
        let world = |start: &Raw| {
            let mut t = start.offset;
            let mut parent = start.parent;
            for _ in 0..16 {
                let Some(p) = parent.and_then(|name| raw.iter().find(|r| r.name.eq_ignore_ascii_case(name))) else { break };
                t = p.offset.then(&t);
                parent = p.parent;
            }
            t
        };
        // A component's meshes are in its `<source geometry>` folder: usually
        // `<path>_data`, but a mod can borrow another's (RecycleShipsAndStations'
        // recycle module uses `prod_gen_dronecomponents_data`).
        let geometry = comp
            .children()
            .find(|c| c.has_tag_name("source"))
            .and_then(|s| s.attribute("geometry"))
            .map_or_else(|| format!("{component_path}_data"), str::to_owned);
        let mesh_path = |r: &Raw| match r.reference.and_then(|v| v.split_once('.')) {
            Some((component, part)) => Some(normalize(&format!("{}_data/{part}", self.components.get(&component.to_ascii_lowercase())?.path))),
            None => Some(normalize(&format!("{geometry}/{}", r.name))),
        };
        let visible = raw
            .iter()
            .filter(|r| r.visible)
            .filter_map(|r| {
                let mesh = mesh_path(r)?;
                let lods: Vec<u8> = (0..5).filter(|l| self.vfs.contains(&format!("{mesh}-lod{l}.xmf"))).collect();
                (!lods.is_empty()).then(|| Part { name: r.name.to_owned(), mesh, lods, detail: r.detail, transform: world(r) })
            })
            .collect();
        let colliding = raw
            .iter()
            .filter(|r| r.collides)
            .filter_map(|r| mesh_path(r).filter(|m| self.vfs.contains(&format!("{m}-collision.xmf"))).map(|m| (m, world(r))))
            .collect();
        (visible, colliding)
    }

    /// Boxes that closely enclose the module's collision meshes, in its frame
    /// (see `split_boxes`); the part boxes when it has no collision meshes.
    pub fn collision_boxes(&self, macro_name: &str) -> Result<Arc<Vec<ShapeBox>>> {
        let key = macro_name.to_ascii_lowercase();
        if let Some(b) = self.collision_cache.lock().unwrap().get(&key) {
            return Ok(b.clone());
        }
        let module = self.module(macro_name)?;
        // `-collision1` is a coarser copy of `-collision`, good enough for boxes.
        let from_meshes = || -> Result<Vec<ShapeBox>> {
            let mut boxes = Vec::new();
            for (points, triangles) in self.collision_triangles(&module, true)? {
                split_boxes(&points, triangles, 0, &mut boxes);
            }
            Ok(boxes)
        };
        // A module whose collision meshes are missing or unreadable keeps its part boxes.
        let boxes = Arc::new(from_meshes().ok().filter(|b| !b.is_empty()).unwrap_or_else(|| module.shape.clone()));
        self.collision_cache.lock().unwrap().insert(key, boxes.clone());
        Ok(boxes)
    }

    /// The module's full collision meshes (`-collision.xmf`) as one mesh with a
    /// tree, for exact tests; `None` when it has none (or they can't be read).
    pub fn collision_mesh(&self, macro_name: &str) -> Result<Option<Arc<CollisionMesh>>> {
        let key = macro_name.to_ascii_lowercase();
        if let Some(m) = self.mesh_cache.lock().unwrap().get(&key) {
            return Ok(m.clone());
        }
        let module = self.module(macro_name)?;
        let mesh = self.collision_triangles(&module, false).ok().and_then(|parts| {
            let mut points: Vec<[f32; 3]> = Vec::new();
            let mut triangles: Vec<[u32; 3]> = Vec::new();
            for (p, t) in parts {
                let base = points.len() as u32;
                points.extend(p.iter().map(|v| v.map(|x| x as f32)));
                triangles.extend(t.iter().map(|t| t.map(|i| i + base)));
            }
            if triangles.is_empty() {
                return None;
            }
            let mut nodes = Vec::new();
            build_tree(&points, &mut triangles, 0, &mut nodes);
            Some(Arc::new(CollisionMesh { points, triangles, nodes }))
        });
        self.mesh_cache.lock().unwrap().insert(key, mesh.clone());
        Ok(mesh)
    }

    /// Each colliding part's collision mesh in the module frame: (points, triangles).
    /// `coarse` takes `-collision1.xmf` where it exists.
    fn collision_triangles(&self, module: &Module, coarse: bool) -> Result<Vec<MeshPart>> {
        let mut out = Vec::new();
        for (mesh, at) in &module.collision {
            let coarse_file = format!("{mesh}-collision1.xmf");
            let file = if coarse && self.vfs.contains(&coarse_file) { coarse_file } else { format!("{mesh}-collision.xmf") };
            let m = mesh::parse(&self.vfs.read(&file)?).map_err(|e| Error(format!("{file}: {e}")))?;
            let points = m.positions.iter().map(|p| at.then(&Transform { position: p.map(f64::from), rotation: Quat::IDENTITY }).position).collect();
            out.push((points, m.indices.as_chunks::<3>().0.to_vec()));
        }
        Ok(out)
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

/// Glows, light cones, decals and other effect materials: drawn by the game as
/// overlays, not as solid geometry.
fn is_effect_material(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    ["fx", "lightcone", "decal", "effects.", "gradient", "laser", "flare", "pattern_hex"].iter().any(|k| n.contains(k))
}

/// Production queue (`<production><queue ware method/>`, or `<queue><item …/>` for
/// several wares), workforce and cargo from a macro's `<properties>`.
fn economy(props: Option<roxmltree::Node>) -> ModuleEconomy {
    let mut out = ModuleEconomy::default();
    for p in props.iter().flat_map(|p| p.children()) {
        match p.tag_name().name() {
            "production" => {
                for q in p.children().filter(|c| c.has_tag_name("queue")) {
                    let items = std::iter::once(q).chain(q.children().filter(|c| c.has_tag_name("item")));
                    out.production.extend(items.filter_map(|i| {
                        Some((i.attribute("ware")?.to_owned(), i.attribute("method").unwrap_or("default").to_owned()))
                    }));
                }
            }
            "workforce" => {
                out.workforce = p.attribute("max").and_then(|v| v.parse().ok()).unwrap_or(0);
                if let (Some(race), Some(capacity)) = (p.attribute("race"), p.attribute("capacity").and_then(|v| v.parse().ok())) {
                    out.housing = Some((race.to_owned(), capacity));
                }
            }
            "cargo" => {
                if let Some(max) = p.attribute("max").and_then(|v| v.parse().ok()) {
                    out.cargo.push((max, p.attribute("tags").unwrap_or_default().split_whitespace().map(str::to_owned).collect()));
                }
            }
            _ => {}
        }
    }
    out
}

fn child<'a, 'i>(node: Option<roxmltree::Node<'a, 'i>>, tag: &str) -> Option<roxmltree::Node<'a, 'i>> {
    node.and_then(|n| n.children().find(|c| c.has_tag_name(tag)))
}

/// Boxes for a component's solid parts: `<connection tags="part ...">` with a
/// `<part><size><max/><center/>` (half extents and centre, in the connection's
/// frame). Detail, collision-only and invisible parts (e.g. a pier's docking
/// volume) are skipped.
fn shape_boxes(component: roxmltree::Node) -> Vec<ShapeBox> {
    let num = |n: Option<roxmltree::Node>, a: &str| n.and_then(|n| n.attribute(a)).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
    let mut boxes = Vec::new();
    for conn in component.descendants().filter(|n| n.has_tag_name("connection")) {
        let tags: Vec<&str> = conn.attribute("tags").unwrap_or_default().split_whitespace().collect();
        let skip = tags.iter().any(|t| t.starts_with("detail") || *t == "collisiononly" || *t == "geometryinvisible");
        if !tags.contains(&"part") || skip {
            continue;
        }
        let size = child(child(child(Some(conn), "parts"), "part"), "size");
        let (Some(max), Some(center)) = (child(size, "max"), child(size, "center")) else { continue };
        let frame = conn.children().find(|c| c.has_tag_name("offset")).map_or(Transform::IDENTITY, parse_offset);
        let centre = Transform { position: [num(Some(center), "x"), num(Some(center), "y"), num(Some(center), "z")], rotation: Quat::IDENTITY };
        boxes.push(ShapeBox {
            transform: frame.then(&centre),
            half_extents: [num(Some(max), "x"), num(Some(max), "y"), num(Some(max), "z")],
        });
    }
    boxes
}

/// Builds a bounding-volume tree over `triangles` (reordering them): each node
/// halves its triangles at the median centre along its longest axis, down to
/// leaves of at most 8. Returns the node's index.
fn build_tree(points: &[[f32; 3]], triangles: &mut [[u32; 3]], first: u32, nodes: &mut Vec<MeshNode>) -> u32 {
    let (mut lo, mut hi) = ([f32::MAX; 3], [f32::MIN; 3]);
    for p in triangles.iter().flatten().map(|&i| points[i as usize]) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let index = nodes.len() as u32;
    nodes.push(MeshNode { lo, hi, children: None, first, count: triangles.len() as u32 });
    if triangles.len() > 8 {
        let axis = (0..3).max_by(|&a, &b| (hi[a] - lo[a]).total_cmp(&(hi[b] - lo[b]))).unwrap();
        let centre = |t: &[u32; 3]| t.iter().map(|&i| points[i as usize][axis]).sum::<f32>();
        let half = triangles.len() / 2;
        triangles.select_nth_unstable_by(half, |a, b| centre(a).total_cmp(&centre(b)));
        let (left, right) = triangles.split_at_mut(half);
        let l = build_tree(points, left, first, nodes);
        let r = build_tree(points, right, first + half as u32, nodes);
        nodes[index as usize].children = Some((l, r));
    }
    index
}

/// Splits a mesh's triangles in half along the longest axis of their bounds
/// (at the median centre), down to `COLLISION_DEPTH` levels or boxes of at most
/// `COLLISION_BOX_M`, and adds each group's bounds as a box. Every triangle lies
/// in a box, so the boxes enclose the mesh surface; together they follow its shape
/// (a spire gets thin boxes, the space beside it none).
fn split_boxes(points: &[[f64; 3]], mut triangles: Vec<[u32; 3]>, depth: u32, out: &mut Vec<ShapeBox>) {
    const COLLISION_DEPTH: u32 = 7;
    const COLLISION_BOX_M: f64 = 100.0;
    let (mut lo, mut hi) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in triangles.iter().flatten().map(|&i| points[i as usize]) {
        for k in 0..3 {
            lo[k] = lo[k].min(p[k]);
            hi[k] = hi[k].max(p[k]);
        }
    }
    let size = [0, 1, 2].map(|k| hi[k] - lo[k]);
    let axis = (0..3).max_by(|&a, &b| size[a].total_cmp(&size[b])).unwrap();
    if triangles.len() < 2 || depth >= COLLISION_DEPTH || size[axis] <= COLLISION_BOX_M {
        if !triangles.is_empty() {
            out.push(ShapeBox {
                transform: Transform { position: [0, 1, 2].map(|k| (lo[k] + hi[k]) / 2.0), rotation: Quat::IDENTITY },
                half_extents: size.map(|s| s / 2.0),
            });
        }
        return;
    }
    let centre = |t: &[u32; 3]| t.iter().map(|&i| points[i as usize][axis]).sum::<f64>();
    let half = triangles.len() / 2;
    triangles.select_nth_unstable_by(half, |a, b| centre(a).total_cmp(&centre(b)));
    let upper = triangles.split_off(half);
    split_boxes(points, triangles, depth + 1, out);
    split_boxes(points, upper, depth + 1, out);
}

/// Fallback shape for modules without parts of their own (assembled from child
/// pieces, e.g. `landmarks_par_storage_01_macro`): one box spanning the snap
/// points, at least 50 m from the centre in each direction.
fn snap_bounds(snaps: &[Snap]) -> Option<ShapeBox> {
    let first = snaps.first()?.transform.position;
    let (mut lo, mut hi) = (first, first);
    for s in snaps {
        for i in 0..3 {
            lo[i] = lo[i].min(s.transform.position[i]);
            hi[i] = hi[i].max(s.transform.position[i]);
        }
    }
    Some(ShapeBox {
        transform: Transform { position: [0, 1, 2].map(|i| (lo[i] + hi[i]) / 2.0), rotation: Quat::IDENTITY },
        half_extents: [0, 1, 2].map(|i| ((hi[i] - lo[i]) / 2.0).max(50.0)),
    })
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
