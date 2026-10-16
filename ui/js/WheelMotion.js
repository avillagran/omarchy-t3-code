.pragma library
// Control-panel's Glide preset. Same adaptive smoothstep/velocity EMA as the
// compositor patch; QML supplies a finite OutCubic coast when the patch is off.
var glide={speed:1.3,max:4.0,coasting:1000}
function step(state,angle,pixel,touchpad,now){
 var device=touchpad?"touchpad":"mouse",previous=state[device] || {},dt=now-(previous.time || 0)
 var raw=pixel!==0?pixel:angle/8,delta=pixel!==0?pixel:angle*1.8,direction=delta>0?1:delta<0?-1:0
 var continuous=dt>0&&dt<=100&&previous.direction===direction
 var velocity=continuous?(previous.velocity || 0)*0.7+Math.abs(raw)/dt*0.3:0
 var t=Math.max(0,Math.min(1,velocity/glide.speed)),factor=1+t*t*(3-2*t)*(glide.max-1)
 var next={mouse:state.mouse,touchpad:state.touchpad,device:device};next[device]={time:now,direction:direction,velocity:velocity}
 return {state:next,delta:delta*factor,continueTarget:continuous&&state.device===device,factor:factor}
}
