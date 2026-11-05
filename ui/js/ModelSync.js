.pragma library
// Keeps a ListModel of {row} items in step with an array of row objects without
// rebuilding delegates. Row objects are never mutated after they are built
// (snapshot deltas replace changed rows), so a print is computed once per object.

// The print is kept on the row itself as a non-enumerable member: JSON and
// Object.assign ignore it, so copies never inherit a stale print. (QV4's
// WeakMap crashes in sameValueZero under this load, so it is not used.)
var PRINT = "\u0001print"
function print(row) {
  if (row === null || typeof row !== "object") return JSON.stringify(row)
  if (Object.prototype.hasOwnProperty.call(row, PRINT)) return row[PRINT]
  var p = JSON.stringify(row)
  try { Object.defineProperty(row, PRINT, {value: p, enumerable: false, writable: false, configurable: false}) } catch (_) {}
  return p
}

// state: {keys: [], prints: {}} owned by the caller, updated in place.
// keyOf(row) names a row; item(row) is what a new model item holds. Rows whose
// print did not change are left alone; changed ones get their "row" role set.
function sync(model, rows, state, keyOf, item) {
  if (state.keys.length !== model.count) { model.clear(); state.keys = []; state.prints = {} }
  var keys = state.keys, known = state.prints, want = Object.create(null), seen = Object.create(null)
  var named = rows.map(function(row) {
    var key = String(keyOf(row)), n = seen[key] = (seen[key] || 0) + 1
    if (n > 1) key += "\u0001" + n
    want[key] = true
    return key
  })
  for (var k = keys.length - 1; k >= 0; k--) if (!want[keys[k]]) { model.remove(k); delete known[keys[k]]; keys.splice(k, 1) }
  for (var i = 0; i < rows.length; i++) {
    var row = rows[i], key = named[i], found = keys[i] === key ? i : keys.indexOf(key), p = print(row)
    if (found < 0) { model.insert(i, item(row)); keys.splice(i, 0, key) }
    else {
      if (found !== i) { model.move(found, i, 1); keys.splice(i, 0, keys.splice(found, 1)[0]) }
      if (known[key] !== p) model.setProperty(i, "row", row)
    }
    known[key] = p
  }
}
