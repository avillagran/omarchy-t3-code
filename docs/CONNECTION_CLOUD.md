# Native T3 Connect account adapter

## Scope and ownership

New `src/connection_cloud.rs` and `ui/qml/T3ConnectionCloud.qml` provide local native CLI account authorization, browser account creation, saved-state inspection, managed/tunnel-free link requests, unlink and logout. They do not start/stop/restart services, alter exposure, replace the native environment catalog, read token stores, implement OAuth, or persist credentials. Only native T3 writes its own authorization and link state.

The component is local-only. Reject `environment != "local"` in the Rust dispatcher, not just QML. For remote environments keep the existing native HTTP access/settings component. Do not execute the local CLI using a remote environment's identity.

**Important native distinction:** CLI OAuth authorization is not the Electron Clerk session. This does not sign the Electron GUI in/out, replace `desktop_cloud.rs`'s discovery authentication, or supply its client token. Do not claim that a CLI sign-in enables the existing Electron-backed remote-environment catalog. Account creation is offered by the native device-authorization page, not a fabricated `register` CLI command or an embedded password form.

## Application integration

`main.rs` keeps one asynchronous manager for the IPC worker lifetime and forwards allowlisted, request-correlated events. It rejects remote identities, handles cancel/install approval outside the message mutation queue, and cancels adapter-owned CLI work at EOF/shutdown. The CLI executable and native base directory are resolved by the backend.

`T3Backend.qml` retains request state when preferences close, ignores late replies, clears device prompts at completion, and refreshes access state after account/link mutations. The registered `T3ConnectionCloud.qml` component sits in Connections; all visible strings use the shared catalog. `T3Connections.qml` offers a separate confirmed restart for a saved pending link when ot3 owns the local native service. Link configuration does not itself restart it.

### Rust public API

- `NativeCli { executable: PathBuf, base_dir: PathBuf, timeout: Duration }`, `new(...)` (15-minute default), `arguments(action)` (validated exact native argv).
- `CloudManager::start(cli, request_id, action, confirmed, emit) -> Result<(), String>`.
- `CloudManager::cancel(request_id)`, `confirm_install(request_id, approved)`, `cancel_active()`.
- `parse_status(raw) -> Result<Value, String>` and `device_prompt(raw) -> Option<Value>` are pure allowlist parsers.

Actions: `read`, `login`, `register`, `link`, `link-publish-only`, `unlink`, `logout`. `register` maps to native `login --headless`, with account creation performed in the browser's native authorization UI. Unlink/logout require `confirmed == true`; QML also confirms exposure/link changes. `link-publish-only` requests no managed tunnel and, per native CLI behavior, enables activity publishing.

### QML bridge contract

The component requires:

- `property var connectionCloud`, initially `{environment:"", requestId:"", busy:false, stage:"", status:null, device:null, error:"", statusError:"", warning:"", step:""}`.
- `requestConnectionCloud(environment, action, values)` where `values` includes explicit confirmation for unlink/logout/link confirmation.
- `cancelConnectionCloud(requestId)`.
- `confirmConnectionCloudInstall(requestId, approved)`.
- Existing `copyText(text)`; browser launch uses `Qt.openUrlExternally` only after explicit user click.

On starting a request set `environment`, `requestId`, `action`, `busy:true`, clear `device/error/statusError/warning/step`, preserve last status **only for the same environment**. Do not start a second job while busy. The adapter enforces one job even if QML is bypassed. Do not clear a running job simply because the preferences page closes: retain it centrally so the user can reopen/cancel it. Public URL/code should never go into persistent UI state, logs or notifications.

Every callback has:

```json
{"type":"connection-cloud","environment":"local","requestId":"...","action":"login","busy":true,"stage":"authorization","data":{"verificationUrl":"https://…","userCode":"…","expiresInMinutes":10}}
```

Progress stages: `starting`, `authorization`, `install-confirmation`, `installing`, `warning`, `refreshing`. Merge as follows:

- `authorization`: `device = event.data`.
- `installing`: `step = event.data.step || ""`.
- `warning`: `warning = event.data.code`, preserving it through the terminal event. The warning is important: native disconnect may return exit zero despite incomplete relay revocation/tunnel shutdown.
- All ordinary progress: update `stage`; preserve prior status and public prompt unless explicitly cleared.

Terminal event has `busy:false`, stage `finished|failed|cancelled`, `error`, `statusError`, `status`, and `device:null`. Copy all terminal fields, clear the public device prompt/step, preserve a previously reported warning. Non-null `status` replaces prior status. If terminal status is null (cancel/read failure), invalidate the old saved state or clearly mark it stale; never imply that a partly completed mutation was rolled back. Refresh before retrying.

`error` is the mutation/authorization result. `statusError` is separate readback failure: a successful mutation must not be retried just because refresh failed. After every non-cancelled mutation, including a failed mutation that may have partially applied, the adapter runs native `status --json` separately. It never retries the mutation.

Status is allowlisted. Older installed CLI versions omit `revoked`; this is returned as null (unknown), while a present value must be boolean:

```json
{"savedSetupOnly":true,"authenticated":true,"desired":true,"linked":false,"revoked":false,"cloudUserId":null,"publishAgentActivity":false,"relayClient":{"status":"available"}}
```

There is no email in native `status --json`: `cloudUserId` describes the provisioned link, not necessarily a current account identity. `authenticated` means a native stored credential exists; it is not a live OAuth/relay check. `desired:true, linked:false` means pending native startup, **not Connected**. Existing `/api/connect/link-state` remains the live-settings source. No JWTs, refresh/device tokens, secret-store contents, arbitrary stdout/stderr, CLI identity scraping, or relay URLs are sent over this bridge.

## Native source evidence

Inspected `/home/avillagran/Work/t3code-nightly`:

- `apps/server/src/cli/connect.ts:75-90`: exact public device authorization prompt. No device JSON output flag exists.
- `connect.ts:94-129`: reuse/refresh stored native credentials, then native device flow; `tokens.store` owns persistence.
- `connect.ts:140-150,546-597`: `status --json` schema; saved setup, not live status (`208`).
- `connect.ts:247-264,466-497`: native relay-client acquisition/download confirmation; managed versus publish-only link setup and pending native startup.
- `connect.ts:499-544,669-685`: exact `connect login --headless`, `link --headless [--publish-only]`, `unlink`, `logout` commands.
- `connect.ts:286-308,331-416`: native live `/api/connect/unlink`, relay-side DELETE, native local cleanup, partial-disconnect warnings, and service ownership retained separately.
- `apps/server/src/cli/config.ts:42-47`: `--base-dir` shares native T3 data, equivalent to `T3CODE_HOME`.
- `apps/server/src/cloud/CliTokenManager.ts:125-157,390-418`: native persisted token schema and RFC8628 device authorization/expiration. No reimplementation here.
- `apps/web/src/cloud/useCloudLinkController.ts:121-228`: native scopes, serialized mode reconciliation and refresh after partial success; separate managed-tunnel/activity states.
- `apps/web/src/cloud/linkEnvironmentAtoms.ts`: native commands target a particular primary environment and use Clerk tokens; CLI OAuth must not be substituted as a Clerk client token.
- `apps/web/src/components/settings/ConnectionsSettings.tsx:1826-2006`: managed tunnel and independent activity/webhook preferences. Existing `T3ConnectionAccess.qml` retains these HTTP controls.

## Verification and limits

`cargo test --test connection_cloud` compiles the new module directly using `#[path]`, so it requires no edits to shared `src/lib.rs`. Fixtures are executable synthetic CLI programs under an isolated temporary base directory. They check exact argv, request scoping, strict/secret-free status, public-prompt parsing and unsafe URL rejection, asynchronous progress, native terminal installation approval/decline, busy guard, cancellation/descendant termination, bounded timeout/output, partial disconnect warnings, mutation readback and no mutation retries.

An isolated offscreen Quickshell fixture exercised the actual new component with a fake bridge: account-creation dispatch, public URL/code display, busy guard, copy, request-scoped cancellation, relay install approval, pending-startup wording, warning display, explicit logout confirmation and remote-environment guard. Fixture and screenshot: `/home/avillagran/.hermes/cache/scratch/ot3-cloud-qml-libgs85y/{shell.qml,cloud.png}`. The screenshot was visually inspected: URL/code, explanatory text and Cancel visible without clipping. No live account login, link, unlink, logout, installation, restart, service changes or publication were performed. Production bridge integration is covered by the repository QML fixtures. Real-account authorization requires the user to complete the native browser flow; it was not performed during testing.
