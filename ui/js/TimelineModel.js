.pragma library
// Mirrors T3's timelineMinimapItems: one marker per user message, paired with
// the final assistant response before the next user message. Keep source intact.
function derive(rows) {
    var items = [], current = null
    for (var i = 0; i < rows.length; ++i) {
        var row = rows[i]
        if (row.role === "user") {
            current = {id: row.id, rowIndex: i, userText: row.text || "", assistantText: null}
            items.push(current)
        } else if (current && row.role === "assistant") current.assistantText = row.text || null
    }
    return items
}
function compact(text) { return String(text || "").replace(/\s+/g, " ").trim() }
function indexAt(y, height, count) {
    return count && height > 0 ? Math.max(0, Math.min(count - 1, Math.round(Math.max(0, Math.min(1, y / height)) * (count - 1)))) : -1
}
