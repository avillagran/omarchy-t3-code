// Installed in the trusted inspector's MainWorld at DocumentCreation, before
// assigning devToolsView. Never install this in an inspected application page.
(() => {
    "use strict";
    const host = globalThis.InspectorFrontendHost;
    const api = globalThis.DevToolsAPI;
    if (!host || !api || typeof host.sendMessageToBackend !== "function" ||
        typeof api.dispatchMessage !== "function") throw Error("private_cdp_unavailable");
    const minimalFrontend = globalThis.__o3PrivateCdpMinimal === true;
    const originalSend = host.sendMessageToBackend.bind(host);
    const originalDispatch = api.dispatchMessage.bind(api);
    // This is decoded debugger JSON, not the outer NDJSON envelope. Native
    // CdpRelay retains its separate 64 Mi-unit serialized output budget.
    const limit = 128 * 1024 * 1024;
    const pending = new Map();
    let next = 1, queued = 0, closed = false, failure = null;
    let chunks = "", expected = 0;
    const output = [];
    function close(reason) {
        if (closed) return;
        closed = true;
        failure = reason;
        pending.clear(); output.length = 0; queued = 0;
        chunks = ""; expected = 0;
    }
    function fail(reason) { close(reason); throw Error(reason); }
    function decode(raw) {
        try {
            const value = JSON.parse(raw);
            if (!value || typeof value !== "object" || Array.isArray(value))
                return fail("private_cdp_invalid_message");
            return value;
        } catch (error) {
            close("private_cdp_invalid_message");
            throw error;
        }
    }
    function enqueue(raw) {
        if (raw.length > limit || queued + raw.length > limit) return fail("private_cdp_queue_limit");
        output.push(raw); queued += raw.length;
    }
    function send(raw, owner) {
        if (closed) throw Error("private_cdp_closed");
        if (typeof raw !== "string" || raw.length > limit) return fail("private_cdp_message_limit");
        const value = decode(raw);
        if (!Number.isSafeInteger(value.id) || value.id < 0 || typeof value.method !== "string")
            return fail("private_cdp_invalid_request");
        if (pending.size >= 4096 || next > 2147483647) return fail("private_cdp_request_limit");
        const physical = next++;
        pending.set(physical, {owner, id:value.id});
        value.id = physical;
        try { originalSend(JSON.stringify(value)); }
        catch (error) { close("private_cdp_send_failed"); throw error; }
    }
    function receive(raw) {
        if (closed) return;
        if (typeof raw !== "string" || raw.length > limit) return fail("private_cdp_message_limit");
        const value = decode(raw);
        if (Object.prototype.hasOwnProperty.call(value, "id")) {
            const request = pending.get(value.id);
            if (!request) return fail("private_cdp_unknown_reply");
            pending.delete(value.id);
            value.id = request.id;
            const restored = JSON.stringify(value);
            if (request.owner === "frontend") { if (!minimalFrontend) originalDispatch(restored); }
            else enqueue(restored);
        } else {
            if (!minimalFrontend) originalDispatch(raw);
            enqueue(raw);
        }
    }
    // Automation owns this hidden frontend exclusively. Suppressed frontend
    // requests stay unanswered; do not fabricate protocol success. Opening real
    // DevTools requires retiring this lease and replacing the inspector.
    host.sendMessageToBackend = raw => { if (!minimalFrontend) send(raw, "frontend"); };
    api.dispatchMessage = receive;
    api.dispatchMessageChunk = (part, size) => {
        if (closed) return;
        if (typeof part !== "string" || !Number.isSafeInteger(size) || size < 0 || size > limit)
            return fail("private_cdp_chunk_limit");
        if (size) {
            if (expected) return fail("private_cdp_chunk_overlap");
            expected = size;
        }
        if (!expected || chunks.length + part.length > expected) return fail("private_cdp_chunk_length");
        chunks += part;
        if (chunks.length === expected) {
            const raw = chunks; chunks = ""; expected = 0;
            receive(raw);
        }
    };
    Object.defineProperty(globalThis, "__o3PrivateCdp", {value:Object.freeze({
        send:raw => send(raw, "server"),
        drain:() => { const messages = output.splice(0); queued = 0; return {messages, closed, failure}; },
        close:() => close("private_cdp_released")
    }), configurable:false, writable:false});
})();
