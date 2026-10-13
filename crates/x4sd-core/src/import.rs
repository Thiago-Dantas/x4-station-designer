//! Getting a plan into the game (D-5): a single-plan file in
//! `<profile>/constructionplan/`, which the player imports from the build menu.
//! The game rescans that folder whenever the build menu opens.

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::plan::{write_plans, Plan};
use crate::{Error, Result};

/// The file name the game gives an exported plan: characters outside
/// `[A-Za-z0-9_-() ]` become `_` (Lua `[^%w_%-%() ]` in the build menu).
pub fn file_name(plan_name: &str) -> String {
    let stem: String = plan_name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || "_-() ".contains(c) { c } else { '_' })
        .collect();
    format!("{stem}.xml")
}

/// A fresh player plan id, `player_<unix seconds>`, the form the game uses.
/// Import reuses the id from the file, and an existing plan with the same id is
/// overwritten (after a prompt), so ids must be unique.
pub fn new_plan_id() -> String {
    let secs = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    format!("player_{secs}")
}

/// Writes `plan` as an import file and returns its path. Refuses to replace an
/// existing file unless `overwrite` is set.
pub fn write_import_file(profile_dir: &Path, plan: &Plan, overwrite: bool) -> Result<PathBuf> {
    let dir = profile_dir.join("constructionplan");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(file_name(&plan.name));
    if path.exists() && !overwrite {
        return Err(Error(format!("{} already exists", path.display())));
    }
    std::fs::write(&path, write_plans(std::slice::from_ref(plan)))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_name_matches_game_export() {
        assert_eq!(file_name("TerranScrapyard"), "TerranScrapyard.xml");
        assert_eq!(file_name("X4SD test B"), "X4SD test B.xml");
        assert_eq!(file_name("Hull/Parts: v2 (big)"), "Hull_Parts_ v2 (big).xml");
    }
}
