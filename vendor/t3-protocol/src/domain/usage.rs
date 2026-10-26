//! Subscription usage: the usage page's request and totals, and the quota strip.
use super::{domain_fn, DomainFn};
use crate::api::{ids, Args};
use crate::{quota_sources, usage_summary, Result};
use chrono::NaiveDate;
use serde_json::{json, Value};

pub(super) const PERIODS: &[&str] = &["day", "week", "month", "quarter", "year"];

fn day(a: &Args, name: &str) -> Result<NaiveDate> {
    NaiveDate::parse_from_str(a.req(name)?, "%Y-%m-%d").map_err(|_| ids::invalid(name))
}
fn ymd(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

pub const FUNCTIONS: &[DomainFn] = &[
    domain_fn! {
        name: "usageSummary.window", module: "usageSummary",
        args: [(period, Enum(PERIODS), required), (today, Text, required)],
        returns: "object {sinceDay, untilDay}: the period's days ending `today` (YYYY-MM-DD, the device's local date)",
        run: |a| {
            let (since, until) = usage_summary::window(a.req("period")?, day(a, "today")?)?;
            Ok(json!({"sinceDay":ymd(since),"untilDay":ymd(until)}))
        },
    },
    domain_fn! {
        name: "usageSummary.request", module: "usageSummary",
        args: [(period, Enum(PERIODS), required), (today, Text, required), (timeZone, Text, required)],
        returns: "object {sinceDay, untilDay, payload}: payload is server.getUsageSummary's arguments, days bucketed in the IANA `timeZone`",
        run: |a| {
            let (since, until, payload) = usage_summary::request(a.req("period")?, day(a, "today")?, a.req("timeZone")?)?;
            Ok(json!({"sinceDay":ymd(since),"untilDay":ymd(until),"payload":payload}))
        },
    },
    domain_fn! {
        name: "usageSummary.aggregate", module: "usageSummary",
        args: [(summary, Object, required), (sinceDay, Text, required), (untilDay, Text, required)],
        returns: "object {sinceDay, untilDay, cost, tokens, sessions, cacheSavings, unpricedRecords, kinds {input, cached, cacheWrite, output, reasoning}, days [{day, cost, tokens}], models [{provider, model, cost, tokens, records}] (top 12), providers [{provider, cost, tokens}], sources [{provider, status, message, refreshing}], pricing, readAt}",
        run: |a| Ok(usage_summary::aggregate(a.req_object("summary")?, day(a, "sinceDay")?, day(a, "untilDay")?)),
    },
    domain_fn! {
        name: "quotaSources.limits", module: "quotaSources",
        args: [(usageLimits, Object, required)],
        returns: "object {checkedAt, windows [{id, label, kind: session, weekly, monthly or other, usedPercent, resetsAt?, windowDurationMins?}], credentialFingerprint?, unavailable? {reason}, resetCredits? {availableCount, nextExpiresAt?}, externalUsage? {label, url}}, or null when unreadable",
        run: |a| Ok(quota_sources::limits(a.req_object("usageLimits")?).unwrap_or(Value::Null)),
    },
    domain_fn! {
        name: "quotaSources.providers", module: "quotaSources",
        args: [(providers, Array, required)],
        returns: "array of {instanceId, driver, enabled, installed, availability, auth {status, email?, workspaceId?, label?}, displayName?, accentColor?, usageLimits?}: public provider statuses, never their configuration",
        run: |a| {
            a.req_array("providers")?;
            Ok(json!(quota_sources::providers(&a.all()["providers"])))
        },
    },
    domain_fn! {
        name: "quotaSources.sources", module: "quotaSources",
        args: [(sources, Array, required)],
        returns: "array of {id, label, accounts [{id, driver, usageLimits, email?, workspaceId?, plan?}], error?}: usage-limit sources with readable accounts",
        run: |a| {
            a.req_array("sources")?;
            Ok(json!(quota_sources::sources(&a.all()["sources"])))
        },
    },
    domain_fn! {
        name: "quotaSources.apply", module: "quotaSources",
        args: [(state, Object, optional), (event, Object, required)],
        returns: "object {state {providers, usageLimitSources, connected}, changed}: one server.subscribeServerConfig event (version 1) applied to the quota strip's state",
        run: |a| {
            let mut state = a.get("state").cloned().unwrap_or_else(|| json!({}));
            let changed = quota_sources::apply(&mut state, a.req_object("event")?);
            Ok(json!({"state":state,"changed":changed}))
        },
    },
];
