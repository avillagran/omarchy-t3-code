.pragma library

// Qt rich text has its own browser-like defaults. Set the supported subset of
// native ChatMarkdown's CSS explicitly; source HTML has already been escaped.
function render(input, fontSize, scale, foreground, muted, surface, border) {
  var html=String(input), block=/^p$|^h[1-6]$|^ul$|^ol$|^blockquote$|^pre$|^table$/
  var tags=/<\/?([a-z][a-z0-9]*)\b[^>]*>/gi, depth=0, roots=[],match
  while((match=tags.exec(html))!==null){
    var name=match[1].toLowerCase(),closing=match[0][1]==="/"
    if(closing){depth=Math.max(0,depth-1);continue}
    if(depth===0&&block.test(name))roots.push({start:match.index,end:tags.lastIndex})
    if(!/^(br|hr|img|input|meta|link|wbr)$/.test(name)&&!/\/>$/.test(match[0]))depth++
  }
  var edits={}
  if(roots.length){edits[roots[0].end]="margin-top:0px;";var last=roots[roots.length-1];edits[last.end]=(edits[last.end] || "")+"margin-bottom:0px;"}
  Object.keys(edits).map(Number).sort(function(a,b){return b-a}).forEach(function(end){html=html.slice(0,end-1)+' style="'+edits[end]+'"'+html.slice(end-1)})
  html=html.replace(/<table\b([^>]*)>/g,function(tag,attributes){return /\bwidth\s*=/i.test(attributes)?tag:'<table width="100%"'+attributes+'>'})
  // Code toolbar tables keep their compact cells instead of inheriting prose
  // table padding. Their links are generated internally, never from raw HTML.
  html=html.replace(/<table\b[^>]*>[\s\S]*?<\/table>/g,function(table){return table.indexOf('href="ot3-code-copy://')>=0?table.replace("<table ",'<table class="ot3-code-toolbar" '):table})
  // Qt applies proportional paragraph leading to the entire inline image.
  // Keep prose leading elsewhere; image lines need their intrinsic height.
  ;["p","li"].forEach(function(tag){
    html=html.replace(new RegExp("<"+tag+"\\b([^>]*)>([\\s\\S]*?)</"+tag+">","g"),function(full,attributes,body){
      if(body.indexOf("<img ")<0)return full;
      if(/\bstyle="/.test(attributes))attributes=attributes.replace(/style="/, 'style="line-height:100%;');
      else attributes+=' style="line-height:100%;"';
      return "<"+tag+attributes+">"+body+"</"+tag+">";
    });
  })
  var px=function(n){return Math.round(n*scale*100)/100+"px"}
  return '<style>'+
    'p,ul,ol,blockquote,pre,table{margin-top:'+px(10.4)+';margin-bottom:'+px(10.4)+';}' +
    'p,li{line-height:150%;}li{margin-top:'+px(4)+';}' +
    'ul,ol{margin-left:'+px(20)+';padding-left:0;}' +
    'h1,h2,h3,h4,h5,h6{margin-top:'+px(20)+';margin-bottom:'+px(8)+';font-weight:600;line-height:130%;color:'+foreground+';}' +
    'h1{font-size:'+px(20)+';}h2{font-size:'+px(18)+';}h3{font-size:'+px(16)+';}h4,h5,h6{font-size:'+fontSize+'px;}' +
    'blockquote{margin-left:'+px(12)+';color:'+muted+';}' +
    'code{font-family:monospace;font-size:'+Math.round(fontSize*12/14)+'px;background-color:'+surface+';}' +
    'pre{background-color:'+surface+';white-space:pre-wrap;}' +
    'table{border-collapse:collapse;font-size:'+Math.round(fontSize*12/14)+'px;}' +
    'th,td{padding:'+px(7)+' '+px(12)+';border-bottom:1px solid '+border+';}' +
    '.ot3-code-toolbar{margin-bottom:0;} .ot3-code-toolbar td{padding:'+px(2)+';border:0;}' +
    'a{text-decoration:none;}' +
    '</style>'+html
}
