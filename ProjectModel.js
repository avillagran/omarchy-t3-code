.pragma library
function path(value){var p=String(value || "").replace(/\\/g,"/");return p.length>1?p.replace(/\/+$/,""):p}
function physical(p){return p.environmentId+":"+path(p.workspaceRoot)}
function mode(p,state){var native=state.nativeProjectGrouping || {},global=state.sidebarProjectGroupingMode || native.mode || "repository",own=(state.sidebarProjectGroupingOverrides || {})[physical(p)];return own?(own==="inherit"?global:own):(native.overrides || {})[physical(p)] || global}
function groupKey(p,state){
 var m=mode(p,state),r=p.repositoryIdentity || {},key=(r.origin || {}).canonicalKey || r.canonicalKey
 if(m==="separate"||!key)return p.key
 var base=path(r.rootPath),cwd=path(p.workspaceRoot)
 return "repository/"+key+(m==="repository_path"&&base&&cwd.startsWith(base+"/")?"::"+cwd.slice(base.length+1):"")
}
function groups(projects,state){var result=[],map={};(projects || []).forEach(function(p){var key=groupKey(p,state),g=map[key];if(!g){g={key:key,title:p.title,members:[]};map[key]=g;result.push(g)}g.members.push(p)});return result}
function scope(filter,groups){var g=groups.find(function(p){return p.key===filter||p.members.some(function(m){return m.key===filter})});return g?g.members.map(function(p){return p.key}):filter}
