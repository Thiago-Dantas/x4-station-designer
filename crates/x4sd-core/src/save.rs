//! Savegames: the player's blueprints (the wares they can build), from
//! `<profile>\save\*.xml.gz` (or plain `.xml`). A save is about 1 GB of XML; its
//! one `<blueprints>` list, of `<blueprint ware="…"/>` lines, comes a quarter of
//! the way in, so the reader streams line by line and stops after it.

use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use flate2::read::MultiGzDecoder;

use crate::catalog::Catalog;
use crate::{Error, Result};

/// The ware ids of the player's blueprints in a save (modules are `module_*`).
pub fn read_blueprints(path: &Path) -> Result<Vec<String>> {
    let mut file = BufReader::new(File::open(path).map_err(|e| Error(format!("{}: {e}", path.display())))?);
    let gzip = file.fill_buf()?.starts_with(&[0x1f, 0x8b]);
    let reader: Box<dyn Read> = if gzip { Box::new(MultiGzDecoder::new(file)) } else { Box::new(file) };
    blueprints(BufReader::with_capacity(1 << 20, reader)).map_err(|e| Error(format!("{}: {e}", path.display())))
}

fn blueprints(mut reader: impl BufRead) -> Result<Vec<String>> {
    let mut line = Vec::new();
    let mut inside = false;
    let mut out = Vec::new();
    loop {
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            return Err(Error("no blueprint list found (not an X4 save?)".to_owned()));
        }
        let tag = line.trim_ascii();
        if !inside {
            if tag.starts_with(b"<blueprints") {
                if tag.ends_with(b"/>") {
                    return Ok(out);
                }
                inside = true;
            }
        } else if tag.starts_with(b"</blueprints") {
            return Ok(out);
        } else if let Some(rest) = tag.strip_prefix(b"<blueprint ware=\"") {
            let end = rest.iter().position(|&b| b == b'"').unwrap_or(rest.len());
            out.push(String::from_utf8_lossy(&rest[..end]).into_owned());
        }
    }
}

/// The station modules (macros, sorted) among blueprint wares: a module ware names
/// its macro in `<component ref>`.
pub fn owned_modules(catalog: &Catalog, blueprints: &[String]) -> Result<Vec<String>> {
    let wares = catalog.wares()?;
    let modules: HashSet<String> = catalog.station_modules()?.into_iter().map(|m| m.macro_name.to_ascii_lowercase()).collect();
    let mut out: Vec<String> = blueprints
        .iter()
        .filter_map(|w| wares.ware(w)?.component.as_ref())
        .filter(|m| modules.contains(&m.to_ascii_lowercase()))
        .cloned()
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

/// The newest save in `<profile>\save` (`.xml.gz` or `.xml`), if any.
pub fn newest_save(profile_dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(profile_dir.join("save"))
        .ok()?
        .flatten()
        .filter(|e| {
            let name = e.file_name().to_string_lossy().to_lowercase();
            name.ends_with(".xml.gz") || name.ends_with(".xml")
        })
        .filter_map(|e| Some((e.metadata().ok()?.modified().ok()?, e.path())))
        .max()
        .map(|(_, p)| p)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAVE: &str = "<?xml version=\"1.0\"?>\n<savegame>\n<info>\n<blueprints>\n<blueprint ware=\"paintmod_0006\"/>\n\
        <blueprint ware=\"module_ter_pier_01\"/>\n</blueprints>\n<blueprint ware=\"not_listed\"/>\n";

    #[test]
    fn reads_the_list_and_stops_after_it() {
        assert_eq!(blueprints(SAVE.as_bytes()).unwrap(), ["paintmod_0006", "module_ter_pier_01"]);
    }

    #[test]
    fn empty_list_and_missing_list() {
        assert_eq!(blueprints("<savegame>\n<blueprints/>\n".as_bytes()).unwrap(), Vec::<String>::new());
        assert!(blueprints("<savegame>\n</savegame>\n".as_bytes()).is_err());
    }

    #[test]
    fn reads_gzip_files() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("x4sd-save-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save_001.xml.gz");
        let mut gz = flate2::write::GzEncoder::new(File::create(&path).unwrap(), flate2::Compression::fast());
        gz.write_all(SAVE.as_bytes()).unwrap();
        gz.finish().unwrap();
        assert_eq!(read_blueprints(&path).unwrap().len(), 2);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
