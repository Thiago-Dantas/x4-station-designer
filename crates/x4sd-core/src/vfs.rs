//! Virtual file system over X4 `.cat`/`.dat` archives.
//!
//! Each `.cat` line is `<path> <size> <unix mtime> <md5>` (paths may contain
//! spaces); the matching `.dat` holds the file bodies back to back in `.cat` order.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use crate::{Error, Result};

#[derive(Default)]
pub struct Vfs {
    dats: Vec<PathBuf>,
    /// Normalized path -> (index into `dats`, offset, size).
    files: HashMap<String, (usize, u64, u64)>,
}

/// Lowercase, forward slashes, no repeated separators (some index values double them).
pub fn normalize(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    for c in path.chars() {
        let c = if c == '\\' { '/' } else { c.to_ascii_lowercase() };
        if c == '/' && out.ends_with('/') {
            continue;
        }
        out.push(c);
    }
    out
}

impl Vfs {
    /// Adds a catalog. Its files appear under `prefix` and override files added earlier.
    pub fn add_catalog(&mut self, cat: &Path, prefix: &str) -> Result<()> {
        let file = File::open(cat).map_err(|e| Error(format!("{}: {e}", cat.display())))?;
        let dat_index = self.dats.len();
        self.dats.push(cat.with_extension("dat"));
        let mut offset = 0;
        for line in BufReader::new(file).lines() {
            let line = line?;
            if line.is_empty() {
                continue;
            }
            let mut parts = line.rsplitn(4, ' ');
            let size = parts.nth(2).and_then(|s| s.parse::<u64>().ok());
            let (Some(size), Some(path)) = (size, parts.next()) else {
                return Err(Error(format!("{}: malformed line: {line}", cat.display())));
            };
            self.files.insert(normalize(&format!("{prefix}{path}")), (dat_index, offset, size));
            offset += size;
        }
        Ok(())
    }

    pub fn contains(&self, path: &str) -> bool {
        self.files.contains_key(&normalize(path))
    }

    pub fn read(&self, path: &str) -> Result<Vec<u8>> {
        let &(dat, offset, size) = self
            .files
            .get(&normalize(path))
            .ok_or_else(|| Error(format!("not in the game files: {path}")))?;
        let mut file = File::open(&self.dats[dat])?;
        file.seek(SeekFrom::Start(offset))?;
        let mut buf = vec![0; size as usize];
        file.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Reads a UTF-8 text file, without the byte order mark some game files have.
    pub fn read_text(&self, path: &str) -> Result<String> {
        let text = String::from_utf8(self.read(path)?).map_err(|_| Error(format!("not UTF-8: {path}")))?;
        Ok(text.strip_prefix('\u{feff}').map(str::to_owned).unwrap_or(text))
    }

    pub fn len(&self) -> usize {
        self.files.len()
    }

    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_collapses_separators_and_case() {
        assert_eq!(normalize(r"Assets\\Structures\\Landmarks\X.xml"), "assets/structures/landmarks/x.xml");
    }
}
