"""Durable notification deduplication; failures never resolve pending work."""
def changes(previous, threads):
    current = {t['key']: {'pending': t['pending'], 'revision': t['revision']} for t in threads}
    events = []
    for t in threads:
        old = previous.get(t['key'])
        if t['pending'] and (old is None or not old['pending'] or old['revision'] != t['revision']):
            events.append(t)
    return current, events
