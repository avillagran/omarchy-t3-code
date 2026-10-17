//! Project icons use native signed assets; the notification daemon only sees a private PNG.
use crate::*;
use image::{DynamicImage, Rgba, RgbaImage};
use std::{
    io::{Cursor, Read, Write},
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    process::{Command, Stdio},
    time::{Duration, SystemTime},
};
const FALLBACK: &[u8] = include_bytes!("../ui/assets/notification-t3.png");

fn decode(bytes: &[u8]) -> Option<DynamicImage> {
    let mut reader = image::ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()?;
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().ok()
}
fn svg(bytes: &[u8]) -> Option<DynamicImage> {
    if bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    let mut child = Command::new("timeout")
        .args(["2s", "rsvg-convert", "-w", "96", "-h", "96"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let written = child.stdin.take()?.write_all(bytes).is_ok();
    let output = child.wait_with_output().ok()?;
    if written && output.status.success() {
        decode(&output.stdout)
    } else {
        None
    }
}
fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn project_override(project: &Value) -> Option<DynamicImage> {
    let icon = &project["projectIcon"];
    let color = match text(icon, "color") {
        "red" => "#ef4444",
        "orange" => "#f97316",
        "amber" | "yellow" => "#eab308",
        "lime" => "#84cc16",
        "green" | "emerald" => "#22c55e",
        "teal" | "cyan" => "#2dd4bf",
        "sky" => "#38bdf8",
        "indigo" => "#818cf8",
        "blue" => "#60a5fa",
        "violet" | "purple" => "#a78bfa",
        "pink" | "fuchsia" | "rose" => "#f472b6",
        _ => "#e5e7eb",
    };
    let body = match text(icon, "kind") {
        "emoji" | "monogram" => {
            let label = if icon["kind"] == "emoji" {
                text(icon, "emoji")
            } else {
                text(icon, "text")
            };
            if label.is_empty() || label.chars().count() > 32 {
                return None;
            }
            format!("<text x='48' y='64' text-anchor='middle' font-family='sans-serif' font-size='52' fill='{color}'>{}</text>",xml(label))
        }
        "lucide" => {
            let name = text(icon, "name");
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            {
                return None;
            }
            static ICONS: std::sync::OnceLock<Value> = std::sync::OnceLock::new();
            let icons = ICONS.get_or_init(|| {
                serde_json::from_str(include_str!("../ui/assets/lucide-project-icons.json"))
                    .expect("bundled Lucide atlas")
            });
            let source = icons[name].as_str()?;
            return svg(source.replace("currentColor", color).as_bytes());
        }
        _ => return None,
    };
    svg(
        format!("<svg xmlns='http://www.w3.org/2000/svg' width='96' height='96'>{body}</svg>")
            .as_bytes(),
    )
}
fn favicon(project: &Value, environment: &str) -> Option<DynamicImage> {
    let cwd = text(project, "workspaceRoot");
    if cwd.is_empty() {
        return None;
    }
    let e = environments()
        .ok()?
        .into_iter()
        .find(|e| e.id == environment)?;
    let connection = if let Some(c) = e.connection {
        c.as_ref().clone()
    } else {
        protocol::Connection::new(
            &e.base_url,
            None,
            protocol::Credential::bearer(&environment_token(&e, false).ok()?).ok()?,
        )
        .ok()?
    };
    let grant = connection
        .rpc(
            "assets.createUrl",
            json!({"resource":{"_tag":"project-favicon","cwd":cwd,"path":project["faviconPath"]}}),
        )
        .ok()?;
    let base = reqwest::Url::parse(connection.base_url()).ok()?;
    let url = base.join(text(&grant, "relativeUrl")).ok()?;
    if url.origin() != base.origin()
        || !url.path().starts_with("/api/assets/")
        || url.path().ends_with("/project-favicon-missing")
    {
        return None;
    }
    let response = reqwest::blocking::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .ok()?
        .get(url)
        .send()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut bytes = Vec::new();
    response
        .take(2 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > 2 * 1024 * 1024 {
        return None;
    }
    decode(&bytes).or_else(|| svg(&bytes))
}
fn line(image: &mut RgbaImage, a: (i32, i32), b: (i32, i32)) {
    let steps = (a.0 - b.0).abs().max((a.1 - b.1).abs()).max(1);
    for i in 0..=steps {
        let x = a.0 + (b.0 - a.0) * i / steps;
        let y = a.1 + (b.1 - a.1) * i / steps;
        for dx in -1..=1 {
            for dy in -1..=1 {
                if (0..96).contains(&(x + dx)) && (0..96).contains(&(y + dy)) {
                    image.put_pixel((x + dx) as u32, (y + dy) as u32, Rgba([255, 255, 255, 255]))
                }
            }
        }
    }
}
pub fn compose(base: DynamicImage, reason: &str) -> RgbaImage {
    let mut image = RgbaImage::new(96, 96);
    let base = base.thumbnail(88, 88).to_rgba8();
    image::imageops::overlay(
        &mut image,
        &base,
        ((96 - base.width()) / 2) as i64,
        ((96 - base.height()) / 2) as i64,
    );
    let color = match reason {
        "response" | "review" => [34, 160, 88, 255],
        "failed" => [215, 55, 65, 255],
        "request" | "waiting" | "plan" => [55, 115, 225, 255],
        _ => return image,
    };
    for x in 56..96 {
        for y in 56..96 {
            let d = (x as i32 - 76).pow(2) + (y as i32 - 76).pow(2);
            if d <= 400 {
                image.put_pixel(x, y, Rgba(if d > 289 { [25, 28, 32, 255] } else { color }))
            }
        }
    }
    match reason {
        "response" | "review" => {
            line(&mut image, (67, 76), (73, 82));
            line(&mut image, (73, 82), (85, 69))
        }
        "failed" => {
            line(&mut image, (69, 69), (83, 83));
            line(&mut image, (83, 69), (69, 83))
        }
        _ => {
            line(&mut image, (65, 76), (81, 76));
            line(&mut image, (76, 70), (82, 76));
            line(&mut image, (82, 76), (76, 82));
            line(&mut image, (85, 67), (85, 85))
        }
    }
    image
}
pub fn path(snapshot: &Value, thread: &Value) -> Result<std::path::PathBuf> {
    let project = snapshot["projects"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| p["environmentId"] == thread["environmentId"] && p["id"] == thread["projectId"])
        .unwrap_or(&Value::Null);
    let root = xdg("XDG_CACHE_HOME", ".cache").join("notification-icons");
    fs::create_dir_all(&root).map_err(|_| "local_error")?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).map_err(|_| "local_error")?;
    let key = format!(
        "{}/{}/{}",
        text(thread, "environmentId"),
        project,
        text(thread, "reason")
    );
    let file = root.join(format!("{:x}.png", Sha256::digest(key.as_bytes())));
    if fs::symlink_metadata(&file).is_ok_and(|m| {
        m.is_file()
            && m.modified()
                .ok()
                .and_then(|t| SystemTime::now().duration_since(t).ok())
                .is_some_and(|d| d < Duration::from_secs(3600))
    }) {
        return Ok(file);
    }
    let base = project_override(project)
        .or_else(|| favicon(project, text(thread, "environmentId")))
        .unwrap_or_else(|| decode(FALLBACK).expect("embedded T3 icon"));
    let image = compose(base, text(thread, "reason"));
    let mut bytes = Cursor::new(Vec::new());
    image
        .write_to(&mut bytes, image::ImageFormat::Png)
        .map_err(|_| "local_error")?;
    let tmp = root.join(format!(".{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)
            .map_err(|_| "local_error")?;
        f.write_all(bytes.get_ref()).map_err(|_| "local_error")?;
        fs::rename(&tmp, &file).map_err(|_| "local_error")?;
        Ok(file.clone())
    })();
    let _ = fs::remove_file(tmp);
    if result.is_ok() {
        let mut entries = fs::read_dir(&root)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let m = e.metadata().ok()?;
                (e.path().extension().is_some_and(|x| x == "png") && m.is_file())
                    .then(|| (e.path(), m.modified().unwrap_or(SystemTime::UNIX_EPOCH)))
            })
            .collect::<Vec<_>>();
        entries.sort_by_key(|e| std::cmp::Reverse(e.1));
        for (path, _) in entries.into_iter().skip(128) {
            if path != file {
                let _ = fs::remove_file(path);
            }
        }
    }
    result
}
