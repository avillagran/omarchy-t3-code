//! Bounded, read-only presentation of the native checkpoint diff RPC's `diff` string.
//! The host owns the `orchestration.getTurnDiff` / `getFullThreadDiff` transport.
use serde_json::{json, Value};

const MAX_PATCH_BYTES: usize = 2 * 1024 * 1024;
const MAX_FILES: usize = 256;
const MAX_LINES: usize = 12_000;
const MAX_LINE_BYTES: usize = 16 * 1024;

fn path_tokens(header: &str) -> Option<(String, String)> {
    if !header.starts_with('"') {
        // Git need not quote whitespace in paths. The second ` b/` delimiter separates
        // unquoted names (Git may also show tabs here with core.quotePath disabled).
        let (old, new) = header.rsplit_once(" b/")?;
        return Some((old.strip_prefix("a/")?.to_string(), new.to_string()));
    }
    // Git uses C-style quoted paths, including escaped spaces, tabs, quotes, and octal UTF-8.
    fn next(input: &str) -> Option<(String, &str)> {
        let input = input.trim_start();
        if let Some(rest) = input.strip_prefix('"') {
            let mut bytes = Vec::new();
            let mut iter = rest.as_bytes().iter().copied().enumerate();
            while let Some((i, b)) = iter.next() {
                if b == b'"' { return String::from_utf8(bytes).ok().map(|s| (s, &rest[i + 1..])); }
                if b != b'\\' { bytes.push(b); continue; }
                let (_, escaped) = iter.next()?;
                if (b'0'..=b'7').contains(&escaped) {
                    let mut value = escaped - b'0';
                    for _ in 0..2 { let (_, digit) = iter.next()?; if !(b'0'..=b'7').contains(&digit) { return None; } value = (value << 3) + digit - b'0'; }
                    bytes.push(value);
                } else { bytes.push(match escaped { b'n' => b'\n', b't' => b'\t', b'r' => b'\r', b'b' => 8, b'f' => 12, b'v' => 11, b'a' => 7, other => other }); }
            }
            None
        } else {
            let end = input.find(char::is_whitespace).unwrap_or(input.len());
            if end == 0 { None } else { Some((input[..end].to_string(), &input[end..])) }
        }
    }
    let (old, remaining) = next(header)?;
    let (new, remaining) = next(remaining)?;
    if !remaining.trim().is_empty() { return None; }
    Some((old.strip_prefix("a/")?.to_string(), new.strip_prefix("b/")?.to_string()))
}

fn header_path(input: &str) -> String {
    let input = input.trim();
    if let Some(quoted) = input.strip_prefix('"') {
        // Git's rename header has no a/ prefix; decode with a synthetic prefix.
        path_tokens(&format!("\"a/{} b/dummy", quoted)).map(|(p, _)| p).unwrap_or_else(|| input.to_string())
    } else { input.to_string() }
}

fn hunk_start(line: &str) -> Option<(u64, u64)> {
    let rest = line.strip_prefix("@@ -")?;
    let old = rest.split_once(' ')?.0.split(',').next()?.parse().ok()?;
    let new = rest.split_once(" +")?.1.split_once(" @@")?.0.split(',').next()?.parse().ok()?;
    Some((old, new))
}

fn row(kind: &str, text: &str, old: Option<u64>, new: Option<u64>) -> Value {
    json!({"kind":kind,"text":text,"oldLine":old,"newLine":new})
}

fn split_rows(rows: &[Value]) -> Vec<Value> {
    let mut output = Vec::new();
    let mut i = 0;
    while i < rows.len() {
        let kind = rows[i]["kind"].as_str().unwrap_or("");
        if kind != "deletion" && kind != "addition" {
            output.push(json!({"kind":kind,"left":rows[i],"right":rows[i]}));
            i += 1;
            continue;
        }
        let mut removed = Vec::new();
        let mut added = Vec::new();
        while i < rows.len() && matches!(rows[i]["kind"].as_str(), Some("deletion" | "addition")) {
            if rows[i]["kind"] == "deletion" { removed.push(rows[i].clone()); }
            else { added.push(rows[i].clone()); }
            i += 1;
        }
        for n in 0..removed.len().max(added.len()) {
            output.push(json!({"kind":"change","left":removed.get(n),"right":added.get(n)}));
        }
    }
    output
}

fn finish(file: &mut Value, rows: Vec<Value>, old: &str, new: &str, additions: u64, deletions: u64) {
    let path = if new == "/dev/null" { old } else { new };
    file["path"] = json!(path);
    file["oldPath"] = json!(old);
    file["additions"] = json!(additions);
    file["deletions"] = json!(deletions);
    file["splitRows"] = json!(split_rows(&rows));
    file["rows"] = json!(rows);
}

/// Parse an actual native patch. No filesystem reads or Git mutations. Missing summary paths
/// remain visible as unavailable rather than silently disappearing.
pub fn present(result: &Value, summary: &Value) -> Result<Value, String> {
    let patch = result["diff"].as_str().ok_or("invalid_diff_result")?;
    let summary_files = summary["files"].as_array().cloned().unwrap_or_default();
    if summary_files.len() > MAX_FILES { return Err("diff_limit_files".into()); }
    if patch.len() > MAX_PATCH_BYTES { return Err("diff_limit_2MiB".into()); }
    let mut files = Vec::new();
    let mut current: Option<Value> = None;
    let mut rows = Vec::new();
    let mut old = String::new();
    let mut new = String::new();
    let (mut additions, mut deletions, mut old_line, mut new_line) = (0, 0, None::<u64>, None::<u64>);
    let mut total_lines = 0;
    for line in patch.lines() {
        total_lines += 1;
        if total_lines > MAX_LINES || line.len() > MAX_LINE_BYTES { return Err("diff_limit_lines".into()); }
        if let Some(header) = line.strip_prefix("diff --git ") {
            if let Some(mut file) = current.take() {
                finish(&mut file, rows, &old, &new, additions, deletions);
                files.push(file);
                if files.len() >= MAX_FILES { return Err("diff_limit_files".into()); }
                rows = Vec::new();
            }
            let (before, after) = path_tokens(header).ok_or("invalid_diff_header")?;
            old = before; new = after;
            additions = 0; deletions = 0; old_line = None; new_line = None;
            current = Some(json!({"kind":"modified","binary":false}));
            continue;
        }
        let Some(file) = current.as_mut() else { continue; };
        if let Some(path) = line.strip_prefix("rename from ") { old = header_path(path); file["kind"] = json!("renamed"); }
        else if let Some(path) = line.strip_prefix("rename to ") { new = header_path(path); file["kind"] = json!("renamed"); }
        else if line.starts_with("new file mode ") { file["kind"] = json!("added"); }
        else if line.starts_with("deleted file mode ") { file["kind"] = json!("deleted"); }
        else if line == "+++ /dev/null" { new = "/dev/null".into(); file["kind"] = json!("deleted"); }
        else if line == "--- /dev/null" { old = "/dev/null".into(); file["kind"] = json!("added"); }
        if line.starts_with("Binary files ") || line == "GIT binary patch" { file["binary"] = json!(true); }
        if let Some((before, after)) = hunk_start(line) {
            old_line = Some(before); new_line = Some(after);
            rows.push(row("hunk", line, None, None));
        } else if old_line.is_some() && new_line.is_some() {
            let kind = match line.as_bytes().first() {
                Some(b'+') if !line.starts_with("+++") => "addition",
                Some(b'-') if !line.starts_with("---") => "deletion",
                Some(b' ') => "context",
                _ => "meta",
            };
            match kind {
                "addition" => { additions += 1; let n = new_line.unwrap(); new_line = n.checked_add(1); rows.push(row(kind, line, None, Some(n))); }
                "deletion" => { deletions += 1; let n = old_line.unwrap(); old_line = n.checked_add(1); rows.push(row(kind, line, Some(n), None)); }
                "context" => { let o = old_line.unwrap(); let n = new_line.unwrap(); old_line = o.checked_add(1); new_line = n.checked_add(1); rows.push(row(kind, line, Some(o), Some(n))); }
                _ => rows.push(row(kind, line, None, None)),
            }
        } else { rows.push(row("meta", line, None, None)); }
    }
    if let Some(mut file) = current {
        finish(&mut file, rows, &old, &new, additions, deletions);
        files.push(file);
    }
    for entry in &summary_files {
        let Some(path) = entry["path"].as_str() else { continue; };
        if path.is_empty() { continue; }
        if let Some(file) = files.iter_mut().find(|f| f["path"] == path || f["oldPath"] == path) {
            file["summaryAdditions"] = entry["additions"].clone();
            file["summaryDeletions"] = entry["deletions"].clone();
        } else if files.len() < MAX_FILES {
            files.push(json!({"path":path,"oldPath":path,"kind":entry["kind"],"binary":false,"missing":true,"rows":[],"additions":entry["additions"],"deletions":entry["deletions"]}));
        } else { return Err("diff_limit_files".into()); }
    }
    let additions = files.iter().map(|f| f["additions"].as_u64().unwrap_or(0)).sum::<u64>();
    let deletions = files.iter().map(|f| f["deletions"].as_u64().unwrap_or(0)).sum::<u64>();
    Ok(json!({"files":files,"additions":additions,"deletions":deletions,"fromTurnCount":result["fromTurnCount"],"toTurnCount":result["toTurnCount"]}))
}

/// Check the entire response against the still-open request; failure never reuses an old patch.
pub fn scoped_result(expected: &Value, receipt: &Value, summary: &Value) -> Option<Result<Value, String>> {
    for key in ["requestId", "environment", "thread"] {
        let wanted = expected[key].as_str().filter(|s| !s.is_empty())?;
        if receipt[key].as_str() != Some(wanted) { return None; }
    }
    let from = expected["fromTurnCount"].as_u64()?;
    let to = expected["toTurnCount"].as_u64()?;
    if from > to || summary["fromTurnCount"].as_u64() != Some(from) || summary["toTurnCount"].as_u64() != Some(to) { return None; }
    if let Some(error) = receipt["error"].as_str().filter(|s| !s.is_empty()) { return Some(Err(error.to_string())); }
    if receipt["result"]["threadId"].as_str() != expected["thread"].as_str() { return Some(Err("invalid_diff_thread".into())); }
    if receipt["result"]["fromTurnCount"].as_u64() != Some(from) || receipt["result"]["toTurnCount"].as_u64() != Some(to) { return Some(Err("invalid_diff_range".into())); }
    Some(present(&receipt["result"], summary))
}
