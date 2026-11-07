// Private Qt debugger scope. Native virtual-browser commands stay in CdpRelay.
// Qt's page debugger otherwise permits browser-wide Target commands.
(function (scope) {
    "use strict";
    scope.T3QtTargetScope = Object.freeze({
        create: function (targetId, rawSend, retired) {
            const targets = new Set([targetId]);
            const sessions = new Map();
            let alive = true;
            const denied = () => { throw Error("Only this preview tab's target and child sessions are available."); };
            const current = sessionId => alive && (sessionId === undefined || sessions.has(sessionId));
            const admit = (sessionId, target, parent) => {
                if (!current(parent) || typeof sessionId !== "string" || typeof target !== "string") return false;
                if (sessions.size >= 4096 || targets.size >= 4096) {
                    alive = false; sessions.clear(); targets.clear();
                    retired("private_browser_session_limit");
                    return false;
                }
                const existing = sessions.get(sessionId);
                if (existing && (existing.target !== target || existing.parent !== parent)) {
                    alive = false; sessions.clear(); targets.clear();
                    retired("private_browser_session_mismatch");
                    return false;
                }
                targets.add(target); sessions.set(sessionId, { target, parent });
                return true;
            };
            const remove = sessionId => {
                const pending = [sessionId];
                while (pending.length) {
                    const next = pending.pop();
                    for (const [id, entry] of sessions) if (entry.parent === next) pending.push(id);
                    const entry = sessions.get(next); sessions.delete(next);
                    if (entry && entry.target !== targetId && ![...sessions.values()].some(value => value.target === entry.target)) targets.delete(entry.target);
                }
            };
            return Object.freeze({
                sendCommand: async (method, params = {}, sessionId) => {
                    if (!current(sessionId)) return denied();
                    if (method.startsWith("Browser.")) return denied();
                    if (method.startsWith("Target.")) {
                        switch (method) {
                            case "Target.getTargetInfo":
                                if (params.targetId !== undefined && !targets.has(params.targetId)) return denied();
                                break;
                            case "Target.setAutoAttach":
                                break; // The physical debugger is attached only to this owned page/session.
                            case "Target.attachToTarget":
                            case "Target.activateTarget":
                            case "Target.closeTarget":
                                if (!targets.has(params.targetId)) return denied();
                                break;
                            case "Target.detachFromTarget":
                                if (!sessions.has(params.sessionId)) return denied();
                                break;
                            default:
                                return denied(); // Includes discovery, context creation and message tunneling.
                        }
                    }
                    const result = await rawSend(method, params, sessionId);
                    if (!current(sessionId)) return denied();
                    if (method === "Target.getTargetInfo" && !targets.has(result?.targetInfo?.targetId)) {
                        alive = false; sessions.clear(); targets.clear();
                        retired("private_browser_target_mismatch");
                        return denied();
                    }
                    if (method === "Target.attachToTarget" && !admit(result.sessionId, params.targetId, sessionId)) return denied();
                    if (method === "Target.detachFromTarget") remove(params.sessionId);
                    return result;
                },
                event: (method, params = {}, sessionId) => {
                    if (!current(sessionId)) return false;
                    if (method === "Target.attachedToTarget") {
                        const info = params.targetInfo;
                        // Only flattened children emitted from an admitted physical parent establish ownership.
                        if (!info || !["iframe", "worker"].includes(info.type)) return false;
                        return admit(params.sessionId, info.targetId, sessionId);
                    }
                    if (method === "Target.detachedFromTarget") {
                        if (!sessions.has(params.sessionId)) return false;
                        remove(params.sessionId); return true;
                    }
                    if (method.startsWith("Target.")) {
                        const target = params.targetInfo?.targetId ?? params.targetId;
                        return targets.has(target);
                    }
                    return true;
                },
                close: () => { alive = false; sessions.clear(); targets.clear(); }
            });
        }
    });
})(globalThis);
