#!/usr/bin/env python3
"""Private QML transport receipts; no server, credentials, shell or inference."""
import json,sys,os,time,datetime
from pathlib import Path
if not Path('/run/.containerenv').is_file():
    raise SystemExit('This fixture requires rootless Podman')
commands=[]
drafts={}
threads=[]
launches=0
clock_expiry=None
project={'id':'project','key':'env/project','environmentId':'env','title':'Private project','workspaceRoot':'/tmp/project'}
selection={'instanceId':'primary','model':'fixture','options':[]}
models=[{'instanceId':'primary','model':'fixture','available':True},{'instanceId':'backup','model':'other','available':True}]
def emit(kind,data):print(json.dumps({'type':kind,'data':data}),flush=True)
def snapshot():return {'projects':[project],'threads':threads,'environments':[{'id':'env','state':'ready'}],'working':0,'pending':0,'stale':False}
emit('ui-state',{})
emit('snapshot',snapshot())
for raw in sys.stdin:
    command=json.loads(raw);commands.append(command)
    Path('/evidence/commands.json').write_text(json.dumps(commands))
    op=command.get('op')
    if op=='draft-save':
        drafts[command['key']]=command['draft'];emit('draft-saved',{'key':command['key'],'revision':command['revision']})
    elif op=='draft-load':emit('draft-loaded',{'key':command['key'],'draft':drafts.get(command['key'],{})})
    elif op=='settings' and os.environ.get('O3_LOCAL_DRAFT_CLOCK')=='1':
        if clock_expiry is None:
            now=time.time();clock_expiry=(int(now)//60+1)*60
            if clock_expiry-now<3:clock_expiry+=60
        expiry=datetime.datetime.fromtimestamp(clock_expiry,datetime.timezone.utc).isoformat().replace('+00:00','Z')
        providers=[{'instanceId':'primary','driver':'codex','enabled':True,'installed':True,'status':'ready','availability':'available','auth':{'status':'authenticated'},'models':[{'slug':'fixture','isDefault':True,'isCustom':False}],'usageLimits':{'windows':[{'id':'session','kind':'session','usedPercent':100,'resetsAt':expiry}]}},{'instanceId':'backup','driver':'claudeAgent','enabled':True,'installed':True,'status':'ready','availability':'available','auth':{'status':'authenticated'},'models':[{'slug':'other','isDefault':True,'isCustom':False}],'usageLimits':{'windows':[]}}]
        settings={'providerInstances':{'primary':{'driver':'codex','enabled':True},'backup':{'driver':'claudeAgent','enabled':True}},'projectSettingsOverrides':{}}
        emit('action',{'op':op,'environment':command['environment'],'requestId':command['requestId'],'result':{'mode':'local','defaultModelSelection':selection,'defaultRuntimeMode':'approval-required','modelDefaults':{'providers':providers,'settings':settings,'fallbackSelection':{'instanceId':'backup','model':'other','options':[]}}}})
    elif op=='settings':
        emit('action',{'op':op,'environment':command['environment'],'requestId':command['requestId'],'result':{'mode':'local','defaultModelSelection':selection,'defaultRuntimeMode':'approval-required','modelDefaults':{'providers':[],'settings':{},'fallbackSelection':None}}})
    elif op=='catalog':emit('catalog',{'environment':command['environment'],'models':models,'providers':[],'canWrite':True})
    elif op=='manage':
        launches+=1
        if launches<=2:
            emit('action',{'op':'manage','environment':'env','requestId':command['requestId'],'error':'rpc_outcome_unknown' if launches==1 else 'rpc_rejected: Synthetic admission refused'})
            continue
        assert command['action']=='new-thread' and command['launchWithMessage'] is True
        assert command['workspaceStrategy']['type']=='root'
        thread=command.get('targetThreadId','omarchy-thread-'+command['requestId'])
        threads.append({'id':thread,'key':'env/'+thread,'environmentId':'env','projectId':'project','projectKey':'env/project','title':command['title'],'modelSelection':command['modelSelection'],'runtimeMode':'approval-required','interactionMode':'default'})
        emit('snapshot',snapshot());emit('action',{'op':'manage','environment':'env','requestId':command['requestId'],'result':{'action':'new-thread','threadId':thread}})
    elif op=='quit':break
