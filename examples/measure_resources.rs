//! Read-only Linux PSS/CPU sampler. Does not log command lines or credentials.
use serde_json::{json, Value};
use std::{
    collections::{HashMap, HashSet},
    fs,
    time::{Duration, Instant},
};
struct Proc {
    pid: u32,
    parent: u32,
    start: u64,
    ticks: u64,
    cmd: String,
}
fn read() -> HashMap<u32, Proc> {
    let mut map = HashMap::new();
    for entry in fs::read_dir("/proc").unwrap().flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        let path = entry.path();
        let Ok(raw) = fs::read_to_string(path.join("stat")) else {
            continue;
        };
        let Some(end) = raw.rfind(')') else { continue };
        let f: Vec<_> = raw[end + 2..].split_whitespace().collect();
        let n = |i: usize| f.get(i).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
        let cmd = fs::read(path.join("cmdline"))
            .map(|s| String::from_utf8_lossy(&s).replace('\0', " "))
            .unwrap_or_default();
        map.insert(
            pid,
            Proc {
                pid,
                parent: n(1) as u32,
                start: n(19),
                ticks: n(11) + n(12),
                cmd,
            },
        );
    }
    map
}
fn under(pid: u32, roots: &HashSet<u32>, map: &HashMap<u32, Proc>) -> bool {
    let mut cur = pid;
    for _ in 0..40 {
        if roots.contains(&cur) {
            return true;
        }
        let Some(p) = map.get(&cur) else { return false };
        if p.parent == cur {
            return false;
        }
        cur = p.parent
    }
    false
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let label = args.get(1).cloned().unwrap_or("sample".into());
    let secs = args.get(2).and_then(|v| v.parse().ok()).unwrap_or(30u64);
    let hz = unsafe { libc::sysconf(libc::_SC_CLK_TCK) } as f64;
    let mut previous = HashMap::new();
    let mut samples: HashMap<String, Vec<Value>> = HashMap::new();
    let mut last = Instant::now();
    for index in 0..=secs {
        let map = read();
        let roots: HashSet<_> = map
            .values()
            .filter(|p| {
                p.cmd
                    .split_whitespace()
                    .next()
                    .is_some_and(|s| s.ends_with("/t3code") && s.starts_with("/opt/t3code"))
            })
            .map(|p| p.pid)
            .collect();
        let backend: HashSet<_> = map
            .values()
            .filter(|p| p.cmd.contains("apps/server/dist/bin.mjs") && p.cmd.contains("t3code"))
            .map(|p| p.pid)
            .collect();
        let workers: HashSet<_> = map
            .values()
            .filter(|p| p.cmd.contains("omarchy-t3-client watch"))
            .map(|p| p.pid)
            .collect();
        let mut groups: HashMap<&str, (u64, u64, u64, Vec<u32>)> = HashMap::new();
        for p in map.values() {
            let group = if backend.contains(&p.pid) {
                "t3_backend"
            } else if under(p.pid, &backend, &map) {
                continue;
            } else if under(p.pid, &roots, &map) {
                "t3_gui"
            } else if under(p.pid, &workers, &map) {
                "omarchy_rust_and_tunnels"
            } else if p.cmd.split_whitespace().next().is_some_and(|s|s.ends_with("quickshell")) && p.cmd.contains("/usr/share/omarchy/shell")
            {
                "omarchy_whole_shell"
            } else {
                continue;
            };
            let memory =
                fs::read_to_string(format!("/proc/{}/smaps_rollup", p.pid)).unwrap_or_default();
            let kb = |key: &str| {
                memory
                    .lines()
                    .find(|l| l.starts_with(key))
                    .and_then(|l| l.split_whitespace().nth(1))
                    .and_then(|s| s.parse::<u64>().ok())
                    .unwrap_or(0)
            };
            let g = groups.entry(group).or_default();
            g.0 += kb("Pss:");
            g.1 += kb("Rss:");
            g.3.push(p.pid);
            if let Some((start, ticks)) = previous.get(&p.pid) {
                if *start == p.start {
                    g.2 += p.ticks.saturating_sub(*ticks)
                }
            }
        }
        let elapsed = last.elapsed().as_secs_f64();
        last = Instant::now();
        if index > 0 {
            for (name, (pss, rss, ticks, pids)) in groups {
                samples.entry(name.into()).or_default().push(json!({"pssMiB":pss as f64/1024.,"rssMiB":rss as f64/1024.,"cpuPercentOneCore":ticks as f64/hz/elapsed*100.,"pids":pids}));
            }
        }
        previous = map.values().map(|p| (p.pid, (p.start, p.ticks))).collect();
        if index < secs {
            std::thread::sleep(Duration::from_secs(1))
        }
    }
    let mut result = serde_json::Map::new();
    for (name, rows) in samples {
        let avg = |key: &str| {
            rows.iter().map(|r| r[key].as_f64().unwrap()).sum::<f64>() / rows.len() as f64
        };
        result.insert(name,json!({"samples":rows.len(),"meanPssMiB":avg("pssMiB"),"meanRssMiB":avg("rssMiB"),"meanCpuPercentOneCore":avg("cpuPercentOneCore"),"measurements":rows}));
    }
    println!(
        "{}",
        json!({"label":label,"secondsRequested":secs,"logicalCpus":std::thread::available_parallelism().map(|n|n.get()).unwrap_or(0),"groups":result})
    );
}
