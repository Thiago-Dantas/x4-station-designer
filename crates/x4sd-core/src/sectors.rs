//! Sectors and their sunlight (`libraries/mapdefaults.xml`): where a station
//! goes decides how much its solar modules make.
//!
//! The base game and each DLC ship a full `<defaults>` file of `<dataset
//! macro="...">` entries. A sector's `<area sunlight="...">` applies; a sector
//! without one takes its cluster's (Boron sectors have it on the cluster).
//! Demo sectors and the Timelines mission maps (`timelines_map_*`) are left out.

use crate::text::Texts;
use crate::vfs::Vfs;

#[derive(Debug, Clone, PartialEq)]
pub struct Sector {
    pub macro_name: String,
    pub name: String,
    /// 1.0 = 100%.
    pub sunlight: f64,
}

/// Sectors of the base game and the given extension folders, by name. A later
/// file's dataset replaces an earlier one with the same macro.
pub fn load(vfs: &Vfs, extension_folders: &[String], texts: &Texts) -> Vec<Sector> {
    // Lowercase macro -> (macro, name text, sunlight).
    let mut sets: Vec<(String, Option<String>, Option<f64>)> = Vec::new();
    let files = std::iter::once("libraries/mapdefaults.xml".to_owned())
        .chain(extension_folders.iter().map(|f| format!("extensions/{f}/libraries/mapdefaults.xml")));
    for file in files.filter(|f| vfs.contains(f)) {
        let Ok(xml) = vfs.read_text(&file) else { continue };
        let Ok(doc) = roxmltree::Document::parse(&xml) else { continue };
        for d in doc.descendants().filter(|n| n.has_tag_name("dataset")) {
            let Some(macro_name) = d.attribute("macro") else { continue };
            let find = |tag: &str| d.descendants().find(|n| n.has_tag_name(tag));
            let name = find("identification").and_then(|n| n.attribute("name")).map(str::to_owned);
            let sunlight = find("area").and_then(|n| n.attribute("sunlight")).and_then(|v| v.trim().parse().ok());
            sets.retain(|(m, ..)| !m.eq_ignore_ascii_case(macro_name));
            sets.push((macro_name.to_owned(), name, sunlight));
        }
    }
    let sunlight_of = |m: &str| sets.iter().find(|(x, ..)| x.eq_ignore_ascii_case(m)).and_then(|s| s.2);
    let mut out: Vec<Sector> = sets
        .iter()
        .filter(|(m, ..)| {
            let m = m.to_ascii_lowercase();
            m.contains("_sector") && !m.starts_with("demo_") && !m.starts_with("timelines_map_")
        })
        .filter_map(|(m, name, sunlight)| {
            let cluster = m.to_ascii_lowercase().find("_sector").map(|i| format!("{}_macro", &m[..i]));
            let sunlight = sunlight.or_else(|| cluster.and_then(|c| sunlight_of(&c)))?;
            Some(Sector { macro_name: m.clone(), name: texts.resolve(name.as_deref()?), sunlight })
        })
        .collect();
    out.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()).then(a.macro_name.cmp(&b.macro_name)));
    out
}
