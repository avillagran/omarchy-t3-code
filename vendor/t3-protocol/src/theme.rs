//! ot3's colours for an Omarchy theme, from its `colors.toml`. ot3 defines no
//! colours of its own: Omarchy's `Commons/Color.qml` reads the theme's base
//! palette, `T3Palette.qml` the status and project colours, and everything else
//! is the foreground or the accent at an opacity over the background. Each role
//! here is the opaque colour ot3 actually draws: Qt Quick blends `Qt.rgba(c, α)`
//! as a premultiplied 8-bit vertex colour over the background, which is not
//! straight alpha compositing (they differ by one on about one channel in four).
use crate::js;
use serde_json::{json, Map, Value};

type Rgb = [u8; 3];

/// Omarchy Commons' defaults when `colors.toml` has nothing (`Color.qml`).
const DEFAULT_FOREGROUND: Rgb = [0xca, 0xcc, 0xcc];
const DEFAULT_BACKGROUND: Rgb = [0x10, 0x13, 0x15];
const DEFAULT_ACCENT: Rgb = [0xca, 0xcc, 0xcc];
const DEFAULT_URGENT: Rgb = [0xa5, 0x55, 0x55];

/// The roles drawn as a colour at an opacity over the background, with the QML that draws them.
pub const LAYERS: &[(&str, &str, f64)] = &[
    ("dim", "foreground", 0.57),           // T3Panel.qml:381, T3ThreadRow.qml:91
    ("statusMuted", "foreground", 0.6),    // T3Palette.qml:33, T3Approvals.qml:14, T3SubagentCards.qml:20
    ("divider", "foreground", 0.09),       // T3Panel.qml:382
    ("surface", "foreground", 0.045),      // T3Panel.qml:387 (bubbles, code blocks)
    ("card", "foreground", 0.035),         // T3Approvals.qml:18
    ("cardBorder", "foreground", 0.12),    // T3Approvals.qml:18
    ("changesCard", "foreground", 0.025),  // T3Changes.qml:14
    ("rowSelected", "foreground", 0.075),  // T3ThreadRow.qml:179
    ("rowPressed", "foreground", 0.045),   // T3ThreadRow.qml:179 (hover)
    ("rowDraft", "accent", 0.06),          // T3ThreadRow.qml:179
    ("field", "foreground", 0.04),         // T3TextField.qml:20
    ("fieldBorder", "foreground", 0.14),   // T3TextField.qml:22
    ("switchOff", "foreground", 0.15),     // T3Switch.qml:14
    ("inlineCode", "foreground", 0.10),    // T3Panel.qml:725
    ("composerBorder", "foreground", 0.09), // T3Composer.qml:265
    ("composerFocus", "accent", 0.45),     // T3Composer.qml:265
];
/// T3Palette.projectColors' keys, in slot order (duplicates removed).
const PROJECT_KEYS: &[&str] = &["blue", "green", "magenta", "cyan", "orange", "yellow", "red", "bright_blue", "bright_magenta"];
/// The theme colours `base` lists, as ot3 reads them.
const BASE_KEYS: &[(&str, &str)] = &[
    ("red", "red"), ("orange", "orange"), ("yellow", "yellow"), ("green", "green"), ("cyan", "cyan"), ("blue", "blue"),
    ("magenta", "magenta"), ("brightBlue", "bright_blue"), ("brightMagenta", "bright_magenta"),
];
/// Sidebar row details and their colours (`T3Palette.detailColor`).
const DETAILS: &[&str] = &["working", "goal", "done", "failed", "limited", "woke", "approval", "input", "plan", "waiting", ""];

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c[0], c[1], c[2])
}
fn parse(s: &str) -> Option<Rgb> {
    let s = s.strip_prefix('#')?;
    (s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit())).then(|| {
        let byte = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).unwrap();
        [byte(0), byte(2), byte(4)]
    })
}
/// `^\s*(key)\s*=\s*` with JavaScript's `\s`; `dash` admits `-` in the key.
fn key(line: &str, dash: bool) -> Option<(&str, &str)> {
    let line = line.trim_start_matches(js::is_space);
    let end = line.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || dash && c == '-')).unwrap_or(line.len());
    if end == 0 {
        return None;
    }
    let rest = line[end..].trim_start_matches(js::is_space).strip_prefix('=')?;
    Some((&line[..end], rest.trim_start_matches(js::is_space)))
}
/// Omarchy Commons' `loadColors`: `key = "#rrggbb"` with or without quotes;
/// every later line wins.
fn commons_values(raw: &str) -> Vec<(&str, Rgb)> {
    raw.split('\n')
        .filter_map(|line| {
            let (k, rest) = key(line, true)?;
            let rest = rest.strip_prefix(['"', '\'']).unwrap_or(rest);
            Some((k, parse(rest.get(..7)?)?))
        })
        .collect()
}
/// T3Palette's `load`: only quoted `key = "#rrggbb"`, the text as written.
fn palette_values(raw: &str) -> Map<String, Value> {
    let mut values = Map::new();
    for line in raw.split('\n') {
        let Some((k, rest)) = key(line, false) else { continue };
        let Some(rest) = rest.strip_prefix(['"', '\'']) else { continue };
        let Some(value) = rest.get(..7).filter(|v| parse(v).is_some()) else { continue };
        if rest[7..].starts_with(['"', '\'']) {
            values.insert(k.to_string(), json!(value));
        }
    }
    values
}

/// What Commons' `Color` holds after reading the file.
struct Commons {
    foreground: Rgb,
    background: Rgb,
    accent: Rgb,
    muted: Rgb,
    urgent: Rgb,
}
fn commons(raw: &str) -> Commons {
    let (mut foreground, mut background, mut accent, mut muted, mut urgent) = (None, None, None, None, DEFAULT_URGENT);
    let (mut color0, mut color4, mut color7, mut color8) = (None, None, None, None);
    for (k, c) in commons_values(raw) {
        match k {
            "foreground" => foreground = Some(c),
            "background" => background = Some(c),
            "accent" => accent = Some(c),
            "muted" => muted = Some(c),
            "color0" => color0 = Some(c),
            "color4" => color4 = Some(c),
            "color7" => color7 = Some(c),
            "color8" => color8 = Some(c),
            "red" | "color1" => urgent = c,
            _ => {}
        }
    }
    let foreground = foreground.or(color7).unwrap_or(DEFAULT_FOREGROUND);
    Commons {
        background: background.or(color0).unwrap_or(DEFAULT_BACKGROUND),
        accent: accent.or(color4).unwrap_or(DEFAULT_ACCENT),
        muted: muted.or(color8).unwrap_or(foreground),
        foreground,
        urgent,
    }
}

/// `qRound` of a float.
fn qround(x: f32) -> i64 {
    if x >= 0.0 { (x + 0.5) as i64 } else { (x - 0.5) as i64 }
}
/// QColor's 16-bit alpha of `Qt.rgba(…, a)`.
fn alpha16(a: f64) -> i64 {
    qround((a as f32) * 65535.0)
}
/// The pixel Qt Quick draws for `Qt.rgba(c, a)` over the opaque `bg`: the
/// rectangle's colour as premultiplied bytes (`colorToColor4ub`), blended over
/// the background.
pub fn drawn(c: Rgb, a: f64, bg: Rgb) -> Rgb {
    let af = alpha16(a) as f32 / 65535.0;
    let premultiplied = |v: u8| qround((f32::from(v) * 257.0 / 65535.0) * af * 255.0) as f64;
    let big_a = qround(af * 255.0) as f64;
    let mut out = [0u8; 3];
    for i in 0..3 {
        // Never an exact tie: bg·(255−A)/255 is not k + ½ for integers.
        out[i] = (premultiplied(c[i]) + f64::from(bg[i]) * (255.0 - big_a) / 255.0).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// QColor's 16-bit channels.
fn rgb16(c: Rgb) -> [i64; 3] {
    c.map(|v| i64::from(v) * 257)
}
/// A 16-bit QColor channel as the 8-bit one Qt reports and draws: x / 257,
/// rounded (never a tie, 257 being odd).
fn to8(x: i64) -> u8 {
    ((2 * x + 257) / 514) as u8
}
fn fuzzy_compare(p1: f32, p2: f32) -> bool {
    (p1 - p2).abs() * 100000.0 <= p1.abs().min(p2.abs())
}
/// `Qt.darker(c, factor)`: `QColor::darker`, the 16-bit HSV value divided by the factor.
pub fn darker(c: Rgb, factor: f64) -> Rgb {
    let factor = qround((factor * 100.0) as f32);
    let [r, g, b] = rgb16(c).map(|v| v as f32 / 65535.0);
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let delta = max - min;
    let value = qround(max * 65535.0) * 100 / factor;
    if delta.abs() <= 0.00001 {
        return [to8(value); 3];
    }
    let mut hue = if fuzzy_compare(r, max) {
        (g - b) / delta
    } else if fuzzy_compare(g, max) {
        2.0 + (b - r) / delta
    } else {
        4.0 + (r - g) / delta
    };
    hue *= 60.0;
    if hue < 0.0 {
        hue += 360.0;
    }
    let (hue, saturation) = (qround(hue * 100.0), qround((delta / max) * 65535.0));
    let h = if hue == 36000 { 0.0 } else { hue as f32 / 6000.0 };
    let s = saturation as f32 / 65535.0;
    let v = value as f32 / 65535.0;
    let i = h as i64;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    let (r, g, b) = match i {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    [r, g, b].map(|x| to8(qround(x * 65535.0)))
}
/// `Qt.tint(base, Qt.rgba(c, a))`.
pub fn tint(base: Rgb, c: Rgb, a: f64) -> Rgb {
    let a = alpha16(a) as f32 / 65535.0;
    let inv = 1.0 - f64::from(a);
    let (c, base) = (rgb16(c), rgb16(base));
    let mut out = [0u8; 3];
    for i in 0..3 {
        let mixed = f64::from(c[i] as f32 / 65535.0) * f64::from(a) + f64::from(base[i] as f32 / 65535.0) * inv;
        out[i] = to8(qround((mixed as f32) * 65535.0));
    }
    out
}
fn unit(c: Rgb) -> [f64; 3] {
    c.map(|v| f64::from(v) / 255.0)
}
/// Rec.709 luminance, ot3's sky rule (`T3Starfield.qml:58`).
pub fn luminance(c: Rgb) -> f64 {
    let [r, g, b] = unit(c);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

/// Title case of a theme directory name (`tokyo-night` → `Tokyo Night`).
fn title(id: &str) -> String {
    id.split(['-', '_', ' '])
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut c = w.chars();
            c.next().map(|f| f.to_uppercase().chain(c).collect::<String>()).unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// ot3's colour roles for a theme's `colors.toml` contents, every one an opaque
/// `#rrggbb`. `id` is the theme's directory name, when known.
pub fn tokens(colors_toml: &str, id: Option<&str>) -> Value {
    let c = commons(colors_toml);
    let values = palette_values(colors_toml);
    let (fg, bg, accent) = (c.foreground, c.background, c.accent);
    // T3Palette.pick: the file's colour, else the fallback.
    let pick = |k: &str, fallback: Rgb| values.get(k).and_then(Value::as_str).and_then(parse).unwrap_or(fallback);
    let failed = c.urgent;
    let palette = [
        ("working", pick("blue", accent)),
        ("done", pick("green", accent)),
        ("waiting", pick("yellow", accent)),
        ("failed", failed),
        ("warning", pick("orange", pick("yellow", accent))),
        ("input", pick("magenta", accent)),
    ];
    let mut roles = Map::new();
    let mut layers = Map::new();
    for (role, over, alpha) in LAYERS {
        let color = if *over == "accent" { accent } else { fg };
        roles.insert(role.to_string(), json!(hex(drawn(color, *alpha, bg))));
        layers.insert(role.to_string(), json!({"color":over,"alpha":alpha}));
    }
    for (role, color) in palette {
        roles.insert(role.into(), json!(hex(color)));
    }
    // Text on the accent (selection, primary buttons): the background, as Qt's highlightedText.
    roles.insert("onAccent".into(), json!(hex(bg)));
    let status = |detail: &str| -> Value {
        let color = match detail {
            "working" | "goal" => palette[0].1,
            "done" => palette[1].1,
            "failed" => failed,
            "limited" | "woke" | "approval" => palette[4].1,
            "input" => palette[5].1,
            "plan" => palette[2].1,
            _ => drawn(fg, 0.6, bg),
        };
        json!(hex(color))
    };
    let mut project = Vec::<Rgb>::new();
    for k in PROJECT_KEYS {
        let color = pick(k, accent);
        if !project.contains(&color) {
            project.push(color);
        }
    }
    let mut base = Map::new();
    for (name, color) in [("background", bg), ("foreground", fg), ("accent", accent), ("muted", c.muted)] {
        base.insert(name.into(), json!(hex(color)));
    }
    for (name, k) in BASE_KEYS {
        base.insert(name.to_string(), values.get(*k).and_then(Value::as_str).and_then(parse).map(hex).map_or(Value::Null, Value::from));
    }
    let light_sky = luminance(bg) > 0.5;
    let [r, g, b] = unit(bg);
    let lightness = (r.max(g).max(b) + r.min(g).min(b)) / 2.0;
    json!({
        "id": id,
        "name": id.map(title),
        "appearance": if light_sky { "light" } else { "dark" },
        // Four decimals: enough to see how far from the 0.5 line a theme is.
        "luminance": (luminance(bg) * 10000.0).round() / 10000.0,
        "mode": mode(colors_toml).unwrap_or(Value::Null),
        "base": base,
        "roles": roles,
        "layers": layers,
        "statusColors": DETAILS.iter().map(|d| (d.to_string(), status(d))).collect::<Map<_, _>>(),
        "projectPalette": project.iter().copied().map(hex).collect::<Vec<_>>(),
        "projectFills": project.iter().map(|p| hex(drawn(*p, 0.14, bg))).collect::<Vec<_>>(),
        "sky": {
            "lightSky": light_sky,
            "star": hex(fg),
            "meteor": if light_sky { hex(fg) } else { "#ffffff".into() },
            "ink": if light_sky { 1.7 } else { 1.0 },
            "header": {"brightness": if light_sky { 0.9 } else { 1.25 }, "nebula": 0.55, "meteorRate": 0.85,
                "base": hex(if light_sky { tint(bg, fg, 0.06) } else { darker(bg, 2.4) })},
            "panel": {"brightness": 0.5, "nebula": 0.0, "meteorRate": 0.3},
        },
        "document": document(bg, fg, accent, &values),
        "mcpTheme": if lightness < 0.5 { "dark" } else { "light" },
    })
}
/// The theme's own `mode = "dark"|"light"`, which ot3 does not read (cross-check only).
fn mode(raw: &str) -> Option<Value> {
    raw.split('\n').find_map(|line| {
        let (k, rest) = key(line, false)?;
        (k == "mode").then(|| json!(rest.trim().trim_matches(['"', '\''])))
    })
}
/// `DocumentTheme.theme(bg, fg, accent, T3Palette.values)`: the CSS variables
/// ot3 gives web documents and MCP apps (`T3Backend.qml:1087`).
fn document(bg: Rgb, fg: Rgb, accent: Rgb, c: &Map<String, Value>) -> Value {
    let (bg_s, fg_s, accent_s) = (hex(bg), hex(fg), hex(accent));
    let or = |k: &str, fallback: &str| c.get(k).and_then(Value::as_str).filter(|s| !s.is_empty()).unwrap_or(fallback).to_string();
    let dim = c.get("bright_black").or(c.get("brightBlack")).and_then(Value::as_str).filter(|s| !s.is_empty()).map(str::to_string).unwrap_or(fg_s.clone());
    let black = or("black", &bg_s);
    let [r, g, b] = unit(bg);
    let appearance = if r * 0.299 + g * 0.587 + b * 0.114 < 0.5 { "dark" } else { "light" };
    let v = |pairs: &[(&str, String)]| pairs.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<Map<_, _>>();
    let variables = v(&[
        ("--background", bg_s.clone()), ("--foreground", fg_s.clone()), ("--card", black.clone()), ("--card-foreground", fg_s.clone()),
        ("--popover", bg_s.clone()), ("--popover-foreground", fg_s.clone()), ("--muted", black.clone()), ("--muted-foreground", dim.clone()),
        ("--border", dim.clone()), ("--input", dim), ("--ring", accent_s.clone()), ("--primary", accent_s.clone()), ("--primary-foreground", bg_s.clone()),
        ("--accent", accent_s.clone()), ("--accent-foreground", bg_s.clone()), ("--accent-surface", black.clone()), ("--accent-surface-foreground", accent_s.clone()),
        ("--secondary", black.clone()), ("--secondary-foreground", fg_s.clone()), ("--destructive", or("red", &accent_s)), ("--destructive-foreground", fg_s.clone()),
        ("--destructive-surface", black.clone()), ("--warning", or("yellow", &accent_s)), ("--warning-foreground", or("yellow", &accent_s)), ("--warning-surface", black.clone()),
        ("--success", or("green", &accent_s)), ("--success-foreground", or("green", &accent_s)), ("--info", or("blue", &accent_s)), ("--info-foreground", or("blue", &accent_s)),
        ("--code-background", black), ("--code-foreground", fg_s), ("--chart-1", accent_s.clone()), ("--chart-2", or("green", &accent_s)), ("--chart-3", or("blue", &accent_s)),
        ("--chart-4", or("magenta", &accent_s)), ("--chart-5", or("cyan", &accent_s)), ("--chart-6", or("yellow", &accent_s)),
        ("--radius", "0.625rem".into()), ("--font-sans", "sans-serif".into()), ("--font-mono", "monospace".into()),
    ]);
    json!({"appearance":appearance,"variables":variables})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn qt_colour_arithmetic() {
        let (fg, bg) = ([0xa9, 0xb1, 0xd6], [0x1a, 0x1b, 0x26]);
        // tokyo-night: what ot3 draws, not straight compositing (#6c718a).
        assert_eq!(hex(drawn(fg, 0.57, bg)), "#6b718a");
        assert_eq!(hex(drawn(fg, 0.0, bg)), "#1a1b26");
        assert_eq!(hex(drawn(fg, 1.0, bg)), "#a9b1d6");
        // Qt.darker and Qt.tint are checked against Qt on the 22 themes (tests/theme_tokens.rs).
        assert_eq!((to8(1927), to8(65535), to8(0)), (7, 255, 0));
        assert_eq!(title("catppuccin-latte"), "Catppuccin Latte");
    }
    #[test]
    fn commons_and_palette_read_the_file_as_omarchy_and_ot3_do() {
        let raw = "background = '#101010'\ncolor1 = #aa0000\nred = \"#cc0000\"\naccent=\"#ABCDEF\" # comment\nbright-blue = \"#123456\"\nmode = \"dark\"\n";
        let c = commons(raw);
        assert_eq!((c.background, c.urgent, c.accent, c.foreground), ([0x10; 3], [0xcc, 0, 0], [0xab, 0xcd, 0xef], DEFAULT_FOREGROUND));
        let v = palette_values(raw);
        // Unquoted color1 and the dashed key are Commons-only; T3Palette keeps the text as written.
        assert_eq!(v.keys().map(String::as_str).collect::<Vec<_>>(), ["accent", "background", "red"]);
        assert_eq!(v["accent"], "#ABCDEF");
        let t = tokens(raw, Some("x"));
        assert_eq!((t["roles"]["failed"].as_str(), t["mode"].as_str()), (Some("#cc0000"), Some("dark")));
        assert_eq!(tokens("", None)["base"]["background"], "#101315");
    }
}
