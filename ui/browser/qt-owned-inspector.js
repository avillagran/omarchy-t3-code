// One actual inspector/view lifetime. Only the trusted QML host calls this API.
(function (scope) {
    "use strict";
    const maxBytes = 256 * 1024 * 1024, maxRecords = 4096;
    const byteLength = text => {
        let size = 0;
        for (let i = 0; i < text.length; i++) {
            const code = text.charCodeAt(i);
            if (code < 128) size++;
            else if (code < 2048) size += 2;
            else if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length && text.charCodeAt(i + 1) >= 0xdc00 && text.charCodeAt(i + 1) <= 0xdfff) { size += 4; i++; }
            else size += 3;
        }
        return size;
    };
    scope.T3QtOwnedInspector = Object.freeze({
        create: function (identity) {
            if (!identity || ["threadId", "tabId", "instance", "target"].some(name => typeof identity[name] !== "string" || !identity[name])) throw Error("private_browser_identity_invalid");
            const key = Object.freeze({ threadId: identity.threadId, tabId: identity.tabId });
            const instance = identity.instance, target = identity.target;
            const requests = new Map(), listeners = new Set(), navigation = new Set();
            let sequence = 0, frames = [], queuedBytes = 0, closed = false;
            let guard, platform, host, lease, creating = false, waiting = [];
            let snapshot = { url: "about:blank", title: "", userAgent: String(identity.userAgent ?? ""), activeFocus: false, shown: false };
            const packet = (kind, fields) => ({ kind, ...key, instance, ...fields });
            const post = (kind, fields = {}) => {
                if (closed && kind !== "retired") return;
                const frame = JSON.stringify(packet(kind, fields));
                const bytes = byteLength(frame) + 1;
                if (frames.length >= maxRecords || bytes > maxBytes - queuedBytes) { retire("private_browser_output_limit"); return; }
                frames.push(frame); queuedBytes += bytes;
            };
            const retire = code => {
                if (closed) return;
                closed = true; frames = []; queuedBytes = 0; waiting = [];
                for (const request of requests.values()) request.reject(Error("private_browser_closed"));
                requests.clear(); listeners.clear(); navigation.clear();
                guard?.close(); platform?.close();
                scope.__o3PrivateCdp.close();
                host?.close().catch(() => {});
                post("retired", { code, ...(lease === undefined ? {} : { lease }) });
            };
            const raw = (method, params = {}, sessionId) => new Promise((resolve, reject) => {
                if (closed || requests.size >= maxRecords || sequence >= Number.MAX_SAFE_INTEGER) {
                    reject(Error("private_browser_request_limit")); retire("private_browser_request_limit"); return;
                }
                const id = ++sequence;
                requests.set(id, { resolve, reject });
                try { scope.__o3PrivateCdp.send(JSON.stringify({ id, method, params, ...(sessionId === undefined ? {} : { sessionId }) })); }
                catch (_) { retire("private_browser_transport_failed"); }
            });
            const matches = frame => frame && frame.instance === instance && frame.threadId === key.threadId && frame.tabId === key.tabId;
            const command = frame => {
                if (!host) {
                    if (waiting.length >= maxRecords) { retire("private_browser_request_limit"); return; }
                    waiting.push(frame); return;
                }
                const value = frame.command;
                if (!value || value.threadId !== key.threadId || (value.type !== "turn" && value.tabId !== key.tabId)) { retire("private_browser_scope_mismatch"); return; }
                host.command(JSON.stringify(value)).catch(() => retire("private_browser_runtime_failed"));
            };
            post("expected");
            (async () => {
                const info = await raw("Target.getTargetInfo");
                if (closed) return;
                if (typeof target !== "string" || !target || info.targetInfo?.targetId !== target) throw Error("private_target_mismatch");
                guard = scope.T3QtTargetScope.create(target, raw, retire);
                platform = scope.T3QtGuestInput.create(guard.sendCommand, () => snapshot, retire);
                post("proof", { viewTarget: target, cdpTarget: info.targetInfo.targetId });
            })().catch(() => retire("private_browser_probe_failed"));
            return Object.freeze({
                accept: frame => {
                    if (closed || !matches(frame)) return false;
                    if (frame.kind === "retire" && frame.lease === lease) { retire("private_browser_released"); return true; }
                    if (frame.kind === "admitted") {
                        if (!platform || typeof frame.lease !== "string" || !frame.lease || lease !== undefined) { retire("private_browser_scope_mismatch"); return false; }
                        lease = frame.lease; creating = true;
                        scope.T3NativeDesktopHost.create().then(async value => {
                            if (closed) { value.close().catch(() => {}); return; }
                            host = value; creating = false;
                            const webContents = Object.assign(platform.webContents, {
                                on: (name, fn) => { if (name === "did-navigate") navigation.add(fn); },
                                off: (name, fn) => navigation.delete(fn)
                            });
                            host.setShown(key, snapshot.shown);
                            await host.command(JSON.stringify({type:"turn",threadId:key.threadId,active:frame.agentTurn===true}));
                            if(closed)return;
                            host.attach(key, { webContents, debugger: {
                                sendCommand: platform.sendCommand,
                                on: (name, fn) => { if (name === "message") listeners.add(fn); },
                                off: (name, fn) => listeners.delete(fn)
                            } });
                            const queued = waiting; waiting = []; for (const entry of queued) command(entry);
                        }).catch(() => retire("private_browser_runtime_failed"));
                        return true;
                    }
                    if (frame.lease !== lease || lease === undefined) return false;
                    if (frame.kind === "command" && (host || creating)) { command(frame); return true; }
                    return false;
                },
                snapshot: value => {
                    if (closed) return;
                    const shown = value.shown === true;
                    const previous = snapshot.shown;
                    snapshot = { url: String(value.url), title: String(value.title), userAgent: String(value.userAgent ?? snapshot.userAgent), activeFocus: value.activeFocus === true, shown };
                    if (host && previous !== shown) host.setShown(key, shown);
                },
                drain: () => {
                    if (!closed) {
                        const data = scope.__o3PrivateCdp.drain();
                        if (data.closed || data.failure) retire("private_browser_transport_failed");
                        for (const text of data.messages) {
                            if (closed) break;
                            let value; try { value = JSON.parse(text); } catch (_) { retire("private_browser_transport_failed"); break; }
                            if (value.id !== undefined) {
                                const request = requests.get(value.id);
                                if (!request) { retire("private_browser_reply_mismatch"); break; }
                                requests.delete(value.id);
                                if (value.error) request.reject(Error(value.error.message)); else request.resolve(value.result);
                            } else if (!guard || guard.event(value.method, value.params, value.sessionId)) {
                                if (value.method === "Page.frameNavigated" && !value.params?.frame?.parentId) for (const fn of navigation) fn();
                                for (const fn of listeners) fn(null, value.method, value.params, value.sessionId);
                            }
                        }
                        if (host && !closed) {
                            const output = host.take();
                            if (output.closed || output.failure) retire("private_browser_runtime_failed");
                            else {
                                for (const line of output.events) post("event", { lease, event: JSON.parse(line) });
                                for (const event of output.painting) post("painting", { lease, active: event.active });
                                for (const event of output.pointers) post("pointer", { lease, event });
                            }
                        }
                    }
                    const output = frames; frames = []; queuedBytes = 0; return output;
                },
                close: () => retire("private_browser_released")
            });
        }
    });
})(globalThis);
