//! Construction plan model and XML reader/writer.
//!
//! Reading is strict: an unknown element or attribute is an error, so format
//! surprises surface instead of being dropped silently. Writing follows the
//! game's own style: 2-space indent, LF, all three position coordinates with at
//! most 3 decimals, zero rotation/quaternion components omitted. Numbers are
//! `f32` like in the game, so a read/write round trip keeps every value exactly.

use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Seen only in the game's library plans; kept verbatim.
    pub tags: Option<String>,
    pub useplanname: Option<String>,
    pub fixed: Option<String>,
    /// DLCs/extensions the plan needs.
    pub patches: Vec<Patch>,
    pub entries: Vec<Entry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Patch {
    pub extension: String,
    pub version: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub index: u32,
    pub macro_name: String,
    /// Snap connection on this module that attaches to the predecessor.
    pub connection: Option<String>,
    pub bookmark: Option<String>,
    pub fixed: Option<String>,
    pub predecessor: Option<Predecessor>,
    pub offset: Option<Offset>,
    /// Contents of `<upgrades><groups>`.
    pub upgrades: Vec<Upgrade>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Predecessor {
    pub index: u32,
    /// Snap connection on the predecessor module.
    pub connection: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Offset {
    pub position: [f32; 3],
    pub orientation: Orientation,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Orientation {
    None,
    /// Degrees, written as `<rotation yaw pitch roll>`.
    Euler { yaw: f32, pitch: f32, roll: f32 },
    /// Written as `<quaternion qx qy qz qw>`.
    Quaternion { x: f32, y: f32, z: f32, w: f32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Upgrade {
    /// Element name, e.g. `shields` or `turrets`.
    pub kind: String,
    pub macro_name: String,
    pub path: Option<String>,
    pub group: String,
    pub exact: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlanError(pub String);

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PlanError {}

// ---------------------------------------------------------------- reading

type Node<'a, 'i> = roxmltree::Node<'a, 'i>;

/// Parses a `<plans>` document: `constructionplans.xml` or a single-plan import file.
pub fn parse_plans(xml: &str) -> Result<Vec<Plan>, PlanError> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| PlanError(e.to_string()))?;
    let root = doc.root_element();
    check(root, "plans", &[])?;
    elements(root).map(parse_plan).collect()
}

fn parse_plan(node: Node) -> Result<Plan, PlanError> {
    check(node, "plan", &["id", "name", "description", "tags", "useplanname", "fixed"])?;
    let mut plan = Plan {
        id: req(node, "id")?.to_owned(),
        name: req(node, "name")?.to_owned(),
        description: opt(node, "description"),
        tags: opt(node, "tags"),
        useplanname: opt(node, "useplanname"),
        fixed: opt(node, "fixed"),
        patches: Vec::new(),
        entries: Vec::new(),
    };
    for child in elements(node) {
        match child.tag_name().name() {
            "patches" => {
                check(child, "patches", &[])?;
                for p in elements(child) {
                    check(p, "patch", &["extension", "version", "name"])?;
                    plan.patches.push(Patch {
                        extension: req(p, "extension")?.to_owned(),
                        version: req(p, "version")?.to_owned(),
                        name: req(p, "name")?.to_owned(),
                    });
                }
            }
            "entry" => plan.entries.push(parse_entry(child)?),
            other => return Err(err(child, &format!("unexpected <{other}> in <plan>"))),
        }
    }
    Ok(plan)
}

fn parse_entry(node: Node) -> Result<Entry, PlanError> {
    check(node, "entry", &["index", "macro", "connection", "bookmark", "fixed"])?;
    let mut entry = Entry {
        index: int(node, "index")?,
        macro_name: req(node, "macro")?.to_owned(),
        connection: opt(node, "connection"),
        bookmark: opt(node, "bookmark"),
        fixed: opt(node, "fixed"),
        predecessor: None,
        offset: None,
        upgrades: Vec::new(),
    };
    for child in elements(node) {
        match child.tag_name().name() {
            "predecessor" => {
                check(child, "predecessor", &["index", "connection"])?;
                entry.predecessor = Some(Predecessor {
                    index: int(child, "index")?,
                    connection: req(child, "connection")?.to_owned(),
                });
            }
            "offset" => entry.offset = Some(parse_offset(child)?),
            "upgrades" => {
                check(child, "upgrades", &[])?;
                for groups in elements(child) {
                    check(groups, "groups", &[])?;
                    for u in elements(groups) {
                        check(u, u.tag_name().name(), &["macro", "path", "group", "exact"])?;
                        entry.upgrades.push(Upgrade {
                            kind: u.tag_name().name().to_owned(),
                            macro_name: req(u, "macro")?.to_owned(),
                            path: opt(u, "path"),
                            group: req(u, "group")?.to_owned(),
                            exact: opt(u, "exact"),
                        });
                    }
                }
            }
            other => return Err(err(child, &format!("unexpected <{other}> in <entry>"))),
        }
    }
    Ok(entry)
}

fn parse_offset(node: Node) -> Result<Offset, PlanError> {
    check(node, "offset", &[])?;
    let mut position = None;
    let mut orientation = Orientation::None;
    for child in elements(node) {
        match child.tag_name().name() {
            "position" => {
                check(child, "position", &["x", "y", "z"])?;
                position = Some([num(child, "x")?, num(child, "y")?, num(child, "z")?]);
            }
            "rotation" | "quaternion" if orientation != Orientation::None => {
                return Err(err(child, "<offset> has more than one orientation"));
            }
            "rotation" => {
                check(child, "rotation", &["yaw", "pitch", "roll"])?;
                orientation = Orientation::Euler {
                    yaw: num(child, "yaw")?,
                    pitch: num(child, "pitch")?,
                    roll: num(child, "roll")?,
                };
            }
            "quaternion" => {
                check(child, "quaternion", &["qx", "qy", "qz", "qw"])?;
                orientation = Orientation::Quaternion {
                    x: num(child, "qx")?,
                    y: num(child, "qy")?,
                    z: num(child, "qz")?,
                    w: num(child, "qw")?,
                };
            }
            other => return Err(err(child, &format!("unexpected <{other}> in <offset>"))),
        }
    }
    let position = position.ok_or_else(|| err(node, "<offset> without <position>"))?;
    Ok(Offset { position, orientation })
}

fn elements<'a, 'i>(node: Node<'a, 'i>) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(|n| n.is_element())
}

fn err(node: Node, msg: &str) -> PlanError {
    let pos = node.document().text_pos_at(node.range().start);
    PlanError(format!("line {}: {msg}", pos.row))
}

/// Checks the element name and rejects attributes outside `allowed`.
fn check(node: Node, tag: &str, allowed: &[&str]) -> Result<(), PlanError> {
    if node.tag_name().name() != tag {
        return Err(err(node, &format!("expected <{tag}>, found <{}>", node.tag_name().name())));
    }
    for a in node.attributes() {
        if !allowed.contains(&a.name()) {
            return Err(err(node, &format!("unknown attribute '{}' on <{tag}>", a.name())));
        }
    }
    Ok(())
}

fn req<'a>(node: Node<'a, '_>, name: &str) -> Result<&'a str, PlanError> {
    node.attribute(name)
        .ok_or_else(|| err(node, &format!("missing attribute '{name}' on <{}>", node.tag_name().name())))
}

fn opt(node: Node, name: &str) -> Option<String> {
    node.attribute(name).map(str::to_owned)
}

fn int(node: Node, name: &str) -> Result<u32, PlanError> {
    let v = req(node, name)?;
    v.parse().map_err(|_| err(node, &format!("'{name}' is not an integer: {v}")))
}

/// A missing number is 0: the game omits zero rotation components, and the
/// library plans also omit zero position components.
fn num(node: Node, name: &str) -> Result<f32, PlanError> {
    match node.attribute(name) {
        None => Ok(0.0),
        Some(v) => v.parse().map_err(|_| err(node, &format!("'{name}' is not a number: {v}"))),
    }
}

// ---------------------------------------------------------------- writing

/// Writes a `<plans>` document in the game's style. One plan makes an import file.
pub fn write_plans(plans: &[Plan]) -> String {
    let mut out = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plans>\n");
    for plan in plans {
        write_plan(&mut out, plan);
    }
    out.push_str("</plans>\n");
    out
}

fn write_plan(out: &mut String, plan: &Plan) {
    out.push_str("  <plan");
    attr(out, "id", &plan.id);
    attr(out, "name", &plan.name);
    opt_attr(out, "description", &plan.description);
    opt_attr(out, "tags", &plan.tags);
    opt_attr(out, "useplanname", &plan.useplanname);
    opt_attr(out, "fixed", &plan.fixed);
    out.push_str(">\n");
    if !plan.patches.is_empty() {
        out.push_str("    <patches>\n");
        for p in &plan.patches {
            out.push_str("      <patch");
            attr(out, "extension", &p.extension);
            attr(out, "version", &p.version);
            attr(out, "name", &p.name);
            out.push_str("/>\n");
        }
        out.push_str("    </patches>\n");
    }
    for e in &plan.entries {
        write_entry(out, e);
    }
    out.push_str("  </plan>\n");
}

fn write_entry(out: &mut String, e: &Entry) {
    out.push_str("    <entry");
    attr(out, "index", &e.index.to_string());
    attr(out, "macro", &e.macro_name);
    opt_attr(out, "connection", &e.connection);
    opt_attr(out, "bookmark", &e.bookmark);
    opt_attr(out, "fixed", &e.fixed);
    if e.predecessor.is_none() && e.offset.is_none() && e.upgrades.is_empty() {
        out.push_str("/>\n");
        return;
    }
    out.push_str(">\n");
    if let Some(p) = &e.predecessor {
        out.push_str("      <predecessor");
        attr(out, "index", &p.index.to_string());
        attr(out, "connection", &p.connection);
        out.push_str("/>\n");
    }
    if let Some(o) = &e.offset {
        out.push_str("      <offset>\n        <position");
        for (name, v) in ["x", "y", "z"].iter().zip(o.position) {
            attr(out, name, &fmt_position(v));
        }
        out.push_str("/>\n");
        match o.orientation {
            Orientation::None => {}
            Orientation::Euler { yaw, pitch, roll } => {
                out.push_str("        <rotation");
                nonzero_attrs(out, &[("yaw", yaw), ("pitch", pitch), ("roll", roll)]);
                out.push_str("/>\n");
            }
            Orientation::Quaternion { x, y, z, w } => {
                out.push_str("        <quaternion");
                nonzero_attrs(out, &[("qx", x), ("qy", y), ("qz", z), ("qw", w)]);
                out.push_str("/>\n");
            }
        }
        out.push_str("      </offset>\n");
    }
    if !e.upgrades.is_empty() {
        out.push_str("      <upgrades>\n        <groups>\n");
        for u in &e.upgrades {
            out.push_str("          <");
            out.push_str(&u.kind);
            attr(out, "macro", &u.macro_name);
            opt_attr(out, "path", &u.path);
            attr(out, "group", &u.group);
            opt_attr(out, "exact", &u.exact);
            out.push_str("/>\n");
        }
        out.push_str("        </groups>\n      </upgrades>\n");
    }
    out.push_str("    </entry>\n");
}

fn attr(out: &mut String, name: &str, value: &str) {
    out.push(' ');
    out.push_str(name);
    out.push_str("=\"");
    for c in value.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out.push('"');
}

fn opt_attr(out: &mut String, name: &str, value: &Option<String>) {
    if let Some(v) = value {
        attr(out, name, v);
    }
}

fn nonzero_attrs(out: &mut String, values: &[(&str, f32)]) {
    for &(name, v) in values {
        if v != 0.0 {
            attr(out, name, &fmt_number(v));
        }
    }
}

/// Positions: at most 3 decimals, trailing zeros dropped (the game's format),
/// unless that would change the value (hand-made library plans have values
/// like -0.0001526); then the shortest exact form.
fn fmt_position(v: f32) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    let s = if s == "-0" { "0" } else { s };
    if s.parse::<f32>() == Ok(v) { s.to_owned() } else { fmt_number(v) }
}

/// Angles and quaternion components: the shortest text that reads back as the same `f32`.
fn fmt_number(v: f32) -> String {
    v.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn position_format_matches_game() {
        assert_eq!(fmt_position(530.34), "530.34");
        assert_eq!(fmt_position(-1766.173), "-1766.173");
        assert_eq!(fmt_position(100.0), "100");
        assert_eq!(fmt_position(-0.0), "0");
        assert_eq!(fmt_position(-0.0001526), "-0.0001526");
    }

    #[test]
    fn unknown_attribute_is_an_error() {
        let xml = r#"<plans><plan id="a" name="b" colour="red"/></plans>"#;
        let e = parse_plans(xml).unwrap_err();
        assert!(e.0.contains("unknown attribute 'colour' on <plan>"), "{e}");
    }
}
