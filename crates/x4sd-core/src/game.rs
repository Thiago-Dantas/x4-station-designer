//! Locating the game's extensions and the player profile, and building the
//! virtual file system from them.

use std::path::{Path, PathBuf};

use crate::vfs::Vfs;
use crate::Result;

#[derive(Debug, Clone, PartialEq)]
pub struct Extension {
    /// `content.xml` id: what the profile enables and what plan `<patches>` name.
    pub id: String,
    /// Folder name: what virtual paths use (`extensions/<folder>/...`). Often differs from `id`.
    pub folder: String,
    pub dir: PathBuf,
    pub name: String,
    pub version: String,
    pub enabled: bool,
    /// `content.xml` `save`: savegames depend on it. The game lists only such
    /// extensions in plan `<patches>` (a mod with `save="false"` was left out).
    pub save: bool,
}

/// Extensions in `<game>/extensions` and `<profile>/extensions`. The enabled state
/// comes from the profile's `content.xml`, falling back to the extension's own
/// default. Order: Egosoft DLCs first, then the rest by id. Folders without a
/// readable `content.xml` are not extensions and are skipped.
pub fn find_extensions(game_dir: &Path, profile_dir: Option<&Path>) -> Result<Vec<Extension>> {
    let overrides = match profile_dir.map(|p| p.join("content.xml")) {
        Some(path) if path.exists() => read_profile_overrides(&path)?,
        _ => Vec::new(),
    };
    let mut roots = vec![game_dir.join("extensions")];
    roots.extend(profile_dir.map(|p| p.join("extensions")));

    let mut extensions = Vec::new();
    for root in roots.iter().filter(|r| r.is_dir()) {
        for dir in std::fs::read_dir(root)? {
            let dir = dir?.path();
            if let Some(mut ext) = read_extension(&dir) {
                if let Some((_, enabled)) = overrides.iter().find(|(id, _)| *id == ext.id) {
                    ext.enabled = *enabled;
                }
                extensions.push(ext);
            }
        }
    }
    extensions.sort_by_key(|e| (!e.id.starts_with("ego_dlc_"), e.id.to_ascii_lowercase()));
    Ok(extensions)
}

fn read_extension(dir: &Path) -> Option<Extension> {
    let text = std::fs::read_to_string(dir.join("content.xml")).ok()?;
    let doc = roxmltree::Document::parse(text.trim_start_matches('\u{feff}')).ok()?;
    let root = doc.root_element();
    Some(Extension {
        id: root.attribute("id")?.to_owned(),
        folder: dir.file_name()?.to_string_lossy().into_owned(),
        dir: dir.to_owned(),
        name: root.attribute("name").unwrap_or_default().to_owned(),
        version: root.attribute("version").unwrap_or_default().to_owned(),
        enabled: root.attribute("enabled").is_none_or(is_true),
        save: root.attribute("save").is_none_or(is_true),
    })
}

fn read_profile_overrides(path: &Path) -> Result<Vec<(String, bool)>> {
    let text = std::fs::read_to_string(path)?;
    let doc = roxmltree::Document::parse(text.trim_start_matches('\u{feff}'))?;
    Ok(doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("extension"))
        .filter_map(|n| Some((n.attribute("id")?.to_owned(), is_true(n.attribute("enabled")?))))
        .collect())
}

fn is_true(value: &str) -> bool {
    value == "1" || value.eq_ignore_ascii_case("true")
}

/// Base-game catalogs (`01.cat`, `02.cat`, ...) followed by every enabled
/// extension's `ext_NN.cat` (under `extensions/<folder>/`) and `subst_NN.cat`
/// (which replace base-game paths).
pub fn load_vfs(game_dir: &Path, extensions: &[Extension]) -> Result<Vfs> {
    let mut vfs = Vfs::default();
    for cat in catalogs(game_dir, "")? {
        vfs.add_catalog(&cat, "")?;
    }
    for ext in extensions.iter().filter(|e| e.enabled) {
        for cat in catalogs(&ext.dir, "ext_")? {
            vfs.add_catalog(&cat, &format!("extensions/{}/", ext.folder))?;
        }
        for cat in catalogs(&ext.dir, "subst_")? {
            vfs.add_catalog(&cat, "")?;
        }
    }
    Ok(vfs)
}

/// `<prefix>NN.cat` files in `dir`, sorted. Signature catalogs (`NN_sig.cat`) are skipped.
fn catalogs(dir: &Path, prefix: &str) -> Result<Vec<PathBuf>> {
    let mut cats: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy().to_ascii_lowercase();
            name.strip_prefix(prefix)
                .and_then(|rest| rest.strip_suffix(".cat"))
                .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    cats.sort();
    Ok(cats)
}

/// The most recently used profile folder, `Documents\Egosoft\X4\<id>`.
pub fn find_profile_dir() -> Option<PathBuf> {
    let root = PathBuf::from(std::env::var_os("USERPROFILE")?).join("Documents/Egosoft/X4");
    std::fs::read_dir(root)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.file_name().to_string_lossy().bytes().all(|b| b.is_ascii_digit()))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())
        .map(|e| e.path())
}
