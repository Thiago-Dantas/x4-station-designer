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
    /// `collection.material` (lowercase) -> maps; read on first use.
    materials: OnceLock<HashMap<String, MaterialMaps>>,
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
            materials: OnceLock::new(),
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
            materials: OnceLock::new(),
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

        let parts = self.mesh_parts(comp, &centry.path);
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
    fn mesh_parts(&self, comp: roxmltree::Node, component_path: &str) -> Vec<Part> {
        struct Raw<'a> {
            name: &'a str,
            reference: Option<&'a str>,
            detail: u8,
            offset: Transform,
            parent: Option<&'a str>,
            visible: bool,
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
        raw.iter()
            .filter(|r| r.visible)
            .filter_map(|r| {
                let mesh = match r.reference.and_then(|v| v.split_once('.')) {
                    Some((component, part)) => format!("{}_data/{part}", self.components.get(&component.to_ascii_lowercase())?.path),
                    None => format!("{component_path}_data/{}", r.name),
                };
                let lods: Vec<u8> = (0..5).filter(|l| self.vfs.contains(&format!("{mesh}-lod{l}.xmf"))).collect();
                (!lods.is_empty()).then(|| Part { name: r.name.to_owned(), mesh: normalize(&mesh), lods, detail: r.detail, transform: world(r) })
            })
            .collect()
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
