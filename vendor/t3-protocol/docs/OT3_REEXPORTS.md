# ot3 re-exports for the portable domain modules

These modules moved from ot3 (`omarchy-t3-code` `main` at `191c1fc`, v0.2.22)
into `t3-protocol`. Function names, signatures and output bytes are unchanged, so
ot3's callers, QML and tests stay as they are when ot3 adds the re-exports
below. Point ot3's `t3-protocol` dependency (and `vendor/t3-protocol`) at a
revision that contains this change first.

## 1. Crate root (`src/lib.rs`)

Replace the helper bodies at `lib.rs:105-143` with:

```rust
pub use t3_protocol::util::{epoch, text, truth, valid_id};
```

`pub type Result<T>` stays in ot3. It is the same type as `t3_protocol::Result`.
`MAX_BYTES`, `rendered_markdown`, `math_enabled` and `markdown_render` stay in ot3.

## 2. Modules that moved whole

Delete these files from `src/` and replace their `pub mod` lines in `lib.rs`:

```rust
pub use t3_protocol::checkpoint_revert;
pub use t3_protocol::latex;
pub use t3_protocol::message_text;
pub use t3_protocol::plan_workflow;
pub use t3_protocol::prompt_recall;
pub use t3_protocol::storage_settings;
```

Nothing of these modules stays in ot3. `latex` also gains `Surface` and
`render(tex, display, Surface::Qt)`, which return exactly what `to_rich_text` returns.

## 3. Modules split between the lib and ot3

Keep `pub mod <module>;` in `lib.rs`. In each `src/<module>.rs`, delete the
moved items and the in-file tests that moved with them, keep the functions
listed under "Stays in ot3", and add the `pub use` line. Items marked * were
private in ot3. They are public in the lib because ot3's remaining host code
calls them. Then remove the imports the remaining code no longer uses
(`sha2`, `HashSet`, `BTreeMap`, `chrono` items, `protocol`, `std::io::Write`).

This was checked by compiling every split file of ot3 `main`, after these edits,
against this branch. The edited files used exactly the `pub use` lines below,
with stubs for `Environment`, `Client` and `write::rpc`.

### `diff_review`
```rust
pub use t3_protocol::diff_review::{present, scoped_result};
```
Stays in ot3: `execute`.

### `tool_inspector`
```rust
pub use t3_protocol::tool_inspector::{compact_reference, parse, request, scope, METHOD, RAW_BYTES, SECTION_BYTES};
```
Stays in ot3: `execute`.

### `quota_sources`
```rust
pub use t3_protocol::quota_sources::{apply, limits, providers, sources};
```
Stays in ot3: `connection`, `authorize`, `subscribe`.

### `git_controls`
```rust
pub use t3_protocol::git_controls::{actions, build_action, build_verified_action, present, resolve_scripts, scope, scoped_result, validate_status, MAX_FILES, MAX_SCRIPTS};
```
Stays in ot3: `execute` and the module's private `type Result<T>` alias, which `execute` uses.

### `provider_instances`
```rust
pub use t3_protocol::provider_instances::{confirm_target, mutation, projection};
```
Stays in ot3: `execute`. `data/provider-drivers.json` is now only read by the lib,
so ot3 can delete it.

### `scoped_settings`
```rust
pub use t3_protocol::scoped_settings::{aggregate, base_key, effective, project_catalog_config, project_file, project_patch, validated_targets, write_was_dropped, PROJECT_KEYS};
```
`base_key`, `validated_targets`, `write_was_dropped` and `project_catalog_config` are marked *.
Stays in ot3: `execute`, `execute_in`. Delete the in-file `dropped_tests`.

### `native_settings`
```rust
pub use t3_protocol::native_settings::{description, idle_compaction, idle_compaction_patch, model_selection, patch, resolve_idle_compaction, snapshot, thread_defaults, MODEL_KEYS};
```
`MODEL_KEYS` is marked *.
Stays in ot3: `execute`.

### `branch_picker`
```rust
pub use t3_protocol::branch_picker::{dedupe, derive_local, mismatch, sanitize, selection_target, temporary_branch};
```
Stays in ot3: `thread_scope` (private), `refs`, `switch`, `check`, `resolve`. Delete
the in-file `tests` and `mismatch_tests`.

### `pull_request_actions` → merged into `t3_protocol::pull_requests`
```rust
pub use t3_protocol::pull_requests::{payload, thread_command};
```
Stays in ot3: `run`, `thread_link`, `open_thread`. Delete `ACTIONS`, `reference`,
`body` and the in-file `tests` and `link_tests`. Existing `protocol::pull_requests::request`
and `sidebar_badge` callers are unchanged.

### `thread_details`
```rust
pub use t3_protocol::thread_details::{editors, workspace};
```
Stays in ot3: `shell` (private), `read`, `open`. Delete the in-file `tests`.
`src/native_editors.json` stays because `keybinding_settings` still includes it.

### `failure_context`
```rust
pub use t3_protocol::failure_context::{candidates, detail, input_summary, present};
```
`present` is marked *.
Stays in ot3: `read_item` (private), `run`, `tool`. Delete the in-file `tests`.

### `usage_summary`
```rust
pub use t3_protocol::usage_summary::{aggregate, request, window};
```
Stays in ot3: `time_zone` (reads `TZ` and `/etc/localtime`) and `read`. `request(period,
today, time_zone)` is new and optional: it builds the same `server.getUsageSummary`
payload that `read` writes inline today. Delete the in-file `tests`.

### `server_updates`
```rust
pub use t3_protocol::server_updates::{advisory, channel, compare, due, exact, newest, quiet, CLAIM_MS, QUIET_MS, RETRY_MS};
```
Stays in ot3: `RELEASE_INDEX`, `REMOTE_UPDATE`, `latest` (GitHub), `route` and `run`
(processes, `t3_home`).

### `reconnect`
```rust
pub use t3_protocol::reconnect::{transient, Backoff, Waiter, BASE_DELAY, MAX_DELAY, PROBE_TIMEOUT, STABLE_AFTER};
```
Stays in ot3: `reachable`, `wake`, `Signal`, `after`, `parse_signal`, `monitor`,
`clock_gap_watch`, `watch_system`. Delete the `WAKE`/`WAKE_SIGNAL` statics and
`wake_count`, and make `wake` forward to the lib's counter so `Waiter::default()` sees it:

```rust
pub fn wake(reason: &str) {
    t3_protocol::reconnect::notify();
    eprintln!("ot3: reconnecting streams after {reason}");
}
```

Keep the in-file `gdbus_monitor_lines` test. The other reconnect tests moved to the lib
(`waiter_sees_each_wakeup_once` calls `notify()` there).

## 4. ot3 tests

The pure tests now run in the lib (`tests/checkpoint_revert.rs`, `plan_workflow.rs`,
`prompt_recall.rs`, `storage_settings.rs`, `diff_review.rs`, `tool_inspector.rs`,
`quota_sources.rs`, `git_controls.rs`, `provider_instances.rs`, `scoped_settings.rs`,
`native_settings.rs`, `server_updates.rs` and the in-file unit tests). ot3 keeps its
copies unchanged, except:

- `tests/checkpoint_revert.rs` includes `#[path = "../src/checkpoint_revert.rs"]`,
  which no longer exists. Use `use omarchy_t3_client::checkpoint_revert;`.
- `tests/tool_inspector.rs` and `tests/git_controls.rs` include the source file with
  `#[path]` and stub the crate root. They still compile because the remaining file
  only adds a `pub use`. Switching to `use omarchy_t3_client::{tool_inspector, git_controls};`
  is simpler.
- The fake-server tests (`execute`, `subscribe`, `execute_in`, `normalize_messages`,
  `rendered_markdown`) stay in ot3 only. They cover the host half.

## 5. Not in this batch

`bounded_snapshot` is in batch 2. Everything that needs `Environment`, `Client`,
`write::rpc`, `std::fs`, processes or other global state is in batches 4 and 5.
`markdown_render` and the rest of `lib.rs:144-825` are batch 5.

## 6. Sidebar, timeline and Markdown facts (0.3.3)

These parts of ot3 `main` at `7b5b1de` now live in the lib with the same output:
a temporary harness compiled ot3's own functions next to the lib and compared
them byte for byte (rows and enrichment on the shell vectors, the timeline on
ot3's test projections and 600 random variants, both math settings).

In `src/lib.rs`, the pure halves become re-exports or thin wrappers:

```rust
pub use t3_protocol::markdown::{file_references, inline_code_path, search_text as markdown_search_text, tables as markdown_tables};
pub use t3_protocol::projects::monogram as project_identity;
pub use t3_protocol::sidebar::view::{is_settled, rows as normalize};
pub use t3_protocol::timeline::{active_run_id, context_window, live_activity, subagent_summary, subagent_title, thread_workflows};

pub fn conversation_timeline(projection: &Value, messages: &[Value]) -> Vec<Value> {
    t3_protocol::timeline::conversation(projection, messages, &rendered_markdown)
}
pub fn normalize_messages(d: &Value) -> Result<Value> {
    let render = |text: &str, blocks: &[Value], math: bool| markdown_render(text, Some(blocks), math);
    let math = math_enabled();
    t3_protocol::timeline::view(&d["projection"], d, &t3_protocol::timeline::Render { math, message: Some(&render), plan: &rendered_markdown, blocks: false })
}
```

`markdown_images::descriptors` is `t3_protocol::markdown::images`. The enrichment
loop of `snapshot` (`lib.rs:379-404`) is `t3_protocol::sidebar::view::environment(&d,
&e.id, &e.name, now)`, which also returns the environment's project rows.
`normalize_messages_cached` and its `RenderCache` stay in ot3 (they only avoid
re-rendering unchanged messages), as do `markdown_render`, `rendered_markdown`,
`math_enabled`, `native_project_grouping` and the rest of `snapshot`. ot3's
`SidebarModel.js`, `ProjectModel.js` and `T3Palette.projectColor` have Rust
equivalents in `sidebar::view` and `projects::identities` for hosts without QML;
ot3 can keep its JavaScript.

## 7. Settings executor, minimap, subagents, theme and strings (0.3.4)

Checked against ot3 `main` at `7b5b1de` with temporary harnesses outside both
repos: ot3's JavaScript run in node, ot3's QML run under quickshell with Omarchy's
`Color.qml` (pixels read back from Qt), ot3's Rust pasted next to the lib, all
compared with the lib through `{"op":"domain"}`.

The pure halves of `scoped_settings::execute_in` (`src/scoped_settings.rs`) are now
`t3_protocol::scoped_settings::{write_patch, target_view, page}`: ot3 keeps the
reads (session, `server.getConfig`, `server.getSettings`, the shell, `t3.json`)
and the write, and calls them in the same order:

```rust
// For each target, after the permission check and the reads:
if action != "read" {
    let patch = write_patch(input, config, &raw, project.as_ref(), write_allowed)?;
    // update_settings, re-read, write_was_dropped, as today
}
views.push(target_view(target, config, &raw, project.as_ref(), file.as_ref(), write_allowed));
// After the loop:
let result = page(&input["scope"], &views, &errors, &saved, requested.len());
```

The rest are ports for hosts without QML; ot3 keeps its JavaScript and QML:
`timeline::minimap` (`ui/js/TimelineModel.js` and the marker rules of
`T3TimelineMinimap.qml`), `subagents` (`ui/js/Subagents.js` and the header of
`T3SubagentCards.qml`), `theme` (`T3Palette.qml`, the layers of `T3Panel.qml`,
`T3ThreadRow.qml`, `T3Approvals.qml`, `T3Changes.qml`, `T3TextField.qml`,
`T3Switch.qml` and `T3Composer.qml`, `DocumentTheme.js` and `T3Starfield.qml`)
and `i18n::t` (`T3I18n.tr`). `data/i18n.json` is a copy of ot3's
`translations/i18n.json` plus the eight strings `awareness.view` adds (0.3.5,
section 8); when ot3's catalog changes, copy it again and keep those eight
unless ot3 has them by then.

## 8. Thread awareness (0.3.5)

`t3_protocol::awareness` is new, not a move: it joins ot3's alert key with T3's
awareness rules for hosts that notify (ot3-Android's follow service first).
`awareness::phase` is T3's `projectThreadAwarenessV2`
(`packages/shared/src/agentAwareness.ts`, T3 nightly `680152e27`), checked with a
temporary node harness outside both repos against T3's own TypeScript on ot3's
`docs/protocol/fixtures/shell.json`, the lib's sidebar fixtures and a cross
product of every field it reads: 116,470 threads, no difference.

The alert decision is ot3's `notify` (`src/main.rs:54-144`) with T3's groups,
2-minute freshness for results, subagents excluded and the thread on screen
consumed, over a `baseline` the host stores instead of `seen-rust.json`. ot3
can keep `notify` as it is; to share the rule, call `awareness::view` with the
snapshot's environments and post `alerts` (and `summaries`) instead of diffing
`seen-rust.json`, keeping its sound choice (a question outranks a completion)
and its retry on a failed delivery (save the returned baseline only after
`notify-send` succeeds).
