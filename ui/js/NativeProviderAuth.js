// Original nightly 4340 account selection uses the actual returned methods.
// Removal eligibility belongs only to the provider controller's canRemove flag.
var changeAccount = "chatgpt-change-account";
function profileMethods(state) {
    var methods = state && state.methods;
    if (!Array.isArray(methods) || methods.length > 32) return [];
    var profiles = [];
    methods.forEach(function(method) {
        if (!method || typeof method.id !== "string" || typeof method.name !== "string") return;
        var hasRemove = Object.prototype.hasOwnProperty.call(method, "canRemove");
        var id = method.id.trim(), name = method.name.trim();
        if (id.length > 128 || id.indexOf("chatgpt-profile:") !== 0 || !name
            || !(method.description === null || typeof method.description === "string")
            || ["agent", "terminal", "credentials"].indexOf(method.type) < 0
            || hasRemove && typeof method.canRemove !== "boolean"
            || !(method.accountEmail === undefined || typeof method.accountEmail === "string" && method.accountEmail.trim())) return;
        // Original Schema.Struct drops unknown wire keys. Never copy a remote
        // __proto__ or constructor into the validated method projection.
        var profile = {id: id, name: name, type: method.type, description: method.description};
        if (hasRemove) profile.canRemove = method.canRemove;
        if (typeof method.accountEmail === "string") profile.accountEmail = method.accountEmail.trim();
        profiles.push(profile);
    });
    return profiles;
}
function selectedId(selection, profiles) {
    if (selection === changeAccount) return selection;
    var selected = profiles.find(function(profile) { return profile.id === selection; });
    return selected ? selected.id : profiles.length ? profiles[0].id : changeAccount;
}
function removable(selection, profiles) {
    var selected = profiles.find(function(profile) { return profile.id === selection; });
    return !!selected && Object.prototype.hasOwnProperty.call(selected, "canRemove") && selected.canRemove === true;
}
function busy(setup) {
    var phase = setup && setup.state && setup.state.phase;
    return !!(setup && setup.busy)
        || ["starting", "waiting", "verifying", "downloading", "extracting"].indexOf(phase) >= 0;
}
