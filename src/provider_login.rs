//! Sign a provider's own CLI in on one host, for providers T3 cannot sign in
//! itself (Codex and Claude report `setup.canAuthenticate: false`). A terminal
//! here runs the CLI's login on that host: directly for this device, over the
//! connection's SSH for another one. The CLI shows its page and code; nothing
//! is copied between hosts. ot3 then asks T3 to re-check that provider.
use crate::{json, protocol, text, Environment, Result};
use serde_json::Value;
use std::process::{Command, Stdio};

/// The login each CLI offers for a terminal on another machine.
fn login_command(driver: &str) -> Option<&'static str> {
    match driver {
        "codex" => Some("codex login --device-auth"),
        "claudeAgent" => Some("claude auth login"),
        _ => None,
    }
}
fn quote(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "'\\''"))
}
/// The terminal's script: the login on that host (a login shell, so the CLI is
/// on PATH as it is for T3 there), then a pause to read how it ended.
#[cfg(test)]
fn script(driver: &str, ssh: Option<&[String]>) -> Result<String> {
    Ok(terminal_script(login_command(driver).ok_or("provider_login_unsupported")?, ssh))
}
fn terminal_script(login: &str, ssh: Option<&[String]>) -> String {
    let remote = format!("exec \"${{SHELL:-/bin/sh}}\" -lc {}", quote(login));
    let run = match ssh {
        None => remote,
        Some(args) => {
            let mut line = vec!["ssh".to_string(), "-t".into()];
            line.extend(args.iter().map(|a| quote(a)));
            line.push(quote(&remote));
            line.join(" ")
        }
    };
    format!("{run}; printf '\\n%s' 'Press Enter to close.'; read -r _")
}
/// Opens a terminal here running `command` in a login shell, here or over SSH.
pub(crate) fn open_terminal(command: &str, ssh: Option<&[String]>) -> Result<()> {
    let script = terminal_script(command, ssh);
    let mut child = Command::new("omarchy-launch-terminal").args(["sh", "-c", &script])
        .stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null())
        .spawn().map_err(|_| "terminal_launch_failed")?;
    std::thread::spawn(move || { let _ = child.wait(); });
    Ok(())
}
/// `{environment, instanceId, driver}`: opens the terminal. Local or SSH only.
pub fn launch(e: &Environment, input: &Value) -> Result<Value> {
    let driver = text(input, "driver");
    let ssh = if e.id == "local" { None } else { Some(crate::connections::ssh_args_for(&e.id)?) };
    open_terminal(login_command(driver).ok_or("provider_login_unsupported")?, ssh.as_deref())?;
    Ok(json!({"opened": true}))
}
/// Asks T3 on that host to probe the provider again, so a finished login shows.
pub fn refresh(e: &Environment, input: &Value) -> Result<Value> {
    let instance = text(input, "instanceId");
    if instance.is_empty() || instance.len() > 128 { return Err("invalid_provider_setup".into()) }
    protocol::api::provider::refresh(&e.connection(true)?, Some(instance), None, Some(true), None)?;
    Ok(json!({"refreshed": true}))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn runs_each_cli_login_here_or_over_ssh() {
        let here = script("codex", None).unwrap();
        assert!(here.starts_with("exec \"${SHELL:-/bin/sh}\" -lc 'codex login --device-auth';"));
        let args = vec!["-o".to_string(), "ControlPath=/run/user/1/ot3-ssh/%C".into(), "node@NODE.local".into()];
        let there = script("claudeAgent", Some(&args)).unwrap();
        assert!(there.starts_with("ssh -t '-o' 'ControlPath=/run/user/1/ot3-ssh/%C' 'node@NODE.local' 'exec \"${SHELL:-/bin/sh}\" -lc '\\''claude auth login'\\'''"), "{there}");
        assert!(there.ends_with("read -r _"));
        assert!(script("opencode", None).is_err());
    }
}
