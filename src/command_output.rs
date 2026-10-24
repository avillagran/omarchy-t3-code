//! Live output of one command row (orchestration.subscribeCommandOutput), only while the
//! tool inspector follows it. Terminal escapes are stripped for the plain-text view.
use omarchy_t3_client::*;
use serde_json::json;
use std::{
    sync::{atomic::{AtomicU64, Ordering}, Arc},
    time::{Duration, Instant},
};

const LIMIT: usize = 64 * 1024;

/// Drops ANSI/VT escape sequences and carriage-return overwrites from terminal output.
pub fn plain(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\u{1b}' => match chars.peek() {
                Some('[') => {
                    chars.next();
                    while let Some(&n) = chars.peek() {
                        chars.next();
                        if ('@'..='~').contains(&n) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(n) = chars.next() {
                        if n == '\u{7}' || (n == '\u{1b}' && chars.peek() == Some(&'\\')) {
                            if n == '\u{1b}' {
                                chars.next();
                            }
                            break;
                        }
                    }
                }
                _ => {
                    chars.next();
                }
            },
            '\r' => {
                if chars.peek() != Some(&'\n') {
                    if let Some(start) = out.rfind('\n') {
                        out.truncate(start + 1);
                    } else {
                        out.clear();
                    }
                }
            }
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
    }
    out
}

#[derive(Clone, Default)]
pub struct Watch {
    generation: Arc<AtomicU64>,
}
impl Watch {
    pub fn stop(&self) {
        self.generation.fetch_add(1, Ordering::Relaxed);
    }
    pub fn start(&self, env: String, thread: String, item: String, request: String) {
        let generation = self.generation.fetch_add(1, Ordering::Relaxed) + 1;
        let current = self.generation.clone();
        std::thread::spawn(move || {
            let result = (|| -> Result<()> {
                let e = environments()?.into_iter().find(|e| e.id == env).ok_or("unknown_environment")?;
                let c = e.connection(true)?;
                let mut text = String::new();
                let mut truncated = false;
                let mut running = true;
                let mut emitted = Instant::now() - Duration::from_secs(1);
                let mut dirty = false;
                let cancel = || current.load(Ordering::Relaxed) != generation;
                c.subscribe("orchestration.subscribeCommandOutput", json!({"threadId":thread,"itemId":item}), cancel, |frame| {
                    if let Some(f) = frame {
                        let chunk = f["text"].as_str().unwrap_or("");
                        if f["kind"] == "replace" {
                            text.clear();
                        }
                        text.push_str(chunk);
                        if text.len() > LIMIT * 2 {
                            let cut = text.len() - LIMIT;
                            let cut = (cut..text.len()).find(|i| text.is_char_boundary(*i)).unwrap_or(cut);
                            text.drain(..cut);
                            truncated = true;
                        }
                        truncated |= f["truncated"] == true;
                        running = f["running"] != false;
                        dirty = true;
                    }
                    if dirty && (emitted.elapsed() >= Duration::from_millis(120) || !running) {
                        let shown = plain(&text);
                        let shown = if shown.len() > LIMIT { shown[shown.len() - LIMIT..].to_string() } else { shown };
                        crate::emit("command-output", json!({"requestId":request,"text":shown,"running":running,"truncated":truncated}));
                        emitted = Instant::now();
                        dirty = false;
                    }
                    Ok(())
                })
            })();
            if current.load(Ordering::Relaxed) == generation {
                if let Err(error) = result {
                    crate::emit("command-output", json!({"requestId":request,"error":error,"running":false}));
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn strips_escapes_and_carriage_returns() {
        assert_eq!(super::plain("\u{1b}[32mok\u{1b}[0m\n"), "ok\n");
        assert_eq!(super::plain("10%\r50%\r100%\n"), "100%\n");
        assert_eq!(super::plain("\u{1b}]0;title\u{7}done"), "done");
    }
}
