// Same inert metadata corpus in the original Node realm and the Qt adapter.
function runVoicePreferencesOracle(factory, flatMap) {
    var observations=[]
    function primitive(name, make, mapper) {
        var log=[],array=make(log),value=null,failure=null
        try {value=flatMap(array,function(item,index,receiver){log.push("map:"+index);return mapper(item,index,receiver,log)})}
        catch(error){failure=error.name}
        observations.push({name:name,value:value,error:failure,log:log})
    }
    primitive("sparse-depth",function(){var a=[];a.length=4;a[1]=2;a[3]=4;return a},function(value){var a=[];a.length=3;a[1]=value;a[2]=[value+1];return a})
    primitive("immediate-flatten",function(){return [1,2]},function(value,index,receiver,log){var a=[];Object.defineProperty(a,"0",{enumerable:true,get:function(){log.push("get:"+value);return value}});return a})
    primitive("getter-throws-before-next-map",function(){return [1,2]},function(value,index,receiver,log){var a=[];Object.defineProperty(a,"0",{get:function(){log.push("throw:"+value);throw new RangeError("inert")}});return a})
    primitive("length-and-delete",function(){return [1,2,3]},function(value,index,receiver){if(index===0){delete receiver[1];receiver.push(4)}return [value]})
    primitive("nonarray-result",function(){return [1]},function(){return {0:"not-flattened",length:1}})
    for(var seed=0;seed<80;seed++)(function(seed){
        primitive("generated:"+seed,function(){var a=[];a.length=seed%9;for(var i=0;i<a.length;i++)if((i+seed)%3)a[i]=(seed+i)%11;return a},function(value,index){if((index+seed)%4===0)return value;var a=[];a.length=3;if(value%2)a[0]=value;a[2]=[value,index];return a})
    })(seed)
    var cases=[
        {personal:[],project:[],hints:[]},
        {personal:[{term:"Alpha",aliases:["A","a"]}],project:[{term:"alpha",aliases:["Project"]}],hints:[" alpha ","  new\t hint  "]},
        {personal:[{term:"e\u0301",aliases:["cafe\u0301"]}],project:[{term:"\u00e9",aliases:["CAF\u00c9"]}],hints:["\u00c9","Fresh"]},
        {personal:[{term:"\u0130",aliases:["i\u0307"]},{term:"\u03a3",aliases:["\u03c3","\u03c2"]}],project:[],hints:["\u0130","\u03a3"]},
        {personal:[],project:[],hints:["x".repeat(50),"y".repeat(51),"\ud83d\ude00".repeat(25),"\ud83d\ude00".repeat(26)]}
    ]
    var many=[];for(var n=0;n<110;n++)many.push({term:"word"+n,aliases:["alias"+n]});cases.push({personal:many,project:[{term:"priority",aliases:[]}],hints:["hint"]})
    var state={input:{ownerKey:"owner",environmentId:"draft",projectName:"Project",cleanup:true},clientSettings:{speechLanguage:"auto",speechTranslateToEnglish:false,speechCustomWords:[],speechRemoveFillerWords:true,speechSilenceFiltering:true,speechCorrectionWord:"cue",speechPostProcessingPrompt:{mode:"default",customInstructions:""}},projectWords:[],cleanupPrepared:true}
    var ref={current:state},grant="cleanup",audio=true,requests=[]
    var preferences=factory({latestRef:ref,hasAudioConsent:function(){return audio},readCleanupGrant:function(){return grant},runEnvironmentSpeech:function(value){return Promise.resolve(value)},postProcessEnvironmentTranscript:function(target,transcript,draft,options,allowed){requests.push({target:target,transcript:transcript,draft:draft,options:options,allowed:allowed()});return {text:"clean"}}})
    cases.forEach(function(input,index){observations.push({name:"vocabulary:"+index,value:preferences.resolveSpeechVocabulary(input)})})
    var contexts=[{text:"abc",selection:{start:-3,end:9}},{text:"a".repeat(1500)+"SELECT"+"z".repeat(1500),selection:{start:1500,end:1506}},{text:"x".repeat(1023)+"\ud83d\ude00"+"y".repeat(1025),selection:{start:1024,end:1025}},{text:"abc",selection:{start:2,end:1}}]
    contexts.forEach(function(input,index){observations.push({name:"context:"+index,value:preferences.cleanupContext(input)})})
    state.clientSettings.speechCustomWords=cases[1].personal;state.projectWords=cases[1].project
    observations.push({name:"options",value:preferences.getTranscriptionOptions(),captured:preferences.getRecordingPreferences()})
    var options={draft:{ownerKey:"owner",text:"hello",selection:{start:5,end:5}}}
    return Promise.resolve(preferences.postProcess("spoken",options)).then(function(value){
        observations.push({name:"postprocess",value:value,requests:requests.slice()})
        grant="new-cleanup";observations.push({name:"changed-cleanup",value:preferences.postProcess("spoken",options)})
        grant="cleanup";audio=false;observations.push({name:"revoked-audio",value:preferences.postProcess("spoken",options)})
        audio=true;state.input.ownerKey="successor";observations.push({name:"changed-owner",value:preferences.postProcess("spoken",options)})
        state.input.ownerKey="owner";state.input.environmentId="elsewhere";observations.push({name:"changed-environment",value:preferences.postProcess("spoken",options)})
        state.input.environmentId="draft";state.cleanupPrepared=null
        return Promise.resolve(preferences.postProcess("spoken",options)).then(function(){throw new Error("Missing cleanup target unexpectedly accepted")},function(error){observations.push({name:"missing-target",error:error.message})})
    }).then(function(){return {count:observations.length,cases:observations}})
}
