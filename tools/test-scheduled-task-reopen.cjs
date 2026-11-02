const fs=require('fs'),vm=require('vm'),assert=require('assert');
const qml=fs.readFileSync(require('path').join(__dirname,'../ui/qml/T3ScheduledTasks.qml'),'utf8');
const sent=[]; const c={Date,Math,root:null,editing:null,pendingCreate:null,signatureSecret:{text:'',clear(){this.text=''}},title:{text:'Hook'},prompt:{text:'x'},baseRef:{text:''},worktreePath:{text:''},minutes:{text:'60'},time:{text:'09:00'},signatureHeader:{text:'x-signature'},signaturePrefix:{text:'sha256='},projects:[],open(){},bridge:{selectedEnvironment:'e',selectedInfo:{projectId:'p'},scheduledTasksAction(v){sent.push(v)}},view:{busy:false,error:'scheduled_task_outcome_unknown'}};c.root=c;vm.createContext(c);
for(const n of ['show','edit','webhookSchedule','save'])vm.runInContext(qml.split('\n').find(l=>l.trim().startsWith(`function ${n}(`)).trim(),c);
vm.runInContext('edit(null); save(); signatureSecret.clear(); show(); edit(null); save()',c);
const writes=sent.filter(s=>s.action==='upsert');assert.equal(writes[0].commandId,writes[1].commandId); console.log('REOPEN_IDENTITY_PASS');

assert.equal(c.pendingCreate.title, 'Hook');
c.bridge.selectedEnvironment='another'; const before=sent.length; vm.runInContext('save()',c); assert.equal(sent.length,before);
console.log('PENDING_ENVIRONMENT_BARRIER_PASS');
