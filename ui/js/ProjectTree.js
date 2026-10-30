.pragma library
.import "SidebarModel.js" as Sidebar

function stamp(t) { return Number(t.repliedAt || t.updatedAt || t.startedAt || 0) }
function state(t) {
    if (t.snoozed) return "snoozed"
    var s = Sidebar.state(t)
    if (s === "waiting" || s === "working" || s === "failed") return s
    if (t.settled) return "settled"
    return s === "done" ? "done" : "idle"
}
function build(groups, threads, collapsed) {
    var projects = [], lookup = Object.create(null), members = Object.create(null)
    groups.forEach(function(g) { (g.members || []).forEach(function(p) { members[p.key] = g }) })
    threads.filter(function(t) { return t.relationshipToParent !== "subagent" }).forEach(function(t) {
        var g = members[t.projectKey], key = g ? g.key : t.projectKey || t.environmentId + "/" + (t.projectId || "scratch")
        if (!lookup[key]) { lookup[key] = {key:key, title:g ? g.title : t.project || t.environmentName || "", threads:[], latest:0}; projects.push(lookup[key]) }
        lookup[key].threads.push(t); lookup[key].latest = Math.max(lookup[key].latest, stamp(t))
    })
    projects.sort(function(a,b) { return b.latest - a.latest || a.title.localeCompare(b.title) })
    var result = [], states = [{id:"waiting",label:"Waiting"},{id:"working",label:"Working"},{id:"failed",label:"Failed"},{id:"idle",label:"Pending"},{id:"done",label:"Done"},{id:"snoozed",label:"Snoozed"},{id:"settled",label:"Settled"}]
    projects.forEach(function(p) {
        var projectKey = "project/" + p.key
        result.push({kind:"project",key:projectKey,title:p.title,count:p.threads.length,projectKey:p.key,expanded:!collapsed[projectKey]})
        if (collapsed[projectKey]) return
        states.forEach(function(s) {
            var rows = p.threads.filter(function(t) { return state(t) === s.id }).sort(function(a,b) { return stamp(b)-stamp(a) || String(a.key).localeCompare(String(b.key)) })
            if (!rows.length) return
            var key = projectKey + "/state/" + s.id
            result.push({kind:"state",key:key,title:s.label,count:rows.length,projectKey:p.key,state:s.id,expanded:!collapsed[key]})
            if (!collapsed[key]) rows.forEach(function(t) { result.push({kind:"thread",key:t.key,thread:t,projectKey:p.key,state:s.id}) })
        })
    })
    return result
}
