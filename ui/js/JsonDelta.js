.pragma library
// Applies the structural patches of src/json_delta.rs. Unchanged subtrees keep
// the identity of the previous value, so row models and print caches can skip
// them; changed objects are new copies (values are never mutated in place).

function ident(raw) { return typeof raw === "string" ? "s" + raw : typeof raw === "number" ? "n" + raw : null }
function has(o, k) { return Object.prototype.hasOwnProperty.call(o, k) }

function apply(prev, patch) {
  if (patch === null || typeof patch !== "object" || Array.isArray(patch)) throw new Error("delta_bad_patch")
  if (has(patch, "$")) return patch.$
  if (typeof patch.k === "string") {
    if (!Array.isArray(prev) || !Array.isArray(patch.a)) throw new Error("delta_base_not_array")
    var field = patch.k, by = Object.create(null)
    for (var i = 0; i < prev.length; i++) {
      var e = prev[i], id = e !== null && typeof e === "object" ? ident(e[field]) : null
      if (id !== null) by[id] = e
    }
    return patch.a.map(function(entry) {
      if (entry !== null && typeof entry === "object") {
        if (has(entry, "$")) return entry.$
        var base = by[ident(entry.i)]
        if (base === undefined) throw new Error("delta_missing_element")
        return apply(base, entry.p)
      }
      var found = by[ident(entry)]
      if (found === undefined) throw new Error("delta_missing_element")
      return found
    })
  }
  if (prev === null || typeof prev !== "object" || Array.isArray(prev)) throw new Error("delta_base_not_object")
  var out = Object.assign({}, prev), set = patch.o || {}
  Object.keys(set).forEach(function(k) { out[k] = apply(has(prev, k) ? prev[k] : null, set[k]) })
  ;(patch.r || []).forEach(function(k) { delete out[k] })
  return out
}

// Resolves one stream frame against the held bases. Returns the full value, or
// undefined when the frame does not fit its base (the caller asks for a resync).
// bases: {"<type>/<stream>": {seq, data}}, mutated in place.
function receive(bases, event) {
  var key = event.type + "/" + event.stream, base = bases[key]
  var data = event.data
  if (has(event, "patch")) {
    if (!base || base.seq !== event.base) return undefined
    try { data = apply(base.data, event.patch) } catch (_) { return undefined }
  }
  bases[key] = {seq: event.seq, data: data}
  return data
}
