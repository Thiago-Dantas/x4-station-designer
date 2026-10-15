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

/// The X4 install: `X4_GAME_DIR` if set, else the folder the player picked in the
/// builder (`save_game_dir`), else a Steam library that has it, else a GOG install.
pub fn find_game_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("X4_GAME_DIR") {
        return Some(PathBuf::from(dir));
    }
    let saved = saved_game_dir_file().and_then(|f| std::fs::read_to_string(f).ok()).map(|s| PathBuf::from(s.trim()));
    saved.into_iter().chain(steam_game_dirs()).chain(gog_game_dirs()).find(|dir| is_game_dir(dir))
}

/// A folder with the game's data catalogs.
pub fn is_game_dir(dir: &Path) -> bool {
    dir.join("01.cat").is_file()
}

/// Remembers the X4 folder the player picked, for `find_game_dir`.
pub fn save_game_dir(dir: &Path) -> Result<()> {
    if !is_game_dir(dir) {
        return Err(crate::Error(format!("{} has no X4 game data (01.cat); pick the folder with X4.exe", dir.display())));
    }
    let file = saved_game_dir_file().ok_or_else(|| crate::Error("APPDATA is not set".to_owned()))?;
    std::fs::create_dir_all(file.parent().unwrap())?;
    std::fs::write(file, dir.to_string_lossy().as_bytes())?;
    Ok(())
}

fn saved_game_dir_file() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("APPDATA")?).join("x4sd/game-dir.txt"))
}

/// `X4 Foundations` in every Steam library (listed in `<Steam>\steamapps\libraryfolders.vdf`).
/// Steam's folder comes from the registry, else the default under Program Files.
fn steam_game_dirs() -> Vec<PathBuf> {
    let steam = steam_registry_dir().or_else(|| Some(PathBuf::from(std::env::var_os("ProgramFiles(x86)")?).join("Steam")));
    let Some(vdf) = steam.and_then(|s| std::fs::read_to_string(s.join("steamapps/libraryfolders.vdf")).ok()) else {
        return Vec::new();
    };
    vdf.lines()
        .filter(|l| l.trim_start().starts_with("\"path\""))
        .filter_map(|l| l.trim_end().strip_suffix('"')?.rsplit_once('"').map(|(_, p)| p.replace("\\\\", "\\")))
        .map(|lib| PathBuf::from(lib).join("steamapps/common/X4 Foundations"))
        .collect()
}

/// Install folders of every GOG game (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games\<id>`, value `path`).
#[cfg(windows)]
fn gog_game_dirs() -> Vec<PathBuf> {
    use winreg::{enums::HKEY_LOCAL_MACHINE, RegKey};
    let Ok(games) = RegKey::predef(HKEY_LOCAL_MACHINE).open_subkey(r"SOFTWARE\WOW6432Node\GOG.com\Games") else {
        return Vec::new();
    };
    games
        .enum_keys()
        .filter_map(|id| games.open_subkey(id.ok()?).ok()?.get_value::<String, _>("path").ok())
        .map(PathBuf::from)
        .collect()
}

#[cfg(not(windows))]
fn gog_game_dirs() -> Vec<PathBuf> {
    Vec::new()
}

/// Steam's folder (`HKCU\Software\Valve\Steam`, value `SteamPath`).
#[cfg(windows)]
fn steam_registry_dir() -> Option<PathBuf> {
    use winreg::{enums::HKEY_CURRENT_USER, RegKey};
    let path: String = RegKey::predef(HKEY_CURRENT_USER).open_subkey(r"Software\Valve\Steam").ok()?.get_value("SteamPath").ok()?;
    Some(PathBuf::from(path))
}

#[cfg(not(windows))]
fn steam_registry_dir() -> Option<PathBuf> {
    None
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
