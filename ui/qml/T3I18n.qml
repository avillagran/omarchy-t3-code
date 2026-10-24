pragma Singleton
import QtQuick 6.11
import Quickshell
import Quickshell.Io
Singleton{
 id:root
 property var catalog:({})
 property string systemLocale:""
 readonly property var languages:[{id:"system",name:tr("System language")},{id:"en",name:"English"},{id:"es",name:"Español"},{id:"pt",name:"Português"},{id:"fr",name:"Français"},{id:"de",name:"Deutsch"},{id:"it",name:"Italiano"},{id:"nl",name:"Nederlands"},{id:"pl",name:"Polski"},{id:"ru",name:"Русский"},{id:"uk",name:"Українська"},{id:"ja",name:"日本語"},{id:"ko",name:"한국어"},{id:"zh",name:"中文"},{id:"ar",name:"العربية"},{id:"tr",name:"Türkçe"},{id:"sv",name:"Svenska"},{id:"da",name:"Dansk"},{id:"no",name:"Norsk"},{id:"fi",name:"Suomi"},{id:"cs",name:"Čeština"}]
 function normalize(locale){return String(locale || "").split(":")[0].split(/[.@]/)[0].replace(/_/g,"-").toLowerCase()}
 readonly property string choice:T3Backend.viewState.language || "system"
 readonly property string rawLocale:String(choice!=="system"?choice:Quickshell.env("OMARCHY_T3_LANGUAGE") || Quickshell.env("LANGUAGE") || Quickshell.env("LC_ALL") || Quickshell.env("LC_MESSAGES") || systemLocale || Quickshell.env("LANG") || "en")
 function base(locale){var b=locale.split("-")[0];return b==="nb"||b==="nn"?"no":b}
 readonly property var candidates:rawLocale.split(":").map(function(l){return root.normalize(l)})
 readonly property string locale:candidates.find(function(l){return !!root.catalog[l] || !!root.catalog[root.base(l)]}) || candidates[0] || "en"
 readonly property string language:base(locale)
 readonly property bool rtl:language==="ar"
 function tr(source,args){
  var normalized=locale;var base=language;var value=(catalog[normalized] || {})[source] || (catalog[base] || {})[source] || (catalog.en || {})[source] || source
  if(root.rtl)value=String(value).replace(/(@files\/folders|\$skills|\/commands|Ctrl\+Enter)/g,"\u2068$1\u2069")
  return String(value).replace(/\{([A-Za-z_][A-Za-z0-9_]*)\}/g,function(full,key){if(!args || args[key]===undefined)return full;var a=String(args[key]);return root.rtl?"\u2068"+a+"\u2069":a})
 }
 // A missing T3 scope comes first, "[settings:write] message"; show it once, after the message.
 function scoped(reason){var s=/^\[([a-z-]+:[a-z-]+)\](?: ([\s\S]*))?$/.exec(reason);if(!s)return reason;var text=s[2]||"";return !text?s[1]:text.indexOf(s[1])>=0?text:text+" ("+s[1]+")"}
 // Codes may carry the server's reason after ": " (e.g. "rpc_rejected: Project not found").
 function error(value){if(!value)return "";var m=/^([a-z][a-z0-9_]+): ([\s\S]+)$/.exec(String(value));if(m)return m[1]==="rpc_rejected"?tr("T3 rejected the request: {reason}",{reason:m[2]}):m[1]==="write_not_authorized"?tr("T3 did not allow this: {reason}",{reason:scoped(m[2])}):tr("T3 request failed ({code}): {reason}",{code:m[1],reason:scoped(m[2])});if(!/^[a-z][a-z0-9_]+$/.test(value))return tr(value);return tr("T3 request failed ({code})",{code:value})}
 property FileView translations:FileView{
  path:Qt.resolvedUrl("../../translations/i18n.json").toString().replace(/^file:\/\//,"");watchChanges:true;printErrors:false
  onLoaded:{try{var next=JSON.parse(text());if(next.en)root.catalog=next}catch(e){console.warn("Invalid T3 translation catalog")}}
  onFileChanged:reload()
 }
 property FileView system:FileView{
  path:"/etc/locale.conf";watchChanges:true;printErrors:false
  onLoaded:{var match=text().match(/^\s*LANG=["']?([^\s"']+)/m);root.systemLocale=match?match[1]:""}
  onFileChanged:reload()
 }
}
