//! Getting a plan into the game (D-5): a single-plan file in
//! `<profile>/constructionplan/`, which the player imports from the build menu.
//! The game rescans that folder whenever the build menu opens.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::design::Design;
use crate::plan::{parse_plans, write_plans, Plan};
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
/// overwritten (after a prompt), so ids must be unique: two ids asked for in
/// the same second differ (the second is one second ahead).
pub fn new_plan_id() -> String {
    static LAST: AtomicU64 = AtomicU64::new(0);
    let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let last = LAST.try_update(Ordering::SeqCst, Ordering::SeqCst, |last| Some(now.max(last + 1))).unwrap_or(0);
    format!("player_{}", now.max(last + 1))
}

/// The player's plan files: the saved plans (`constructionplans.xml`) and the
/// import files in `constructionplan/`, those that exist.
pub fn plan_files(profile_dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = vec![profile_dir.join("constructionplans.xml")];
    if let Ok(dir) = std::fs::read_dir(profile_dir.join("constructionplan")) {
        files.extend(dir.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.extension().is_some_and(|x| x == "xml")));
    }
    files.retain(|f| f.exists());
    files
}

/// A renamed plan becomes a new plan (user decision, 2026-10-14): when the
/// player's plan files have the design's id under another name, the design gets
/// a new id, so the game imports it next to the original instead of replacing
/// it. Returns whether the id changed. Unreadable files are skipped.
pub fn renew_id_if_renamed(profile_dir: &Path, design: &mut Design) -> bool {
    let plan = design.plan();
    let renamed = plan_files(profile_dir)
        .iter()
        .filter_map(|f| parse_plans(&std::fs::read_to_string(f).ok()?).ok())
        .flatten()
        .any(|p| p.id == plan.id && p.name != plan.name);
    if renamed {
        design.set_id(&new_plan_id());
    }
    renamed
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

    #[test]
    fn renamed_plan_becomes_a_new_plan() {
        let profile = std::env::temp_dir().join(format!("x4sd-renamed-{}", std::process::id()));
        std::fs::create_dir_all(&profile).unwrap();
        let mut design = Design::new("A");
        let original = design.plan().id.clone();
        std::fs::write(profile.join("constructionplans.xml"), write_plans(std::slice::from_ref(design.plan()))).unwrap();

        assert!(!renew_id_if_renamed(&profile, &mut design), "same name: the same plan");
        design.rename("B");
        assert!(renew_id_if_renamed(&profile, &mut design), "renamed: a new plan");
        let renewed = design.plan().id.clone();
        assert_ne!(renewed, original, "even within the same second");
        assert!(!renew_id_if_renamed(&profile, &mut design), "the new id is free");
        assert!(design.undo());
        assert_eq!((design.plan().name.as_str(), design.plan().id.as_str()), ("A", renewed.as_str()), "undo keeps the new id");
        std::fs::remove_dir_all(&profile).unwrap();
    }
}
