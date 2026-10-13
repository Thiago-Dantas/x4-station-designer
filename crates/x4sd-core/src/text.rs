//! Game texts (`t/0001-lNNN.xml`): `{page,id}` references, resolved the way the
//! game shows them.
//!
//! A text may contain references to other texts (`{20202,101}`), developer
//! comments in parentheses (dropped) and escaped characters (`\(` is a literal
//! parenthesis).

use std::collections::HashMap;

use crate::Result;

#[derive(Default)]
pub struct Texts {
    texts: HashMap<(u32, u32), String>,
}

impl Texts {
    /// Adds the texts of a language file: a full `<language>` document, or a
    /// `<diff>` whose additions contain `<page>` or `<t>` elements.
    pub fn merge(&mut self, xml: &str) -> Result<()> {
        let doc = roxmltree::Document::parse(xml)?;
        for t in doc.descendants().filter(|n| n.has_tag_name("t")) {
            let Some(id) = t.attribute("id").and_then(|v| v.parse().ok()) else { continue };
            let Some(page) = t.ancestors().find_map(page_id) else { continue };
            self.texts.insert((page, id), t.text().unwrap_or_default().to_owned());
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.texts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.texts.is_empty()
    }

    /// Resolves a text as the game displays it, e.g. `{20104,50201}`.
    pub fn resolve(&self, text: &str) -> String {
        self.resolve_depth(text, 0)
    }

    fn resolve_depth(&self, text: &str, depth: u32) -> String {
        let mut out = String::new();
        let mut chars = text.chars().peekable();
        let mut comment_depth = 0;
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    if let Some(next) = chars.next() {
                        if comment_depth == 0 {
                            out.push(next);
                        }
                    }
                }
                '(' => comment_depth += 1,
                ')' if comment_depth > 0 => comment_depth -= 1,
                _ if comment_depth > 0 => {}
                '{' => {
                    let reference: String = chars.by_ref().take_while(|&c| c != '}').collect();
                    let target = reference
                        .split_once(',')
                        .and_then(|(p, i)| Some((p.trim().parse().ok()?, i.trim().parse().ok()?)))
                        .and_then(|key| self.texts.get(&key));
                    match target {
                        Some(t) if depth < 10 => out.push_str(&self.resolve_depth(t, depth + 1)),
                        _ => {
                            out.push('{');
                            out.push_str(&reference);
                            out.push('}');
                        }
                    }
                }
                _ => out.push(c),
            }
        }
        out.trim().to_owned()
    }
}

/// The page id of a `<page id>` element, or of a diff `<add sel="...page[@id='N']...">`.
fn page_id(node: roxmltree::Node) -> Option<u32> {
    if node.has_tag_name("page") {
        return node.attribute("id")?.parse().ok();
    }
    let sel = node.attribute("sel")?;
    let start = sel.find("page[@id=")? + "page[@id=".len();
    let rest = sel[start..].trim_start_matches(['\'', '"']);
    rest[..rest.find(|c: char| !c.is_ascii_digit())?].parse().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_references_comments_and_escapes() {
        let mut t = Texts::default();
        t.merge(
            r#"<language id="44"><page id="1">
                 <t id="1">Argon</t><t id="2">(note){1,1} Base</t><t id="3">\(L\)</t>
               </page></language>"#,
        )
        .unwrap();
        t.merge(r#"<diff><add sel="/language/page[@id='2']"><t id="5">02</t></add></diff>"#).unwrap();
        assert_eq!(t.resolve("(Argon Base 02){1,2} {2,5} {1,3}"), "Argon Base 02 (L)");
        assert_eq!(t.resolve("{9,9}"), "{9,9}");
    }
}
