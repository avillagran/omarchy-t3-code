//! Chat Markdown as renderer-neutral blocks with flat inline spans: what ot3's
//! Qt renderer shows, as data a native view draws with no Markdown or link
//! logic of its own. Code, table and image blocks carry the facts behind ot3's
//! actions (run command, Markdown/CSV copy, image descriptor id).
use super::{inline_code_path, path_href, tables};
use crate::latex;
use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

/// Blocks kept per message; parsing stops once this many were emitted.
const MAX_BLOCKS: usize = 4096;
/// Quotes, lists and items kept nested; deeper ones are flattened into their
/// parent, which keeps the JSON well inside common parser depth limits.
const MAX_DEPTH: usize = 32;
/// ot3's link rule: anything else with a scheme is not followed.
const SAFE_SCHEMES: &[&str] = &["http://", "https://", "file://", "t3-thread://"];

/// The blocks of a chat message. `math` is T3's `mathRenderingEnabled`.
pub fn blocks(markdown: &str, math: bool) -> Vec<Value> {
    let normalized;
    let input = if math {
        normalized = latex::normalize_delimiters(markdown);
        normalized.as_str()
    } else {
        markdown
    };
    let mut options = Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH;
    if math {
        options |= Options::ENABLE_MATH;
    }
    let mut builder = Builder { source: markdown, ..Default::default() };
    for event in Parser::new_ext(input, options) {
        if builder.emitted >= MAX_BLOCKS {
            break;
        }
        builder.event(event);
    }
    builder.finish()
}

/// A link destination as `{"href","kind"[,"fileType"]}`, with ot3's glyph
/// classes as kinds; `None` when ot3 would not follow it.
pub fn link(href: &str) -> Option<Value> {
    let lower = href.to_ascii_lowercase();
    let safe = !lower.contains('\0') && (!lower.contains(':') || SAFE_SCHEMES.iter().any(|scheme| lower.starts_with(scheme)));
    if !safe {
        return None;
    }
    if href.is_empty() || href.starts_with('#') {
        return Some(json!({"href":href,"kind":"anchor"}));
    }
    if let Some(rest) = lower.strip_prefix("http://").or_else(|| lower.strip_prefix("https://")) {
        let authority = &rest[..rest.find(['/', '?', '#']).unwrap_or(rest.len())];
        let host = authority.rsplit('@').next().unwrap_or("").split(':').next().unwrap_or("").to_lowercase();
        let host = host.strip_suffix('.').unwrap_or(&host);
        let kind = if host == "github.com" || host.ends_with(".github.com") { "github" } else { "web" };
        return Some(json!({"href":href,"kind":kind}));
    }
    if lower.starts_with("t3-thread://") {
        return Some(json!({"href":href,"kind":"thread"}));
    }
    Some(json!({"href":href,"kind":"file","fileType":file_type(href)}))
}

fn file_type(href: &str) -> &'static str {
    let path = href.split(['?', '#']).next().unwrap_or("").to_lowercase();
    match path.rsplit_once('.').map(|(_, extension)| extension) {
        Some("md" | "mdx" | "markdown") => "markdown",
        Some("rs") => "rust",
        Some("js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "json" | "qml" | "html" | "css" | "py" | "sh" | "toml" | "yaml" | "yml") => {
            "code"
        }
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "ico") => "image",
        Some("pdf") => "pdf",
        _ if path.ends_with('/') => "folder",
        _ => "file",
    }
}

#[derive(Default)]
struct Builder<'a> {
    /// The message as written: run commands and table copies come from it, as in ot3.
    source: &'a str,
    code_blocks: Option<Vec<Value>>,
    table_copies: Option<Vec<Value>>,
    code_count: usize,
    table_count: usize,
    stack: Vec<Frame>,
    /// Open quotes, lists and items that were not flattened.
    depth: usize,
    root: Vec<Value>,
    emitted: usize,
    /// Spans of the open paragraph, heading or table cell.
    inline: Vec<Span>,
    strong: usize,
    emphasis: usize,
    strikethrough: usize,
    links: Vec<Option<Value>>,
    image: Option<Capture>,
}

enum Frame {
    Quote(Vec<Value>),
    List {
        start: Option<u64>,
        items: Vec<Value>,
    },
    Item(Vec<Value>),
    /// A container past `MAX_DEPTH`: its blocks go to the enclosing one.
    Flat,
    /// `implicit` for the bare text of a tight list item.
    Paragraph {
        implicit: bool,
    },
    Heading(u8),
    Code {
        index: usize,
        language: String,
        code: String,
    },
    Html(String),
    Table {
        index: usize,
        alignments: Vec<&'static str>,
        header: Vec<Value>,
        rows: Vec<Value>,
        row: Vec<Value>,
    },
}

/// An image being read: only its text counts, as the alt.
struct Capture {
    depth: usize,
    url: String,
    title: String,
    alt: String,
}

impl Builder<'_> {
    fn event(&mut self, event: Event) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            // ot3 shows raw HTML as literal text.
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => self.text(&text),
            Event::Code(code) => self.inline_code(&code),
            Event::InlineMath(tex) => self.math(&tex, false),
            Event::DisplayMath(tex) => self.math(&tex, true),
            Event::SoftBreak => self.line_break(false),
            Event::HardBreak => self.line_break(true),
            Event::Rule => {
                self.close_implicit();
                self.emit(json!({"type":"rule"}));
            }
            Event::FootnoteReference(_) | Event::TaskListMarker(_) => {}
        }
    }

    fn start(&mut self, tag: Tag) {
        if let Some(capture) = &mut self.image {
            if matches!(tag, Tag::Image { .. }) {
                capture.depth += 1;
            }
            return;
        }
        match tag {
            Tag::Paragraph => self.open(Frame::Paragraph { implicit: false }),
            Tag::Heading { level, .. } => self.open(Frame::Heading(level as u8)),
            Tag::BlockQuote(_) => self.open_container(Frame::Quote(Vec::new()), 1),
            // Room for the list and its items.
            Tag::List(start) => self.open_container(Frame::List { start, items: Vec::new() }, 2),
            Tag::Item => {
                self.close_implicit();
                if matches!(self.stack.last(), Some(Frame::List { .. })) {
                    self.depth += 1;
                    self.stack.push(Frame::Item(Vec::new()));
                } else {
                    self.stack.push(Frame::Flat);
                }
            }
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                let index = self.code_count;
                self.code_count += 1;
                self.open(Frame::Code { index, language, code: String::new() });
            }
            Tag::HtmlBlock => self.open(Frame::Html(String::new())),
            Tag::Table(alignments) => {
                let alignments = alignments.iter().map(alignment).collect();
                let index = self.table_count;
                self.table_count += 1;
                self.open(Frame::Table { index, alignments, header: Vec::new(), rows: Vec::new(), row: Vec::new() });
            }
            Tag::TableCell => self.inline.clear(),
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strikethrough += 1,
            Tag::Link { dest_url, .. } => self.links.push(link(&dest_url)),
            Tag::Image { dest_url, title, .. } => {
                self.image = Some(Capture { depth: 1, url: dest_url.to_string(), title: title.to_string(), alt: String::new() });
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        if let Some(capture) = &mut self.image {
            if tag == TagEnd::Image {
                capture.depth = capture.depth.saturating_sub(1);
            }
            if capture.depth == 0 {
                if let Some(capture) = self.image.take() {
                    let link = self.links.last().cloned().flatten();
                    self.push_span(Span::Image(Image { url: capture.url, alt: capture.alt, title: capture.title, link }));
                }
            }
            return;
        }
        match tag {
            TagEnd::Paragraph
            | TagEnd::Heading(_)
            | TagEnd::BlockQuote(_)
            | TagEnd::CodeBlock
            | TagEnd::HtmlBlock
            | TagEnd::List(_)
            | TagEnd::Item
            | TagEnd::Table => {
                self.close_implicit();
                self.close();
            }
            TagEnd::TableCell => {
                let cell = spans_json(std::mem::take(&mut self.inline));
                if let Some(Frame::Table { row, .. }) = self.stack.last_mut() {
                    row.push(cell);
                }
            }
            TagEnd::TableHead => {
                if let Some(Frame::Table { header, row, .. }) = self.stack.last_mut() {
                    *header = std::mem::take(row);
                }
            }
            TagEnd::TableRow => {
                if let Some(Frame::Table { rows, row, .. }) = self.stack.last_mut() {
                    rows.push(Value::Array(std::mem::take(row)));
                }
            }
            TagEnd::Emphasis => self.emphasis = self.emphasis.saturating_sub(1),
            TagEnd::Strong => self.strong = self.strong.saturating_sub(1),
            TagEnd::Strikethrough => self.strikethrough = self.strikethrough.saturating_sub(1),
            TagEnd::Link => {
                self.links.pop();
            }
            _ => {}
        }
    }

    fn text(&mut self, text: &str) {
        match self.stack.last_mut() {
            Some(Frame::Code { code, .. }) => code.push_str(text),
            Some(Frame::Html(html)) => html.push_str(text),
            _ => match &mut self.image {
                Some(capture) => capture.alt.push_str(text),
                None => self.push_span(Span::Text { text: text.to_string(), style: self.style() }),
            },
        }
    }

    /// Inline code naming a path is a `file://` link, as in ot3; an enclosing
    /// link wins.
    fn inline_code(&mut self, code: &str) {
        if let Some(capture) = &mut self.image {
            capture.alt.push_str(code);
            return;
        }
        let mut style = self.style();
        if style.link.is_none() {
            style.link = inline_code_path(code).and_then(|path| link(&format!("file://{}", path_href(&path))));
        }
        self.push_span(Span::Code { text: code.to_string(), style });
    }

    fn math(&mut self, tex: &str, display: bool) {
        match &mut self.image {
            Some(capture) => capture.alt.push_str(tex),
            None => self.push_span(Span::Math { tex: tex.to_string(), display }),
        }
    }

    fn line_break(&mut self, hard: bool) {
        match &mut self.image {
            Some(capture) => capture.alt.push(' '),
            None => self.push_span(Span::Break { hard }),
        }
    }

    fn style(&self) -> Style {
        Style {
            strong: self.strong > 0,
            emphasis: self.emphasis > 0,
            strikethrough: self.strikethrough > 0,
            link: self.links.last().cloned().flatten(),
        }
    }

    /// Adds a span to the open paragraph, heading or cell (a tight list item's
    /// text opens a paragraph), merging text that looks the same.
    fn push_span(&mut self, span: Span) {
        if !matches!(self.stack.last(), Some(Frame::Paragraph { .. } | Frame::Heading(_) | Frame::Table { .. })) {
            self.stack.push(Frame::Paragraph { implicit: true });
        }
        if let (Span::Text { text, style }, Some(Span::Text { text: last, style: previous })) = (&span, self.inline.last_mut()) {
            if style == previous {
                last.push_str(text);
                return;
            }
        }
        self.inline.push(span);
    }

    fn open(&mut self, frame: Frame) {
        self.close_implicit();
        self.stack.push(frame);
    }

    fn open_container(&mut self, frame: Frame, room: usize) {
        self.close_implicit();
        if self.depth + room <= MAX_DEPTH {
            self.depth += 1;
            self.stack.push(frame);
        } else {
            self.stack.push(Frame::Flat);
        }
    }

    fn close_implicit(&mut self) {
        if matches!(self.stack.last(), Some(Frame::Paragraph { implicit: true })) {
            self.close();
        }
    }

    fn close(&mut self) {
        let Some(frame) = self.stack.pop() else { return };
        match frame {
            Frame::Paragraph { .. } => {
                if let Some(block) = paragraph(std::mem::take(&mut self.inline)) {
                    self.emit(block);
                }
            }
            Frame::Heading(level) => {
                let spans = spans_json(std::mem::take(&mut self.inline));
                self.emit(json!({"type":"heading","level":level,"spans":spans}));
            }
            Frame::Code { index, language, code } => {
                let command = self.command(index);
                self.emit(json!({
                    "type":"code","index":index,"language":language,"code":code,
                    "runnable":command.is_string(),"command":command,
                }));
            }
            Frame::Html(html) => {
                let text = html.trim_end_matches(['\r', '\n']);
                if !text.is_empty() {
                    self.emit(json!({"type":"paragraph","spans":[{"type":"text","text":text}]}));
                }
            }
            Frame::Table { index, alignments, header, rows, .. } => {
                let copy = self.table_copy(index);
                self.emit(json!({
                    "type":"table","alignments":alignments,"header":header,"rows":rows,
                    "markdown":copy["markdown"],"csv":copy["csv"],
                }));
            }
            Frame::Quote(blocks) => {
                self.depth = self.depth.saturating_sub(1);
                self.emit(json!({"type":"quote","blocks":blocks}));
            }
            Frame::List { start, items } => {
                self.depth = self.depth.saturating_sub(1);
                self.emit(json!({"type":"list","ordered":start.is_some(),"start":start,"items":items}));
            }
            Frame::Item(blocks) => {
                self.depth = self.depth.saturating_sub(1);
                match self.stack.last_mut() {
                    Some(Frame::List { items, .. }) => items.push(json!({"blocks":blocks})),
                    _ => blocks.into_iter().for_each(|block| self.place(block)),
                }
            }
            Frame::Flat => {}
        }
    }

    fn emit(&mut self, block: Value) {
        self.emitted += 1;
        self.place(block);
    }

    /// Appends a block to the innermost open container, or the document.
    fn place(&mut self, block: Value) {
        for frame in self.stack.iter_mut().rev() {
            match frame {
                Frame::Quote(blocks) | Frame::Item(blocks) => {
                    blocks.push(block);
                    return;
                }
                Frame::List { items, .. } => {
                    items.push(json!({"blocks":[block]}));
                    return;
                }
                _ => {}
            }
        }
        self.root.push(block);
    }

    /// The `n`-th code block's run command, as ot3 matches `code_blocks::parse`
    /// (the message's own `codeBlocks`).
    fn command(&mut self, index: usize) -> Value {
        let source = self.source;
        let blocks = self.code_blocks.get_or_insert_with(|| crate::code_blocks::parse(source));
        blocks.get(index).map_or(Value::Null, |block| block["command"].clone())
    }

    /// The `n`-th table as Markdown and CSV, for the copy actions.
    fn table_copy(&mut self, index: usize) -> Value {
        let source = self.source;
        let copies = self.table_copies.get_or_insert_with(|| tables(source));
        copies.get(index).cloned().unwrap_or_else(|| json!({"markdown":"","csv":""}))
    }

    fn finish(mut self) -> Vec<Value> {
        while !self.stack.is_empty() {
            self.close();
        }
        self.root
    }
}

/// A paragraph, or the lone image or display formula it consists of.
fn paragraph(mut spans: Vec<Span>) -> Option<Value> {
    if spans.is_empty() {
        return None;
    }
    let mut content = spans.iter().filter(|span| !span.is_blank());
    let alone = match (content.next(), content.next()) {
        (Some(Span::Image(image)), None) => image.link.is_none(),
        (Some(Span::Math { display, .. }), None) => *display,
        _ => false,
    };
    if alone {
        spans.retain(|span| !span.is_blank());
        return spans.pop().map(Span::json);
    }
    Some(json!({"type":"paragraph","spans":spans_json(spans)}))
}

fn spans_json(spans: Vec<Span>) -> Value {
    spans.into_iter().map(Span::json).collect()
}

fn alignment(alignment: &Alignment) -> &'static str {
    match alignment {
        Alignment::None => "none",
        Alignment::Left => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
    }
}

enum Span {
    Text { text: String, style: Style },
    Code { text: String, style: Style },
    Break { hard: bool },
    Image(Image),
    Math { tex: String, display: bool },
}

impl Span {
    fn is_blank(&self) -> bool {
        match self {
            Span::Text { text, .. } => text.trim().is_empty(),
            Span::Break { .. } => true,
            _ => false,
        }
    }

    fn json(self) -> Value {
        match self {
            Span::Text { text, style } => style.apply(json!({"type":"text","text":text})),
            Span::Code { text, style } => style.apply(json!({"type":"code","text":text})),
            Span::Break { hard } => json!({"type":"break","hard":hard}),
            Span::Image(image) => image.json(),
            Span::Math { tex, display } => json!({"type":"math","tex":tex,"display":display}),
        }
    }
}

#[derive(Clone, PartialEq)]
struct Style {
    strong: bool,
    emphasis: bool,
    strikethrough: bool,
    link: Option<Value>,
}

impl Style {
    /// Flags appear only when set.
    fn apply(self, mut span: Value) -> Value {
        for (flag, on) in [("strong", self.strong), ("emphasis", self.emphasis), ("strikethrough", self.strikethrough)] {
            if on {
                span[flag] = Value::Bool(true);
            }
        }
        if let Some(link) = self.link {
            span["link"] = link;
        }
        span
    }
}

/// An image span (`link` when inside one) or, alone in a paragraph, a block.
struct Image {
    url: String,
    alt: String,
    title: String,
    link: Option<Value>,
}

impl Image {
    /// The id is `super::images`' descriptor id for the same source.
    fn json(self) -> Value {
        let id = format!("markdown-{:x}", Sha256::digest(self.url.as_bytes()));
        let mut image = json!({"type":"image","id":id,"url":self.url,"alt":self.alt,"title":self.title});
        if let Some(link) = self.link {
            image["link"] = link;
        }
        image
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(text: &str) -> Value {
        json!({"type":"text","text":text})
    }

    fn kind(href: &str) -> Value {
        link(href).map_or(Value::Null, |link| link["kind"].clone())
    }

    #[test]
    fn math_is_data_only_when_enabled() {
        let on = blocks("Energy $E=mc^2$ and\n\n$$\\sum_i x_i$$\n\nprice \\(a\\)", true);
        assert_eq!(on[0]["spans"], json!([text("Energy "), {"type":"math","tex":"E=mc^2","display":false}, text(" and")]));
        assert_eq!(on[1], json!({"type":"math","tex":"\\sum_i x_i","display":true}));
        assert_eq!(on[2]["spans"], json!([text("price "), {"type":"math","tex":"a","display":false}]));
        assert_eq!(on.len(), 3);
        let off = blocks("Energy $E=mc^2$", false);
        assert_eq!(off, vec![json!({"type":"paragraph","spans":[text("Energy $E=mc^2$")]})]);
    }

    #[test]
    fn inline_code_paths_are_file_links() {
        let spans = &blocks("see `/home/a b` and `/home` and `x`", false)[0]["spans"];
        assert_eq!(spans[1], json!({"type":"code","text":"/home/a b"}));
        assert_eq!(spans[3], json!({"type":"code","text":"/home","link":{"href":"file:///home","kind":"file","fileType":"file"}}));
        assert_eq!(spans[5], json!({"type":"code","text":"x"}));
        let rust = &blocks("`src/lib.rs` [`src/a.rs`](https://x.y)", false)[0]["spans"];
        assert_eq!(rust[0]["link"], json!({"href":"file://src/lib.rs","kind":"file","fileType":"rust"}));
        assert_eq!(rust[2]["link"], json!({"href":"https://x.y","kind":"web"}), "an enclosing link wins");
    }

    #[test]
    fn tables_carry_cells_and_copies() {
        let t = blocks("| a | b |\n|---|---|\n| 1, x | `y` |\n", false);
        assert_eq!(t.len(), 1);
        assert_eq!(t[0]["type"], "table");
        assert_eq!(t[0]["alignments"], json!(["none", "none"]));
        assert_eq!(t[0]["header"], json!([[text("a")], [text("b")]]));
        assert_eq!(t[0]["rows"], json!([[[text("1, x")], [{"type":"code","text":"y"}]]]));
        assert_eq!(t[0]["csv"], "a,b\n\"1, x\",y");
        assert_eq!(t[0]["markdown"], "| a | b |\n| --- | --- |\n| 1, x | y |");
        let aligned = blocks("| l | c | r |\n|:--|:-:|--:|\n| 1 | 2 | 3 |", false);
        assert_eq!(aligned[0]["alignments"], json!(["left", "center", "right"]));
    }

    #[test]
    fn links_follow_ot3_safety_and_glyphs() {
        assert_eq!(link("https://github.com/a/b/tree/main"), Some(json!({"href":"https://github.com/a/b/tree/main","kind":"github"})));
        assert_eq!(kind("https://api.github.com/a"), "github");
        assert_eq!(kind("HTTPS://GitHub.COM./a"), "github");
        assert_eq!(kind("https://github.com.evil.invalid/a"), "web", "host boundary");
        assert_eq!(kind("https://github.com@evil.invalid/a"), "web", "userinfo is not the host");
        assert_eq!(kind("https://user@github.com:443/a"), "github");
        assert_eq!(kind("https://example.org"), "web");
        assert_eq!(kind("#section"), "anchor");
        assert_eq!(kind("t3-thread://x"), "thread");
        for unsafe_href in ["javascript:alert(1)", "JavaScript:alert(1)", "data:text/html,x", "mailto:a@b.c", "src/a\0.rs", "c:/x"] {
            assert_eq!(link(unsafe_href), None, "{unsafe_href}");
        }
        assert_eq!(link("./src/main.rs").unwrap()["fileType"], "rust");
        assert_eq!(link("./README.md").unwrap()["fileType"], "markdown");
        assert_eq!(link("file:///tmp/a.pdf"), Some(json!({"href":"file:///tmp/a.pdf","kind":"file","fileType":"pdf"})));
        assert_eq!(link("docs/").unwrap()["fileType"], "folder");
        assert_eq!(link("ui/Main.QML?x=1#top").unwrap()["fileType"], "code");
        assert_eq!(link("media/shot.JPEG").unwrap()["fileType"], "image");
        assert_eq!(link("LICENSE").unwrap()["fileType"], "file");

        let spans = &blocks("[run](javascript:alert(1)) [top](#top) [a **b**](https://x.y)", false)[0]["spans"];
        let web = json!({"href":"https://x.y","kind":"web"});
        assert_eq!(
            *spans,
            json!([
                text("run "),
                {"type":"text","text":"top","link":{"href":"#top","kind":"anchor"}},
                text(" "),
                {"type":"text","text":"a ","link":web},
                {"type":"text","text":"b","strong":true,"link":web},
            ])
        );
    }

    #[test]
    fn code_blocks_carry_run_commands() {
        let b = blocks("```bash\necho hi\n```\n\n```bash title=x\necho one\necho two\n```\n\n    indented\n", false);
        assert_eq!(b[0], json!({"type":"code","index":0,"language":"bash","code":"echo hi\n","runnable":true,"command":"echo hi"}));
        assert_eq!(b[1], json!({"type":"code","index":1,"language":"bash","code":"echo one\necho two\n","runnable":false,"command":null}));
        assert_eq!(b[2], json!({"type":"code","index":2,"language":"","code":"indented\n","runnable":false,"command":null}));
        let nested = blocks("- x\n\n  ```sh\n  ls\n  ```\n\n```sh\npwd\n```", false);
        assert_eq!(nested[0]["items"][0]["blocks"][1]["index"], 0);
        assert_eq!(nested[1]["command"], "pwd");
    }

    #[test]
    fn structure_and_inline_styles() {
        let headings = blocks("# One\n\n###### Six", false);
        assert_eq!(headings[0], json!({"type":"heading","level":1,"spans":[text("One")]}));
        assert_eq!(headings[1]["level"], 6);

        let list = blocks("3. a\n4. b\n   - c\n", false);
        let item = |t: &str| json!({"blocks":[{"type":"paragraph","spans":[text(t)]}]});
        assert_eq!(
            list[0],
            json!({"type":"list","ordered":true,"start":3,"items":[
                item("a"),
                {"blocks":[
                    {"type":"paragraph","spans":[text("b")]},
                    {"type":"list","ordered":false,"start":null,"items":[item("c")]},
                ]},
            ]})
        );
        let loose = blocks("- a\n\n- b\n", false);
        assert_eq!(loose[0]["items"], json!([item("a"), item("b")]));

        let quote = blocks("> - x\n> - y", false);
        assert_eq!(quote[0]["type"], "quote");
        assert_eq!(quote[0]["blocks"][0]["items"], json!([item("x"), item("y")]));

        assert_eq!(
            blocks("***a*** ~~b~~", false)[0]["spans"],
            json!([{"type":"text","text":"a","strong":true,"emphasis":true}, text(" "), {"type":"text","text":"b","strikethrough":true}])
        );
        assert_eq!(blocks("a  \nb", false)[0]["spans"], json!([text("a"), {"type":"break","hard":true}, text("b")]));
        assert_eq!(blocks("a\nb", false)[0]["spans"], json!([text("a"), {"type":"break","hard":false}, text("b")]));
        assert_eq!(blocks("a\n\n---\n\nb", false)[1], json!({"type":"rule"}));

        let md = "![shot *one*](media/x.png \"Title\")";
        let image = &blocks(md, false)[0];
        assert_eq!(image["id"], super::super::images(md)[0]["id"]);
        assert_eq!(*image, json!({"type":"image","id":image["id"],"url":"media/x.png","alt":"shot one","title":"Title"}));
        let inline = &blocks("see ![a](b.png) here", false)[0];
        assert_eq!(inline["type"], "paragraph");
        assert_eq!(inline["spans"][0], text("see "));
        assert_eq!(inline["spans"][1]["type"], "image");
        assert_eq!(inline["spans"][1]["alt"], "a");
        assert_eq!(inline["spans"][2], text(" here"));
        let badge = &blocks("[![CI](ci.svg)](https://ci.example)", false)[0];
        assert_eq!(badge["type"], "paragraph", "a linked image stays inline");
        assert_eq!(badge["spans"][0]["link"]["kind"], "web");

        assert_eq!(blocks("<div>x</div>", false), vec![json!({"type":"paragraph","spans":[text("<div>x</div>")]})]);
        assert_eq!(blocks("a <b>x</b>", false)[0]["spans"], json!([text("a <b>x</b>")]));
    }

    #[test]
    fn hostile_input_stays_bounded() {
        let nested_list: String = (0..200).map(|i| format!("{}- x\n", "  ".repeat(i))).collect();
        let inputs = [
            "```bash\necho unclosed".to_string(),
            "~~~".to_string(),
            ">".repeat(5000),
            "> ".repeat(5000) + "x",
            "- - - -".to_string(),
            "- ".repeat(3000) + "x",
            "1. ".repeat(3000) + "x",
            nested_list,
            "|".to_string(),
            "| a |\n|".to_string(),
            "$$".to_string(),
            "$$$$\n$".to_string(),
            "\\[".to_string(),
            "a\0b\0```\0".to_string(),
            format!("# {}", "x".repeat(100_000)),
            "#".repeat(10_000),
            "*".repeat(10_000),
            "[".repeat(10_000),
            "![".repeat(1000) + &"](x)".repeat(1000),
            "[a](<b".to_string(),
            "<div>\n".to_string(),
            "> ```\n> x".to_string(),
        ];
        for input in &inputs {
            for math in [false, true] {
                let json = serde_json::to_string(&blocks(input, math)).unwrap();
                assert!(serde_json::from_str::<Value>(&json).is_ok(), "nesting stays parseable");
            }
        }
        let many = blocks(&"a\n\n".repeat(MAX_BLOCKS + 10), false);
        assert_eq!(many.len(), MAX_BLOCKS);
    }
}
