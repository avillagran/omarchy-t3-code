//! Markdown facts ot3 derives from a message, without its Qt renderer: the
//! tables as Markdown and CSV, the searchable text, workspace file references,
//! image descriptors and the inline-code path rule. Byte-for-byte ot3's
//! `markdown_tables`, `markdown_search_text`, `file_references`,
//! `markdown_images::descriptors` and `inline_code_path`. `blocks` renders a
//! message as renderer-neutral blocks for hosts without ot3's Qt renderer.
pub mod blocks;

pub use blocks::{blocks, link};
use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

/// Tables of a Markdown message as Markdown and CSV, for the copy actions.
pub fn tables(input: &str) -> Vec<Value> {
    let mut tables = vec![];
    let mut rows: Vec<Vec<String>> = vec![];
    let mut row: Vec<String> = vec![];
    let mut cell = String::new();
    let mut inside = false;
    for event in Parser::new_ext(input, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH) {
        match event {
            Event::Start(Tag::Table(_)) => {
                inside = true;
                rows.clear();
            }
            Event::End(TagEnd::TableCell) => {
                row.push(cell.trim().to_string());
                cell.clear();
            }
            Event::End(TagEnd::TableHead) | Event::End(TagEnd::TableRow) => {
                rows.push(std::mem::take(&mut row));
            }
            Event::End(TagEnd::Table) => {
                inside = false;
                let md = rows
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let line = format!("| {} |", r.iter().map(|c| c.replace('|', "\\|")).collect::<Vec<_>>().join(" | "));
                        if i == 0 {
                            format!("{line}\n|{}|", r.iter().map(|_| " --- ").collect::<Vec<_>>().join("|"))
                        } else {
                            line
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let csv = rows
                    .iter()
                    .map(|r| {
                        r.iter()
                            .map(|c| if c.contains([',', '"', '\n']) { format!("\"{}\"", c.replace('"', "\"\"")) } else { c.clone() })
                            .collect::<Vec<_>>()
                            .join(",")
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                tables.push(json!({"markdown":md,"csv":csv}));
            }
            Event::Text(t) | Event::Code(t) if inside => cell.push_str(&t),
            Event::SoftBreak | Event::HardBreak if inside => cell.push(' '),
            _ => {}
        }
    }
    tables
}

/// The text find-in-conversation matches: what the reader sees, one block per line.
pub fn search_text(input: &str) -> String {
    let mut out = String::new();
    for event in Parser::new_ext(input, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH) {
        match event {
            Event::Text(s) | Event::Code(s) | Event::Html(s) | Event::InlineHtml(s) => out.push_str(&s),
            Event::SoftBreak
            | Event::HardBreak
            | Event::End(TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item | TagEnd::TableCell | TagEnd::TableRow) => out.push('\n'),
            _ => {}
        }
    }
    out
}

/// A path as a `file://` destination: unreserved bytes kept, the rest percent-encoded.
pub fn path_href(path: &str) -> String {
    path.bytes()
        .map(|b| if b.is_ascii_alphanumeric() || b"/._-~:".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
        .collect()
}

/// T3 inlineCodeFilePathCandidate: inline code that names a file or folder.
pub fn inline_code_path(code: &str) -> Option<String> {
    let t = code.trim();
    if t.is_empty() || t.len() > 4096 || t.chars().any(|c| c.is_whitespace() || c == '`' || c.is_control()) || t.contains("://") {
        return None;
    }
    let c = t.replace('\\', "/");
    let (body, position) = match c.rsplit_once(':') {
        Some((b, n)) if !b.is_empty() && !n.is_empty() && n.chars().all(|d| d.is_ascii_digit()) => {
            let b = b
                .rsplit_once(':')
                .filter(|(x, n)| !x.is_empty() && n.chars().all(|d| d.is_ascii_digit()))
                .map(|(x, _)| x)
                .unwrap_or(b);
            (b.to_string(), true)
        }
        _ => (c.clone(), false),
    };
    if !position && !body.contains('/') {
        return None;
    }
    if body.starts_with('/') || body.starts_with("~/") || body.starts_with("./") || body.starts_with("../") {
        return Some(c);
    }
    if body.contains(':') {
        return None;
    }
    let first = body.split('/').next().unwrap_or("");
    let host = first.contains('.')
        && !first.starts_with('.')
        && body.contains('/')
        && first.rsplit('.').next().is_some_and(|tld| tld.len() >= 2 && tld.chars().all(|ch| ch.is_ascii_alphabetic()));
    if host {
        return None;
    }
    let base = body.trim_end_matches('/').rsplit('/').next().unwrap_or("");
    let ext = base
        .rsplit_once('.')
        .is_some_and(|(n, e)| !n.is_empty() && !e.is_empty() && e.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'));
    let version = base.as_bytes().windows(3).any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2].is_ascii_digit());
    if version && !position {
        return None;
    }
    if !position && !ext {
        return None;
    }
    Some(c)
}

/// Workspace files a message links to (at most 30, first occurrence wins).
/// Paths in inline code are already links in the text (`inline_code_path`).
pub fn file_references(input: &str) -> Vec<Value> {
    let mut paths = HashSet::new();
    Parser::new(input)
        .filter_map(|e| match e {
            Event::Start(Tag::Link { dest_url, .. }) => Some(dest_url.to_string()),
            _ => None,
        })
        .filter(|p| !p.starts_with('#') && (!p.contains(':') || p.starts_with("file://")))
        .filter(|p| paths.insert(p.clone()))
        .take(30)
        .map(|p| json!({"name":p.rsplit('/').next().unwrap_or(&p),"path":p}))
        .collect()
}

/// Markdown images as opaque descriptors (`markdown-<sha256 of the source>`);
/// the host resolves each source through its own asset grants.
pub fn images(input: &str) -> Vec<Value> {
    let mut rows = Vec::new();
    let mut current: Option<(String, String)> = None;
    for event in Parser::new_ext(input, Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH) {
        match event {
            Event::Start(Tag::Image { dest_url, .. }) => current = Some((dest_url.to_string(), String::new())),
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, alt)) = &mut current {
                    alt.push_str(&t)
                }
            }
            Event::End(TagEnd::Image) => {
                if let Some((source, alt)) = current.take() {
                    let id = format!("markdown-{:x}", Sha256::digest(source.as_bytes()));
                    rows.push(json!({"id":id,"type":"image","name":if alt.is_empty(){"Image"}else{&alt},"source":source}));
                }
            }
            _ => {}
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_code_paths_follow_t3_rules() {
        for p in ["/home", "/home/node/report.html", "./src/main.rs", "~/notes.md", "src/lib.rs", "src/lib.rs:12:3", "Cargo.toml:4"] {
            assert!(inline_code_path(p).is_some(), "{p}");
        }
        for p in ["rpc_outcome_unknown", "server.updateProvider", "cargo build", "github.com/x/y", "v1.2/3", "https://a.b/c", "a/b", "x: y"] {
            assert!(inline_code_path(p).is_none(), "{p}");
        }
    }
    #[test]
    fn tables_copy_as_markdown_and_csv() {
        let t = tables("| a | b |\n|---|---|\n| 1, x | `y` |\n");
        assert_eq!(t[0]["csv"], "a,b\n\"1, x\",y");
        assert_eq!(t[0]["markdown"], "| a | b |\n| --- | --- |\n| 1, x | y |");
    }
    #[test]
    fn references_images_and_search_text() {
        let md = "See [a](src/a.rs), [a](src/a.rs), [web](https://x.y/z), [top](#top) and ![shot](media/x.png).\n\n# Title";
        assert_eq!(file_references(md), vec![json!({"name":"a.rs","path":"src/a.rs"})], "images and anchors are not file references");
        let img = images(md);
        assert_eq!(img[0]["name"], "shot");
        assert_eq!(img[0]["source"], "media/x.png");
        assert!(img[0]["id"].as_str().unwrap().starts_with("markdown-"));
        assert_eq!(search_text(md), "See a, a, web, top and shot.\nTitle\n");
        assert_eq!(path_href("/a b/ç.rs"), "/a%20b/%C3%A7.rs");
    }
}
