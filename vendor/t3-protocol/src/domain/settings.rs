//! Settings: the server's preferences as fields and patches, per environment and
//! per project.
use super::{domain_fn, DomainFn};
use crate::{native_settings, scoped_settings};
use serde_json::{json, Value};

/// An empty array when the member is absent.
fn rows(v: Option<&Value>) -> &[Value] {
    v.and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])
}

pub const FUNCTIONS: &[DomainFn] = &[
    domain_fn! {
        name: "nativeSettings.snapshot", module: "nativeSettings",
        args: [(settings, Object, required)],
        returns: "object {fields [{key, label, section, kind: bool, choice, days, text or model, value, resetValue?, choices?, description?, …}]}: server.getSettings as the settings pages' fields; never credentials or provider configuration",
        run: |a| Ok(native_settings::snapshot(a.req_object("settings")?)),
    },
    domain_fn! {
        name: "nativeSettings.patch", module: "nativeSettings",
        args: [(input, Object, required), (config, Object, optional)],
        returns: "object {patch} for server.updateSettings from {key, value} (value null clears a nullable model); with server.getConfig as config, models and storage settings are checked against the server",
        run: |a| native_settings::patch(a.req_object("input")?, a.get("config")),
    },
    domain_fn! {
        name: "nativeSettings.description", module: "nativeSettings",
        args: [(key, Text, required)],
        returns: "string: a field's English description (empty when it has none)",
        run: |a| Ok(json!(native_settings::description(a.req("key")?))),
    },
    domain_fn! {
        name: "nativeSettings.modelSelection", module: "nativeSettings",
        args: [(selection, Object, required)],
        returns: "object {instanceId, model, options? [{id, value}]}: a checked model selection",
        run: |a| native_settings::model_selection(a.req_object("selection")?),
    },
    domain_fn! {
        name: "nativeSettings.idleCompaction", module: "nativeSettings",
        args: [(settings, Object, required), (instanceId, Text, optional)],
        returns: "string: the idle compaction mode (onReturn, beforeCacheExpires or off), the provider instance's override first when instanceId is given; null when the server has none",
        run: |a| {
            let settings = a.req_object("settings")?;
            Ok(json!(match a.str("instanceId")? {
                Some(instance) => native_settings::resolve_idle_compaction(settings, instance),
                None => native_settings::idle_compaction(settings),
            }))
        },
    },
    domain_fn! {
        name: "nativeSettings.idleCompactionPatch", module: "nativeSettings",
        args: [(settings, Object, required), (patch, Object, required)],
        returns: "object {patch}: an idleCompaction patch in the form this server reads (compactIdleThreads on older servers)",
        run: |a| native_settings::idle_compaction_patch(a.req_object("settings")?, a.req_object("patch")?),
    },
    domain_fn! {
        name: "nativeSettings.threadDefaults", module: "nativeSettings",
        args: [(settings, Object, required), (projectId, Id, required)],
        returns: "object {defaultModelSelection, modelDefaultConfigured, defaultRuntimeMode}: a new thread's defaults, the project's over the environment's",
        run: |a| Ok(native_settings::thread_defaults(a.req_object("settings")?, a.req("projectId")?)),
    },
    domain_fn! {
        name: "scopedSettings.effective", module: "scopedSettings",
        args: [(settings, Object, required), (project, Object, optional), (file, Object, optional)],
        returns: "object {settings, sources {key: environment, project or t3.json}, overrides}: a project's effective settings over the environment's and its t3.json",
        run: |a| {
            let (settings, sources, overrides) = scoped_settings::effective(a.req_object("settings")?, a.get("project"), a.get("file"));
            Ok(json!({"settings":settings,"sources":sources,"overrides":overrides}))
        },
    },
    domain_fn! {
        name: "scopedSettings.projectPatch", module: "scopedSettings",
        args: [(settings, Object, required), (project, Object, required), (key, Text, required), (patch, Object, required), (clear, Bool, optional)],
        returns: "object {patch {projectSettingsOverrides {projectId: entry}}}: a project override that keeps its other keys (setting_environment_only for environment keys)",
        run: |a| scoped_settings::project_patch(a.req_object("settings")?, a.req_object("project")?, a.req("key")?, a.req_object("patch")?, a.bool("clear")?.unwrap_or(false)),
    },
    domain_fn! {
        name: "scopedSettings.projectFile", module: "scopedSettings",
        args: [(contents, Text, required)],
        returns: "object: a project's t3.json (comments and trailing commas allowed), or null when it is missing or invalid",
        run: |a| Ok(scoped_settings::project_file(a.req("contents")?).unwrap_or(Value::Null)),
    },
    domain_fn! {
        name: "scopedSettings.validatedTargets", module: "scopedSettings",
        args: [(scope, Object, required)],
        returns: "array of {environment, projectId}: the scope's targets, 1 to 64, all project-scoped or all not (settings_scope_unavailable otherwise)",
        run: |a| Ok(json!(scoped_settings::validated_targets(&json!({"scope":a.req_object("scope")?}))?)),
    },
    domain_fn! {
        name: "scopedSettings.writePatch", module: "scopedSettings",
        args: [
            (input, Object, required), (config, Object, required), (settings, Object, required), (project, Object, optional),
            (writeAllowed, Bool, required),
        ],
        returns: "object {patch} for one target's server.updateSettings from {key, action: update or clear, value}: project overrides for a project, environment patches otherwise",
        run: |a| scoped_settings::write_patch(a.req_object("input")?, a.req_object("config")?, a.req_object("settings")?, a.get("project"), a.bool("writeAllowed")?.unwrap_or(false)),
    },
    domain_fn! {
        name: "scopedSettings.writeWasDropped", module: "scopedSettings",
        args: [(next, Object, required), (before, Object, required), (saved, Object, required)],
        returns: "bool: the server accepted the patch but the read-back still has the old value",
        run: |a| Ok(json!(scoped_settings::write_was_dropped(a.req_object("next")?, a.req_object("before")?, a.req_object("saved")?))),
    },
    domain_fn! {
        name: "scopedSettings.targetView", module: "scopedSettings",
        args: [
            (target, Object, required), (config, Object, required), (settings, Object, required), (project, Object, optional),
            (file, Object, optional), (writeAllowed, Bool, required),
        ],
        returns: "object {target, fields (each with source, scopeWritable, overridden, projectScoped), writeAllowed, storageCapabilities, catalog}: one target's page",
        run: |a| Ok(scoped_settings::target_view(a.req_object("target")?, a.req_object("config")?, a.req_object("settings")?, a.get("project"), a.get("file"), a.bool("writeAllowed")?.unwrap_or(false))),
    },
    domain_fn! {
        name: "scopedSettings.aggregate", module: "scopedSettings",
        args: [(views, Array, required)],
        returns: "object {fields}: the fields every target has, with mixed, source (mixed when they differ), overridden and scopeWritable over all of them",
        run: |a| Ok(scoped_settings::aggregate(a.req_array("views")?)),
    },
    domain_fn! {
        name: "scopedSettings.page", module: "scopedSettings",
        args: [(scope, Object, required), (views, Array, required), (errors, Array, optional), (saved, Array, optional), (requested, Integer, required)],
        returns: "object {fields, storageScopes, scope, modelCatalog, targets, errors, savedTargets, complete, writable}: the settings page over every target",
        run: |a| Ok(scoped_settings::page(a.req_object("scope")?, a.req_array("views")?, rows(a.get("errors")), rows(a.get("saved")), a.req_u64("requested")? as usize)),
    },
    domain_fn! {
        name: "scopedSettings.projectCatalogConfig", module: "scopedSettings",
        args: [(config, Object, required), (settings, Object, required), (project, Object, optional)],
        returns: "object: server.getConfig with the project's provider enable overrides applied (for models.catalog)",
        run: |a| Ok(scoped_settings::project_catalog_config(a.req_object("config")?, a.req_object("settings")?, a.get("project"))),
    },
];
