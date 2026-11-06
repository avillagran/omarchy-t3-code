//! Presentation ported from ot3's QML and JavaScript: the conversation minimap,
//! subagent cards, theme colours and the string catalog.
use super::{domain_fn, DomainFn};
use crate::{i18n, subagents, theme};
use crate::timeline::minimap;
use serde_json::json;

pub const FUNCTIONS: &[DomainFn] = &[
    domain_fn! {
        name: "timeline.minimap", module: "timeline",
        args: [(rows, Array, required), (visibleRowIds, Array, optional), (atEnd, Bool, optional)],
        returns: "object {items [{id, rowIndex, userText, assistantText, userPreview, assistantPreview}], visible (two items or more), currentIndex, visibleIds}: one marker per user row of timeline.view's timeline, paired with the last assistant reply before the next user row; currentIndex and visibleIds follow the rows on screen",
        run: |a| Ok(minimap::view(a.req_array("rows")?, a.array("visibleRowIds")?.map(Vec::as_slice).unwrap_or(&[]), a.bool("atEnd")?.unwrap_or(false))),
    },
    domain_fn! {
        name: "timeline.minimapIndex", module: "timeline",
        args: [(y, Number, required), (height, Number, required), (count, Integer, required)],
        returns: "integer: the marker under y on a rail height tall with count markers, or -1",
        run: |a| Ok(json!(minimap::index_at(a.f64("y")?.unwrap_or(0.0), a.f64("height")?.unwrap_or(0.0), a.i64("count")?.unwrap_or(0)))),
    },
    domain_fn! {
        name: "subagents.buckets", module: "subagents",
        args: [(subagents, Array, required)],
        returns: "object {working, done, failed, stopped, idle}: status counts (queued, running and waiting are working)",
        run: |a| Ok(subagents::buckets(a.req_array("subagents")?)),
    },
    domain_fn! {
        name: "subagents.describe", module: "subagents",
        args: [(subagent, Object, required), (now, Integer, required)],
        returns: "object {live, settled, label, tone, detail, elapsed}: one subagent row; label is an English source string (Queued, Running, Waiting, Idle · resumable, Completed, Failed, Stopped or Unknown), tone the colour's state (working, waiting, done, failed or idle), elapsed like 4m 09s",
        run: |a| Ok(subagents::describe(a.req_object("subagent")?, a.i64("now")?.unwrap_or(0))),
    },
    domain_fn! {
        name: "subagents.cards", module: "subagents",
        args: [(cards, Array, required), (threadSubagents, Array, optional), (now, Integer, required)],
        returns: "object {subagents [{subagent, view}], group, anyLive, expanded, counts, header {source, args}, summary [{source, args}]}: a timeline subagents row's cards overlaid with threadState.subagents; header and summary parts are catalog sources ({n} subagents; {n} working, done, failed, stopped, idle) joined with \" · \"",
        run: |a| Ok(subagents::cards(a.req_array("cards")?, a.array("threadSubagents")?.map(Vec::as_slice).unwrap_or(&[]), a.i64("now")?.unwrap_or(0))),
    },
    domain_fn! {
        name: "subagents.fromThread", module: "subagents",
        args: [(thread, Object, required)],
        returns: "object {id thread:<key>, title, status, childThreadId, startedAt, updatedAt, providerInstanceId, model, fromThread: true}: a sidebar sub-thread seen as a subagent",
        run: |a| Ok(subagents::from_thread(a.req_object("thread")?)),
    },
    domain_fn! {
        name: "theme.tokens", module: "theme",
        args: [(colors, Text, required), (id, Text, optional)],
        returns: "object {id, name, appearance (light when the background's Rec.709 luminance > 0.5), luminance, mode, base, roles {dim, statusMuted, divider, surface, card, cardBorder, changesCard, rowSelected, rowPressed, rowDraft, field, fieldBorder, switchOff, inlineCode, composerBorder, composerFocus, working, done, waiting, failed, warning, input, onAccent}, layers {role: {color, alpha}}, statusColors {detail: colour}, projectPalette, projectFills, sky {lightSky, star, meteor, ink, header {brightness, nebula, meteorRate, base}, panel}, document {appearance, variables}, mcpTheme}: ot3's colours for an Omarchy colors.toml, each role the opaque #rrggbb ot3 draws",
        run: |a| Ok(theme::tokens(a.req("colors")?, a.str("id")?)),
    },
    domain_fn! {
        name: "i18n.t", module: "i18n",
        args: [(locale, Text, required), (key, Text, required), (args, Object, optional)],
        returns: "string: ot3's translation of an English source string for a locale list (fr_CA.UTF-8:en), else the language's, English's or the source; {name} placeholders from args, Arabic isolated with U+2068/U+2069",
        run: |a| Ok(json!(i18n::t(a.req("locale")?, a.req("key")?, a.get("args")))),
    },
    domain_fn! {
        name: "i18n.catalog", module: "i18n",
        args: [(locale, Text, required)],
        returns: "object {locale, language, rtl, strings {source: translation}}: every string of the catalog for the locale as i18n.t gives it without args (2027 sources: Desktop additions and retained compatible sources; 20 languages)",
        run: |a| Ok(i18n::strings(a.req("locale")?)),
    },
    domain_fn! {
        name: "i18n.resolve", module: "i18n",
        args: [(locale, Text, required)],
        returns: "object {locale, language, rtl}: the first locale of the list the catalog has (by itself or its language), else the first",
        run: |a| {
            let (locale, language, rtl) = i18n::resolve(a.req("locale")?);
            Ok(json!({"locale":locale,"language":language,"rtl":rtl}))
        },
    },
    domain_fn! {
        name: "i18n.languages", module: "i18n",
        args: [(locale, Text, optional)],
        returns: "array of {id, name}: ot3's language picker, system first (its name translated for locale), then each catalog language in its own name",
        run: |a| {
            let system = json!({"id":"system","name":i18n::t(a.str("locale")?.unwrap_or("en"), "System language", None)});
            Ok(json!(std::iter::once(system).chain(i18n::LANGUAGES.iter().map(|(id, name)| json!({"id":id,"name":name}))).collect::<Vec<_>>()))
        },
    },
];
