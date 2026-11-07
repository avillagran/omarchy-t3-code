// Qt platform boundary for the pinned native GuestInput consumer.
// Install only in the trusted automation inspector, never in application pages.
(function (scope) {
    "use strict";
    const arrows = { Up: "ArrowUp", Down: "ArrowDown", Left: "ArrowLeft", Right: "ArrowRight" };
    const modifierBits = { alt: 1, control: 2, meta: 4, shift: 8 };
    scope.T3QtGuestInput = Object.freeze({
        create: function (rawSend, snapshot, failed) {
            let alive = true;
            let pending = Promise.resolve();
            let failure;
            const check = () => {
                if (!alive) throw Error("private_browser_closed");
                if (failure) throw failure;
            };
            const enqueue = operation => {
                const result = pending.then(() => { check(); return operation(); });
                pending = result.catch(error => {
                    if (!failure) {
                        failure = error;
                        failed("private_browser_input_failed");
                    }
                });
                return result;
            };
            const webContents = {
                getURL: () => snapshot().url,
                getTitle: () => snapshot().title,
                getUserAgent: () => snapshot().userAgent,
                get focusedFrame() { return snapshot().activeFocus ? {} : undefined; },
                insertText: text => enqueue(() => rawSend("Input.insertText", { text })),
                sendInputEvent: event => {
                    const type = event.type;
                    const modifiers = (event.modifiers || []).reduce((bits, name) => bits | (modifierBits[name] || 0), 0);
                    const params = type === "char"
                        ? { type, text: event.keyCode, modifiers }
                        : { type, key: arrows[event.keyCode] || event.keyCode, modifiers };
                    // Native GuestInput's following Runtime.evaluate must await this actual ACK.
                    // Consume the fire-and-forget rejection here; the barrier retains the failure.
                    enqueue(() => rawSend("Input.dispatchKeyEvent", params)).catch(() => {});
                },
                reload: () => { enqueue(() => rawSend("Page.reload", {})).catch(() => {}); },
                reloadIgnoringCache: () => { enqueue(() => rawSend("Page.reload", { ignoreCache: true })).catch(() => {}); }
            };
            return Object.freeze({
                webContents,
                sendCommand: async (method, params, sessionId) => {
                    await pending;
                    check();
                    return rawSend(method, params, sessionId);
                },
                close: () => { alive = false; }
            });
        }
    });
})(globalThis);
