//! T3 recallableComposerPrompt port. Send-time context is never recalled as fresh context.
const ATTACHMENT_ONLY:&str="[User attached one or more files without additional text. Respond using the conversation context and the attached files.]";
fn whitespace(c: char) -> bool {
    matches!(c,'\u{0009}'..='\u{000d}'|' '|'\u{00a0}'|'\u{1680}'|'\u{2000}'..='\u{200a}'|'\u{2028}'|'\u{2029}'|'\u{202f}'|'\u{205f}'|'\u{3000}'|'\u{feff}')
}
fn trim(value: &str) -> &str {
    value.trim_matches(whitespace)
}
fn trim_end(value: &str) -> &str {
    value.trim_end_matches(whitespace)
}
fn without_trailing_review(prompt: &str) -> &str {
    let mut ranges = vec![];
    let mut cursor = 0;
    while let Some(at) = prompt[cursor..].find("<review_comment") {
        let start = cursor + at;
        let tail = &prompt[start + 15..];
        if tail
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            cursor = start + 15;
            continue;
        }
        let Some(open) = tail.find('>') else { break };
        let body = start + 15 + open + 1;
        let Some(end) = prompt[body..].find("</review_comment>") else {
            break;
        };
        let end = body + end + 17;
        ranges.push((start, end));
        cursor = end;
    }
    let mut cut = prompt.len();
    for (start, end) in ranges.into_iter().rev() {
        if !trim(&prompt[end..cut]).is_empty() {
            break;
        }
        cut = start
    }
    if cut < prompt.len() {
        trim_end(&prompt[..cut])
    } else {
        prompt
    }
}
fn remove_range(prompt: &mut String, start: usize, end: usize) {
    let (mut start, mut end) = (start, end);
    if prompt.as_bytes().get(end) == Some(&b' ') {
        end += 1
    } else if start > 0 && prompt.as_bytes()[start - 1] == b' ' {
        start -= 1
    }
    prompt.replace_range(start..end, "")
}
fn terminal_labels(prompt: &mut String, headers: &[&str]) {
    for header in headers {
        let Some((name, lines)) = header
            .rsplit_once(" lines ")
            .or_else(|| header.rsplit_once(" line "))
        else {
            continue;
        };
        let parts: Vec<_> = lines.split('-').collect();
        if !(1..=2).contains(&parts.len())
            || parts
                .iter()
                .any(|s| s.is_empty() || !s.bytes().all(|c| c.is_ascii_digit()))
        {
            continue;
        }
        let name = trim(name)
            .to_lowercase()
            .split(whitespace)
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("-");
        let label = format!("@{name}:{lines}");
        let found = prompt
            .match_indices(&label)
            .find(|(at, _)| {
                let before = prompt[..*at].chars().next_back();
                let after = prompt[*at + label.len()..].chars().next();
                !before.is_some_and(|c| {
                    c.is_ascii_alphanumeric() || matches!(c, '_' | '@' | '.' | '-')
                }) && !after.is_some_and(|c| c.is_ascii_digit() || c == '-')
            })
            .map(|(at, _)| at);
        if let Some(at) = found {
            remove_range(prompt, at, at + label.len())
        }
    }
}
fn canonical_references(prompt: &mut String) {
    let mut cursor = 0;
    let mut ranges = vec![];
    while let Some(at) = prompt[cursor..].find('[') {
        let at = cursor + at;
        let Some(label_end) = prompt[at + 1..].find(']') else {
            break;
        };
        let label_end = at + 1 + label_end;
        let label = &prompt[at + 1..label_end];
        cursor = at + 1;
        if label.contains('\n')
            || label.encode_utf16().count() > 512
            || !prompt[label_end..].starts_with("](t3-context://v1/")
        {
            continue;
        }
        let href_start = label_end + 2;
        let Some(href_end) = prompt[href_start..].find(')') else {
            continue;
        };
        let href_end = href_start + href_end;
        let href = &prompt[href_start..href_end];
        let rest = &href["t3-context://v1/".len()..];
        if rest.encode_utf16().count() > 200 || rest.chars().any(whitespace) {
            continue;
        }
        let Some((kind, id)) = rest.split_once('/') else {
            continue;
        };
        if kind.is_empty()
            || kind.len() > 40
            || !kind.as_bytes()[0].is_ascii_lowercase()
            || !kind
                .bytes()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
            || id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'_' | b'-'))
        {
            continue;
        }
        let start = if at > 0 && prompt.as_bytes()[at - 1] == b'!' {
            at - 1
        } else {
            at
        };
        ranges.push((start, href_end + 1));
        cursor = href_end + 1;
    }
    for (start, end) in ranges.into_iter().rev() {
        remove_range(prompt, start, end)
    }
}
pub fn recall(message: &str) -> String {
    let mut prompt = trim(message)
        .strip_prefix("Ultrathink:\n")
        .unwrap_or(trim(message))
        .to_string();
    loop {
        let stripped = without_trailing_review(&prompt);
        if stripped.len() != prompt.len() {
            prompt = stripped.to_owned();
            continue;
        }
        let mut legacy = None;
        for tag in ["terminal_context", "element_context", "preview_annotation"] {
            let close = format!("\n</{tag}>");
            let end = trim_end(&prompt);
            if !end.ends_with(&close) {
                continue;
            }
            let body_end = end.len() - close.len();
            let open = format!("<{tag}>\n");
            let Some(at) = prompt[..body_end].find(&open) else {
                continue;
            };
            let mut start = at;
            while start > 0 && prompt.as_bytes()[start - 1] == b'\n' {
                start -= 1
            }
            if legacy
                .as_ref()
                .is_none_or(|(old, _, _, _): &(usize, usize, usize, &str)| start < *old)
            {
                legacy = Some((start, at + open.len(), body_end, tag))
            }
        }
        let Some((start, body_start, end, tag)) = legacy else {
            break;
        };
        let body = &prompt[body_start..end];
        if tag == "preview_annotation"
            && body.lines().any(|l| trim_end(l) == "<preview_annotation>")
            && !body.lines().any(|l| {
                ["Id: ", "Page: ", "Comment: ", "Targets: "]
                    .iter()
                    .any(|key| l.starts_with(key))
            })
        {
            break;
        }
        let headers: Vec<_> = body
            .lines()
            .filter_map(|l| l.strip_prefix("- ")?.strip_suffix(':'))
            .filter(|s| !s.is_empty())
            .collect();
        if tag != "preview_annotation" && headers.is_empty() {
            break;
        }
        let mut next = trim_end(&prompt[..start]).to_owned();
        if tag == "terminal_context" {
            terminal_labels(&mut next, &headers)
        }
        prompt = next;
    }
    canonical_references(&mut prompt);
    let prompt = trim(&prompt);
    if prompt == ATTACHMENT_ONLY || prompt.starts_with("PLEASE IMPLEMENT THIS PLAN:\n") {
        String::new()
    } else {
        prompt.into()
    }
}
/// Port of resolveUserMessagePresentation: who sent a user-role message other than the
/// user, and the text to show. Automations and agents send user-role messages too.
pub fn attribution(m: &serde_json::Value) -> (Option<&'static str>, &str) {
    let text = m["text"].as_str().unwrap_or("");
    if m["role"] != "user" {
        return (None, text);
    }
    if !m["scheduledTaskId"].is_null() {
        return (Some("automation"), text);
    }
    // Legacy automation messages carried a "[Triggered by schedule task: ...]" header.
    let legacy = text
        .strip_prefix("[Triggered by schedule task: ")
        .and_then(|rest| {
            let close = rest.find(']')?;
            if rest[..close].contains(['\r', '\n']) || close == 0 {
                return None;
            }
            let after = &rest[close + 1..];
            after
                .strip_prefix("\r\n\r\n")
                .or_else(|| after.strip_prefix("\n\n"))
                .or_else(|| after.strip_prefix("\r\n\n"))
                .or_else(|| after.strip_prefix("\n\r\n"))
        });
    let legacy_id = m["id"].as_str().is_some_and(|id| {
        id.strip_prefix("scheduled-task-message:")
            .and_then(|rest| {
                let (rest, kind) = rest.rsplit_once(':')?;
                let (task, n) = rest.rsplit_once(':')?;
                Some(
                    matches!(kind, "scheduled" | "manual")
                        && !task.is_empty()
                        && !n.is_empty()
                        && n.bytes().all(|b| b.is_ascii_digit()),
                )
            })
            .unwrap_or(false)
    });
    if let Some(body) = legacy {
        if legacy_id || m["createdBy"] == "agent" {
            return (Some("automation"), body);
        }
    }
    if m["createdBy"] != "agent" {
        (None, text)
    } else if m["creationSource"] == "server" {
        (Some("t3code"), text)
    } else {
        (Some("agent"), text)
    }
}
#[cfg(test)]
mod attribution_tests {
    use super::attribution;
    use serde_json::json;
    #[test]
    fn recall_skips_messages_sent_by_automations_agents_and_t3() {
        let typed = json!({"id":"m1","role":"user","text":"typed"});
        assert_eq!(attribution(&typed), (None, "typed"));
        assert_eq!(attribution(&json!({"id":"m2","role":"user","text":"scheduled","scheduledTaskId":"task"})).0, Some("automation"));
        assert_eq!(
            attribution(&json!({"id":"scheduled-task-message:task:1:scheduled","role":"user","text":"[Triggered by schedule task: Daily]\n\nlegacy scheduled"})),
            (Some("automation"), "legacy scheduled")
        );
        assert_eq!(attribution(&json!({"id":"m3","role":"user","text":"x","createdBy":"agent","creationSource":"mcp"})).0, Some("agent"));
        assert_eq!(attribution(&json!({"id":"m4","role":"user","text":"x","createdBy":"agent","creationSource":"server"})).0, Some("t3code"));
        assert_eq!(attribution(&json!({"id":"m5","role":"user","text":"[Triggered by schedule task: Daily]\n\nby hand"})), (None, "[Triggered by schedule task: Daily]\n\nby hand"));
    }
}
