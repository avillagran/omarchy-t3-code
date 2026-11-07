// Installed in the trusted inspector's MainWorld at DocumentCreation, before
// assigning devToolsView. Never install this in an inspected application page.
(() => {
    "use strict";
    const host = globalThis.InspectorFrontendHost;
    const api = globalThis.DevToolsAPI;
    if (!host || !api || typeof host.sendMessageToBackend !== "function" ||
        typeof api.dispatchMessage !== "function") throw Error("private_cdp_unavailable");
    const originalSend = host.sendMessageToBackend.bind(host);
    const originalDispatch = api.dispatchMessage.bind(api);
    const limit = 16 * 1024 * 1024;
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
    function enqueue(raw) {
        if (raw.length > limit || queued + raw.length > limit) return fail("private_cdp_queue_limit");
        output.push(raw); queued += raw.length;
    }
    function send(raw, owner) {
        if (closed) throw Error("private_cdp_closed");
        if (typeof raw !== "string" || raw.length > limit) return fail("private_cdp_message_limit");
        const value = JSON.parse(raw);
        if (!value || !Number.isSafeInteger(value.id) || value.id < 0 || typeof value.method !== "string")
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
        const value = JSON.parse(raw);
        if (Object.prototype.hasOwnProperty.call(value, "id")) {
            const request = pending.get(value.id);
            if (!request) return fail("private_cdp_unknown_reply");
            pending.delete(value.id);
            value.id = request.id;
            const restored = JSON.stringify(value);
            if (request.owner === "frontend") originalDispatch(restored);
            else enqueue(restored);
        } else {
            originalDispatch(raw);
            enqueue(raw);
        }
    }
    host.sendMessageToBackend = raw => send(raw, "frontend");
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
