//! Research: which modules sit inside other modules' exclusion zones in real plans?
//!
//! Each zone (`Module::clearance`) is tried as a box starting at its connection
//! and running along its +Z for the `parameters.xml` length, `x` by `y` wide
//! ("full"), or twice that ("half": x and y as half widths). Every other module's
//! collision boxes are tested against it (deeper than 1 m).
//!
//!     cargo run -p x4sd-core --release --example zonecheck -- <plans.xml>...
//!     cargo run -p x4sd-core --release --example zonecheck -- --list   # modules with zones
//!
//! Game dir: `X4_GAME_DIR` or the dev machine's; the profile's extensions are loaded.

use std::path::Path;

use x4sd_core::catalog::{Catalog, ShapeBox};
use x4sd_core::geometry::{Quat, Transform};
use x4sd_core::layout::{penetration, WorldBox};
use x4sd_core::plan::parse_plans;

fn main() {
    let game = std::env::var("X4_GAME_DIR").unwrap_or_else(|_| r"F:\SteamLibrary\steamapps\common\X4 Foundations".to_owned());
    let profile = std::env::var("USERPROFILE").map(|u| format!(r"{u}\Documents\Egosoft\X4\104494216")).unwrap();
    let catalog = Catalog::load(Path::new(&game), Some(Path::new(&profile))).unwrap();
    if std::env::args().nth(1).as_deref() == Some("--list") {
        for info in catalog.station_modules().unwrap() {
            let m = catalog.module(&info.macro_name).unwrap();
            if !m.clearance.is_empty() {
                let zones: Vec<String> = m.clearance.iter().map(|z| {
                    let dir = z.transform.rotation.rotate([0.0, 0.0, 1.0]).map(|v| v.round());
                    format!("{} at {:?} towards {:?}", z.tag, z.transform.position.map(|v| v.round()), dir)
                }).collect();
                println!("{} ({}, {}): {}", info.macro_name, info.class, info.name, zones.join("; "));
            }
        }
        return;
    }
    for (scale, label) in [(0.5, "full"), (1.0, "half")] {
        println!("=== zone width = {label} ({}x the parameters' x and y)", scale * 2.0);
        let (mut zones, mut hit) = (0, 0);
        for file in std::env::args().skip(1) {
            // Some DLC library plans have generated upgrades (drones) the plan reader
            // doesn't know; zones don't depend on equipment, so drop them.
            let mut xml = std::fs::read_to_string(&file).unwrap();
            while let Some(start) = xml.find(r#"<upgrades generated="1">"#) {
                let end = start + xml[start..].find("</upgrades>").unwrap() + "</upgrades>".len();
                xml.replace_range(start..end, "");
            }
            // One plan at a time, so a plan the reader rejects only skips itself.
            let mut plans = Vec::new();
            let mut rest = xml.as_str();
            while let Some(start) = rest.find("<plan ") {
                let end = start + rest[start..].find("</plan>").unwrap() + "</plan>".len();
                match parse_plans(&format!("<plans>{}</plans>", &rest[start..end])) {
                    Ok(p) => plans.extend(p),
                    Err(e) => println!("  skipped a plan in {file}: {e}"),
                }
                rest = &rest[end..];
            }
            for plan in plans {
                let placed: Vec<_> = plan
                    .entries
                    .iter()
                    .filter_map(|e| {
                        let m = catalog.module(&e.macro_name).ok()?;
                        let boxes = catalog.collision_boxes(&e.macro_name).ok()?;
                        let at = Transform::from_offset(e.offset.as_ref());
                        let world: Vec<WorldBox> = boxes.iter().map(|b| WorldBox::new(&at, b)).collect();
                        Some((e.index, m, at, world))
                    })
                    .collect();
                for (index, m, at, _) in &placed {
                    for z in &m.clearance {
                        let Some([x, y, l]) = catalog.exclusion_size(&z.tag) else {
                            println!("  no size for {}", z.tag);
                            continue;
                        };
                        zones += 1;
                        let frame = at.then(&z.transform);
                        let zone = WorldBox::new(
                            &frame,
                            &ShapeBox { transform: Transform { position: [0.0, 0.0, l / 2.0], rotation: Quat::IDENTITY }, half_extents: [x * scale, y * scale, l / 2.0] },
                        );
                        let mut intruders = Vec::new();
                        for (other, om, _, boxes) in &placed {
                            if other == index {
                                continue;
                            }
                            let deepest = boxes.iter().map(|b| penetration(&zone, b)).fold(f64::MIN, f64::max);
                            if deepest > 1.0 {
                                // How far out along the zone the nearest intruding box starts.
                                let inv = frame.inverse();
                                let near = boxes
                                    .iter()
                                    .filter(|b| penetration(&zone, b) > 1.0)
                                    .map(|b| inv.then(&Transform { position: b.centre, rotation: Quat::IDENTITY }).position[2])
                                    .fold(f64::MAX, f64::min);
                                intruders.push(format!("#{other} {} ({:.0} m deep, centre {:.0} m out)", om.macro_name, deepest, near));
                            }
                        }
                        if !intruders.is_empty() {
                            hit += 1;
                            println!("  [{}] #{index} {} {} : {}", plan.id, m.macro_name, z.tag, intruders.join("; "));
                        }
                    }
                }
            }
        }
        println!("  zones: {zones}, with intruders: {hit}");
    }
}
