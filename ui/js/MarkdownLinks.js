.pragma library

// Native ChatMarkdown uses the GitHub brand for all github.com destinations,
// including tree/commit/pull links, and a file icon for workspace references.
// Keep decoration in the view so source Markdown, copy and search stay intact.
function glyph(href) {
  var value = String(href || "")
  if (!value || value[0] === "#" || /^ot3-code-/i.test(value)) return ""
  var web = /^https?:\/\/([^/?#]+)/i.exec(value)
  if (web) {
    var host = web[1].split("@").pop().split(":")[0].toLowerCase().replace(/\.$/, "")
    return host === "github.com" || /\.github\.com$/.test(host) ? "\uf09b" : "\uf0ac"
  }
  if (/^t3-thread:\/\//i.test(value)) return "\uf086"
  if (/^[a-z][a-z0-9+.-]*:/i.test(value) && !/^file:\/\//i.test(value)) return ""
  var path = value.split(/[?#]/)[0].toLowerCase()
  if (/\.(md|mdx|markdown)$/.test(path)) return "\uf48a"
  if (/\.rs$/.test(path)) return "\ue7a8"
  if (/\.(js|jsx|mjs|cjs|ts|tsx|json|qml|html|css|py|sh|toml|yaml|yml)$/.test(path)) return "\uf1c9"
  if (/\.(png|jpe?g|gif|webp|svg|ico)$/.test(path)) return "\uf1c5"
  if (/\.pdf$/.test(path)) return "\uf1c1"
  return /\/$/.test(path) ? "\uf07b" : "\uf15b"
}

function decorate(html) {
  return String(html).replace(/<a\b([^>]*\bhref="([^"]*)"[^>]*)>/g, function(tag, attributes, href) {
    var icon = glyph(href)
    // Nerd brand glyphs overhang their advance width. Keep an actual space in
    // the same font run as well as the nonbreaking separator before the label.
    return icon ? tag + '<span style="font-family: FiraCode Nerd Font;">' + icon + ' </span>&nbsp;' : tag
  })
}
