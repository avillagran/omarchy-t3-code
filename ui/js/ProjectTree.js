.pragma library
.import "SidebarModel.js" as Sidebar

function alphabetic(a,b) { return Sidebar.fold(a).localeCompare(Sidebar.fold(b)) }
function compareThreads(a,b) { return alphabetic(a.title,b.title) || String(a.key).localeCompare(String(b.key)) }
function state(t) {
    if (t.snoozed) return "snoozed"
    var s = Sidebar.state(t)
    if (s === "waiting" || s === "working" || s === "failed") return s
    if (t.settled) return "settled"
    return s === "done" ? "done" : "idle"
}
// grouped: threads sit under a header per state; otherwise straight under their project.
function build(groups, threads, collapsed, grouped) {
    if (grouped === undefined) grouped = true
    var projects = [], lookup = Object.create(null), members = Object.create(null)
    groups.forEach(function(g) { (g.members || []).forEach(function(p) { members[p.key] = g }) })
    threads.filter(function(t) { return t.relationshipToParent !== "subagent" }).forEach(function(t) {
        var g = members[t.projectKey], key = g ? g.key : t.projectKey || t.environmentId + "/" + (t.projectId || "scratch")
        if (!lookup[key]) { lookup[key] = {key:key, title:g ? g.title : t.project || t.environmentName || "", threads:[]}; projects.push(lookup[key]) }
        lookup[key].threads.push(t)
    })
    projects.sort(function(a,b) { return alphabetic(a.title,b.title) || String(a.key).localeCompare(String(b.key)) })
    var result = [], states = [{id:"waiting",label:"Waiting"},{id:"working",label:"Working"},{id:"failed",label:"Failed"},{id:"idle",label:"Pending"},{id:"done",label:"Done"},{id:"snoozed",label:"Snoozed"},{id:"settled",label:"Settled"}]
    projects.forEach(function(p) {
        var projectKey = "project/" + p.key
        result.push({kind:"project",key:projectKey,title:p.title,count:p.threads.length,projectKey:p.key,expanded:!collapsed[projectKey]})
        if (collapsed[projectKey]) return
        if (!grouped) {
            p.threads.slice().sort(compareThreads).forEach(function(t) { result.push({kind:"thread",key:t.key,thread:t,projectKey:p.key,state:state(t)}) })
            return
        }
        states.forEach(function(s) {
            var rows = p.threads.filter(function(t) { return state(t) === s.id }).sort(compareThreads)
            if (!rows.length) return
            var key = projectKey + "/state/" + s.id
            result.push({kind:"state",key:key,title:s.label,count:rows.length,projectKey:p.key,state:s.id,expanded:!collapsed[key]})
            if (!collapsed[key]) rows.forEach(function(t) { result.push({kind:"thread",key:t.key,thread:t,projectKey:p.key,state:s.id}) })
        })
    })
    return result
}
