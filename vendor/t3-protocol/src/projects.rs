//! Native project metadata mutations shared with Omi. No host filesystem actions.
use crate::*;
/// Native host paths are resolved by the chosen environment, not the desktop.
pub fn folder_path(path:&str)->Result<String>{
 let p=path.trim();if p.is_empty()||p.len()>512||p.chars().any(char::is_control)||!(p.starts_with('/')||p=="~"||p.starts_with("~/")){return Err("invalid_folder".into())}Ok(p.to_string())
}
pub fn folders(result:&Value)->Result<Value>{
 let parent=folder_path(result["parentPath"].as_str().ok_or("malformed")?)?;
 let entries=result["entries"].as_array().ok_or("malformed")?.iter().take(1000).filter_map(|e|{
  let name=e["name"].as_str()?;let path=folder_path(e["fullPath"].as_str()?).ok()?;Some(json!({"name":name,"fullPath":path}))
 }).collect::<Vec<_>>();Ok(json!({"parentPath":parent,"entries":entries}))
}
pub fn command(input:&Value)->Result<Value>{
 let id=field(input,"projectId")?;if id.is_empty()||id.len()>2048||id.chars().any(char::is_control){return Err("invalid_project".into())}
 let mut c=json!({"projectId":id});
 match field(input,"action")?{
  "project-rename"=>{let title=field(input,"title")?.trim();if title.is_empty()||title.chars().count()>255||title.chars().any(char::is_control){return Err("invalid_title".into())}c["type"]=json!("project.update");c["title"]=json!(title)},
  "project-settings"=>{
   c["type"]=json!("project.update");let patch=&input["patch"];if !patch.is_object(){return Err("invalid_project".into())}
   for (k,v) in patch.as_object().unwrap(){match k.as_str(){
    "title"=>{let title=v.as_str().map(str::trim).filter(|s|!s.is_empty()&&s.chars().count()<=255&&!s.chars().any(char::is_control)).ok_or("invalid_title")?;c[k]=json!(title)},
    "defaultModelSelection" if v.is_null()||v.is_object()=>{c[k]=v.clone()},
    "autoPull" if v.is_boolean()=>{c[k]=v.clone()},
    "defaultThreadEnvMode" if v.is_null()||matches!(v.as_str(),Some("local"|"worktree"))=>{c[k]=v.clone()},
    _=>return Err("invalid_project_settings".into())
   }}
  },
  "project-remove"=>{if input["confirmed"]!=true{return Err("confirmation_required".into())}c["type"]=json!("project.delete");c["force"]=json!(input["force"]==true)},
  _=>return Err("invalid_command".into())
 }
 Ok(c)
}

/// A project's two-letter monogram, T3's geometry (ot3 `project_identity`): the
/// first letter, then the first digit of the first word, else the last word's
/// initial, else the first word's last letter. Colours belong to the theme.
pub fn monogram(name: &str) -> String {
    let words: Vec<&str> = name.trim().split(|c: char| !c.is_alphanumeric()).filter(|s| !s.is_empty()).collect();
    let first = words.first().copied().unwrap_or("PR");
    let initial = first.chars().next().unwrap_or('P');
    let second = first
        .chars()
        .skip(1)
        .find(|c| c.is_numeric())
        .or_else(|| if words.len() > 1 { words.last().and_then(|s| s.chars().next()) } else { first.chars().last() })
        .unwrap_or(initial);
    format!("{initial}{second}").to_uppercase().chars().take(2).collect()
}

/// Theme keys a project colour comes from, in ot3's order (`T3Palette.projectColors`).
pub const COLOR_KEYS: &[&str] = &["blue", "green", "magenta", "cyan", "orange", "yellow", "red", "bright_blue", "bright_magenta"];

/// A theme's project palette: each of `COLOR_KEYS` with its colour, lowercased,
/// or the accent when the theme lacks it; repeated colours keep their first key.
/// Without a theme (`colors` not an object) every key is its own slot.
pub fn palette(colors: &Value, accent: Option<&str>) -> Vec<(&'static str, Option<String>)> {
    if !colors.is_object() {
        return COLOR_KEYS.iter().map(|k| (*k, None)).collect();
    }
    let pick = |k: &str| colors[k].as_str().filter(|c| !c.is_empty());
    let accent = accent.filter(|a| !a.is_empty()).or_else(|| pick("accent"));
    let mut out: Vec<(&'static str, Option<String>)> = vec![];
    for key in COLOR_KEYS {
        let color = pick(key).or(accent).map(str::to_lowercase);
        if color.is_none() || !out.iter().any(|(_, c)| *c == color) {
            out.push((key, color));
        }
    }
    out
}

/// ot3 `T3Palette.projectColor` for every key at once: the keys sorted by UTF-16
/// code unit, each hashed (`h * 31 + unit`, 32-bit) onto a slot, the next free
/// slot on a collision; once every slot is taken, the hashed one.
pub fn color_slots(keys: &[&str], slots: usize) -> std::collections::HashMap<String, usize> {
    let mut sorted = keys.iter().map(|k| k.encode_utf16().collect::<Vec<_>>()).collect::<Vec<_>>();
    // Not deduplicated: a repeated key takes a second slot, as in ot3.
    sorted.sort();
    let mut used = vec![false; slots.max(1)];
    let mut assignment = std::collections::HashMap::new();
    for units in sorted {
        let hash = units.iter().fold(0u32, |h, u| h.wrapping_mul(31).wrapping_add(u32::from(*u)));
        let mut slot = hash as usize % used.len();
        for _ in 0..used.len() {
            if !used[slot] {
                break;
            }
            slot = (slot + 1) % used.len();
        }
        used[slot] = true;
        assignment.insert(String::from_utf16_lossy(&units), slot);
    }
    assignment
}

/// The chip of each project (`{key, title, projectIcon?}`), as ot3's
/// `T3ProjectIcon` draws it: `label` (emoji, the override text or the
/// monogram), `kind` (`monogram`, `emoji` or `lucide` with `icon`), the colour
/// slot over every listed project, its theme key and, given the theme's
/// `colors`, the colour; `fill` is the tint's opacity behind the label. A
/// `projectIcon.color` naming a theme key wins when the theme has that key.
pub fn identities(projects: &[Value], colors: &Value, accent: Option<&str>) -> Vec<Value> {
    let palette = palette(colors, accent);
    let keys = projects.iter().map(|p| p["key"].as_str().unwrap_or("")).collect::<Vec<_>>();
    let slots = color_slots(&keys, palette.len());
    projects
        .iter()
        .map(|p| {
            let key = p["key"].as_str().unwrap_or("");
            let monogram = monogram(p["title"].as_str().unwrap_or(""));
            let icon = &p["projectIcon"];
            let kind = match icon["kind"].as_str() {
                Some("emoji") => "emoji",
                Some("lucide") => "lucide",
                _ => "monogram",
            };
            let label = match icon["kind"].as_str() {
                Some("emoji") => icon["emoji"].as_str().unwrap_or("").to_string(),
                Some("monogram") => icon["text"].as_str().filter(|t| !t.is_empty()).map(str::to_string).unwrap_or_else(|| monogram.clone()),
                _ => monogram.clone(),
            };
            let slot = slots.get(key).copied().unwrap_or(0);
            let (mut color_key, mut color) = (palette[slot].0.to_string(), palette[slot].1.clone());
            if let Some(own) = icon["color"].as_str().filter(|c| !c.is_empty()) {
                match colors[own].as_str().filter(|c| !c.is_empty()) {
                    Some(c) => (color_key, color) = (own.to_string(), Some(c.to_lowercase())),
                    // Without a theme the host resolves the key itself.
                    None if !colors.is_object() => (color_key, color) = (own.to_string(), None),
                    None => {}
                }
            }
            json!({"key":key,"monogram":monogram,"label":label,"kind":kind,"icon":if kind=="lucide"{icon["name"].clone()}else{Value::Null},
                "colorSlot":slot,"colorKey":color_key,"color":color,"fill":if kind=="monogram"{0.14}else{0.0}})
        })
        .collect()
}
