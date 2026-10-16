.pragma library
function theme(bg,fg,accent,colors) {
 var c=colors || {},dim=c.bright_black || c.brightBlack || fg;
 return {appearance:(bg.r*0.299+bg.g*0.587+bg.b*0.114)<0.5?"dark":"light",variables:{
 "--background":String(bg),"--foreground":String(fg),"--card":c.black || String(bg),"--card-foreground":String(fg),
 "--popover":String(bg),"--popover-foreground":String(fg),"--muted":c.black || String(bg),"--muted-foreground":String(dim),
 "--border":String(dim),"--input":String(dim),"--ring":String(accent),"--primary":String(accent),"--primary-foreground":String(bg),
 "--accent":String(accent),"--accent-foreground":String(bg),"--accent-surface":c.black || String(bg),"--accent-surface-foreground":String(accent),
 "--secondary":c.black || String(bg),"--secondary-foreground":String(fg),"--destructive":c.red || String(accent),"--destructive-foreground":String(fg),"--destructive-surface":c.black || String(bg),
 "--warning":c.yellow || String(accent),"--warning-foreground":c.yellow || String(accent),"--warning-surface":c.black || String(bg),
 "--success":c.green || String(accent),"--success-foreground":c.green || String(accent),"--info":c.blue || String(accent),"--info-foreground":c.blue || String(accent),
 "--code-background":c.black || String(bg),"--code-foreground":String(fg),"--chart-1":String(accent),"--chart-2":c.green || String(accent),"--chart-3":c.blue || String(accent),"--chart-4":c.magenta || String(accent),"--chart-5":c.cyan || String(accent),"--chart-6":c.yellow || String(accent),
 "--radius":"0.625rem","--font-sans":"sans-serif","--font-mono":"monospace"}}
}
function escapeAttribute(s){return String(s).replace(/&/g,"&amp;").replace(/"/g,"&quot;").replace(/</g,"&lt;")}
function page(data) {
 var theme=data.theme,css=":root{color-scheme:"+theme.appearance+";";
 Object.keys(theme.variables).forEach(function(k){css+=k+":"+theme.variables[k]+" !important;"});css+="}html,body{background:var(--background)!important;color:var(--foreground)!important;font-family:var(--font-sans)}a{color:var(--accent)}pre,code{color:var(--code-foreground);background:var(--code-background)}";
 var injected='<base href="'+escapeAttribute(data.baseUrl)+'"><style>'+css+'</style>';
 // Append at the end of head so report CSS retains layout while the host owns
 // theme tokens and the page canvas. The opaque sandbox cannot reach Qt/app storage.
 var probe='<script>addEventListener("load",function(){parent.postMessage({type:"t3-document-ready",background:getComputedStyle(document.body).backgroundColor,foreground:getComputedStyle(document.body).color,accent:getComputedStyle(document.documentElement).getPropertyValue("--accent"),sandboxed:(function(){try{void parent.document;return false}catch(e){return true}})()},"*")});<\/script>';
 var html=data.contents+probe;
 if(/<\/head\s*>/i.test(html))html=html.replace(/<\/head\s*>/i,injected+'</head>');else html=injected+html;
 return '<!doctype html><script>addEventListener("message",function(e){var f=document.querySelector("iframe");if(f&&e.source===f.contentWindow&&e.data&&e.data.type==="t3-document-ready")window.__t3DocumentStatus=e.data});<\/script><meta charset="utf-8"><style>html,body,iframe{border:0;margin:0;width:100%;height:100%;overflow:hidden;background:'+theme.variables["--background"]+'}</style><iframe sandbox="allow-scripts" referrerpolicy="no-referrer" srcdoc="'+escapeAttribute(html)+'"></iframe>';
}
