//! `x4sd`: the X4 station designer. Without arguments it opens the visual
//! builder; `x4sd mcp` serves AI agents; the other commands are the CLI (D-8).

mod app;
mod link;
mod mcp;
mod ops;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};
use x4sd_core::catalog::Catalog;
use x4sd_core::game::{find_game_dir, find_profile_dir};
use x4sd_core::import::{new_plan_id, write_import_file};
use x4sd_core::plan::{parse_plans, Plan};
use x4sd_core::validate::validate;
use x4sd_core::{Error, Result};

#[derive(Parser)]
#[command(version, about = "X4 station designer")]
struct Cli {
    /// X4 install folder (default: X4_GAME_DIR, else found through Steam)
    #[arg(long, global = true)]
    game_dir: Option<PathBuf>,
    /// Player profile folder (default: the most recently used Documents\Egosoft\X4\<id>)
    #[arg(long, global = true)]
    profile_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Serve AI agents over MCP (stdio)
    Mcp,
    /// List buildable station modules
    ListModules {
        /// Only this class, e.g. production, habitation, connectionmodule
        #[arg(long)]
        class: Option<String>,
        /// Only this race, e.g. argon, terran
        #[arg(long)]
        race: Option<String>,
        /// Text to look for in the name or macro
        search: Option<String>,
    },
    /// List the snap connections of a module
    Snaps { macro_name: String },
    /// Summarise the plans in a file
    Show { file: PathBuf },
    /// Check plans for problems the game does not catch (exit code 1 if any)
    Validate {
        file: PathBuf,
        /// Only the plan with this name or id
        #[arg(long)]
        plan: Option<String>,
    },
    /// Write a plan into the profile's constructionplan folder for in-game Import
    Export {
        file: PathBuf,
        /// The plan to export, by name or id (required if the file has several)
        #[arg(long)]
        plan: Option<String>,
        /// Give the plan a fresh id, so importing adds it instead of replacing a plan with the same id
        #[arg(long)]
        new_id: bool,
        /// Replace an existing import file of the same name
        #[arg(long)]
        overwrite: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode> {
    let profile = cli.profile_dir.clone().or_else(find_profile_dir);
    let catalog = || -> Result<Catalog> {
        let game = cli.game_dir.clone().or_else(find_game_dir).ok_or_else(|| {
            Error("X4 install not found; pass --game-dir or set X4_GAME_DIR".to_owned())
        })?;
        Catalog::load(&game, profile.as_deref())
    };

    let Some(command) = cli.command else {
        app::run(cli.game_dir.clone().or_else(find_game_dir), profile)?;
        return Ok(ExitCode::SUCCESS);
    };
    match command {
        Command::Mcp => {
            mcp::serve(cli.game_dir.clone().or_else(find_game_dir), profile)?;
        }
        Command::ListModules { class, race, search } => {
            let search = search.map(|s| s.to_lowercase());
            let modules = catalog()?.station_modules()?;
            let shown: Vec<_> = modules
                .iter()
                .filter(|m| class.as_ref().is_none_or(|c| m.class.eq_ignore_ascii_case(c)))
                .filter(|m| race.as_ref().is_none_or(|r| m.race.as_deref().is_some_and(|x| x.eq_ignore_ascii_case(r))))
                .filter(|m| {
                    search.as_ref().is_none_or(|s| m.name.to_lowercase().contains(s) || m.macro_name.to_lowercase().contains(s))
                })
                .collect();
            for m in &shown {
                let source = m.source.as_deref().unwrap_or("base");
                println!("{:<48} {:<17} {:<10} {:<20} {}", m.macro_name, m.class, m.race.as_deref().unwrap_or("-"), source, m.name);
            }
            eprintln!("{} of {} modules", shown.len(), modules.len());
        }
        Command::Snaps { macro_name } => {
            let module = catalog()?.module(&macro_name)?;
            println!("{} ({}, component {})", module.macro_name, module.class, module.component);
            for s in &module.snaps {
                let p = s.transform.position;
                let d = s.transform.rotation.rotate([0.0, 0.0, 1.0]);
                println!("  {:<20} at ({:>8.1}, {:>8.1}, {:>8.1})  facing ({:>5.2}, {:>5.2}, {:>5.2})", s.name, p[0], p[1], p[2], d[0], d[1], d[2]);
            }
        }
        Command::Show { file } => {
            for plan in read_plans(&file)? {
                let patches: Vec<_> = plan.patches.iter().map(|p| p.extension.as_str()).collect();
                let roots = plan.entries.iter().filter(|e| e.predecessor.is_none()).count();
                println!("{} \"{}\": {} modules, {} unattached, patches [{}]", plan.id, plan.name, plan.entries.len(), roots, patches.join(", "));
            }
        }
        Command::Validate { file, plan } => {
            let catalog = catalog()?;
            let mut total = 0;
            for p in select(read_plans(&file)?, plan.as_deref())? {
                let issues = validate(&p, &catalog);
                println!("{} \"{}\": {}", p.id, p.name, if issues.is_empty() { "ok".to_owned() } else { format!("{} issue(s)", issues.len()) });
                for issue in &issues {
                    println!("  {issue}");
                }
                total += issues.len();
            }
            return Ok(if total == 0 { ExitCode::SUCCESS } else { ExitCode::from(1) });
        }
        Command::Export { file, plan, new_id, overwrite } => {
            let profile = profile.ok_or_else(|| Error("player profile not found; pass --profile-dir".to_owned()))?;
            let mut plans = select(read_plans(&file)?, plan.as_deref())?;
            if plans.len() != 1 {
                return Err(Error(format!("{} plans in {}; choose one with --plan", plans.len(), file.display())));
            }
            let mut p = plans.remove(0);
            if new_id {
                p.id = new_plan_id();
            }
            let path = write_import_file(&profile, &p, overwrite)?;
            println!("wrote {} ({} \"{}\")", path.display(), p.id, p.name);
            println!("In game: build menu > Import (the folder is rescanned when the menu opens).");
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn read_plans(file: &Path) -> Result<Vec<Plan>> {
    let xml = std::fs::read_to_string(file).map_err(|e| Error(format!("{}: {e}", file.display())))?;
    parse_plans(&xml).map_err(|e| Error(format!("{}: {e}", file.display())))
}

/// All plans, or the one whose name or id is `wanted`.
fn select(plans: Vec<Plan>, wanted: Option<&str>) -> Result<Vec<Plan>> {
    let Some(w) = wanted else { return Ok(plans) };
    let found: Vec<Plan> = plans.into_iter().filter(|p| p.id == w || p.name == w).collect();
    if found.is_empty() {
        return Err(Error(format!("no plan named or with id \"{w}\"")));
    }
    Ok(found)
}
