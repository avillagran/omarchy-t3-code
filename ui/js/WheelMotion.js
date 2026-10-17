.pragma library
// Control-panel's Glide preset. Same adaptive smoothstep/velocity EMA as the
// compositor patch. QML eases angle-only events; native pixel streams pass through.
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

function duration(distance){return Math.max(80,Math.min(glide.coasting,Math.abs(distance)*5))}

// Qt may expose a mouse coast as angle-only wheel events. Ease those events
// without accelerating again or adding travel to the compositor's distance.
function plan(state, angle, pixel, touchpad, native, now) {
  var motion=step(state,angle,pixel,touchpad,now)
  var sample=motion.state[touchpad?"touchpad":"mouse"],coast=0
  if(!native&&sample.velocity>=0.15)coast=sample.direction*Math.min(8,sample.velocity)*(pixel!==0?1:14.4)*glide.coasting/3
  return {coast:coast,state:motion.state, delta:native?(pixel!==0?pixel:angle*1.8):motion.delta,
          animate:!native||pixel===0, continueTarget:motion.continueTarget}
}
