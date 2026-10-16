"""Direct read-only T3 REST adapter. No DB access, Link server or model calls."""
import json
import hashlib
import math
import os
import re
import stat
import urllib.error
import urllib.parse
import urllib.request
from datetime import datetime, timezone
from pathlib import Path

MAX_BYTES = 8 * 1024 * 1024
ACTIVE = {'preparing', 'queued', 'starting', 'running'}

class T3Error(Exception):
    def __init__(self, code):
        self.code = code
        super().__init__(code)

class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, *args, **kwargs):
        return None

def valid_id(value):
    # T3-owned child IDs contain percent-encoded components. Encode again as one segment.
    return isinstance(value, str) and bool(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_:%-]{0,2047}', value))

def endpoint(value):
    try:
        p = urllib.parse.urlsplit(value)
        if (p.scheme not in ('http', 'https') or not p.hostname or p.username or p.password
                or p.query or p.fragment or p.path not in ('', '/') or p.port == 0):
            raise ValueError()
        # LAN HTTP remains compatible with omi; use trusted LAN or HTTPS remotely.
        return value.rstrip('/')
    except (ValueError, TypeError):
        raise T3Error('invalid_endpoint') from None

def read_token(path):
    try:
        fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(fd) as f:
            s = os.fstat(f.fileno())
            if not stat.S_ISREG(s.st_mode) or s.st_uid != os.getuid() or s.st_mode & 0o077:
                raise ValueError()
            token = f.read(4097).strip()
        if not 8 <= len(token) <= 4096 or any(c.isspace() or ord(c) < 32 for c in token):
            raise ValueError()
        return token
    except (OSError, ValueError, UnicodeError):
        raise T3Error('unauthorized') from None

def epoch_ms(value):
    try:
        if isinstance(value, bool): return 0
        if isinstance(value, (int, float)):
            if not math.isfinite(value) or value < 0: return 0
            return int(value * 1000 if value < 100_000_000_000 else value)
        if isinstance(value, str):
            dt = datetime.fromisoformat(value.replace('Z', '+00:00'))
            if dt.tzinfo is None: return 0
            return max(0, int(dt.timestamp() * 1000))
    except (ValueError, OverflowError, OSError):
        pass
    return 0

def normalize_shell(document, environment_id, now_ms=None):
    if not isinstance(document, dict) or not isinstance(document.get('threads'), list):
        raise T3Error('malformed')
    now_ms = now_ms if now_ms is not None else int(datetime.now(timezone.utc).timestamp() * 1000)
    projects = {p['id']: p.get('title', '') for p in document.get('projects', []) if isinstance(p, dict) and 'id' in p}
    result, ids = [], set()
    for t in document['threads']:
        if not isinstance(t, dict) or not valid_id(t.get('id')) or t['id'] in ids:
            raise T3Error('malformed')
        ids.add(t['id'])
        if t.get('deletedAt') or t.get('archivedAt'): continue
        status = t.get('status', 'unknown')
        if not isinstance(status, str) or not isinstance(t.get('title'), str): raise T3Error('malformed')
        # Activity fields can report background work after the root run completes.
        working = status in ACTIVE or t.get('activityRunStatus') in ACTIVE or bool(t.get('pendingBackgroundTasks'))
        reason = ('request' if t.get('pendingRuntimeRequest') is not None else
                  'plan' if t.get('hasActionableProposedPlan') else
                  'waiting' if status == 'waiting' else
                  'failed' if status == 'failed' else None)
        visited = epoch_ms(t.get('lastVisitedAt'))
        completed = epoch_ms(t.get('latestRunCompletedAt'))
        # A completed turn awaits review; never inspect the prose to infer urgency.
        if reason is None and not working and status == 'completed' and completed > visited:
            reason = 'review'
        if t.get('settledOverride') in (True, 'settled') or (t.get('settledAt') and t.get('settledOverride') != 'active'):
            reason = None
        snoozed = epoch_ms(t.get('snoozedUntil')) > now_ms
        pending = reason is not None and not snoozed
        request = t.get('pendingRuntimeRequest') or {}
        request_id = (request.get('id') or request.get('requestId') or 'pending') if isinstance(request, dict) else 'pending'
        revision_data = [reason or '', t.get('latestRunId') or '', request_id if request else '', str(completed or ('' if t.get('latestRunId') else t.get('updatedAt', '')))]
        revision = hashlib.sha256('\0'.join(revision_data).encode()).hexdigest()
        result.append({'id': t['id'], 'environmentId': environment_id,
                       'key': environment_id + '/' + t['id'], 'title': t['title'],
                       'project': projects.get(t.get('projectId'), t.get('projectId', '')),
                       'status': status, 'working': working, 'pending': pending,
                       'reason': reason, 'snoozed': snoozed,
                       'updatedAt': epoch_ms(t.get('updatedAt')), 'revision': revision,
                       'parentThreadId': (t.get('lineage') or {}).get('parentThreadId')})
    return sorted(result, key=lambda t: (-t['updatedAt'], t['key']))

class Client:
    def __init__(self, base, token_file, timeout=8):
        self.base = endpoint(base)
        self.token_file = token_file
        self.timeout = timeout
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def get(self, path, protocol=True):
        headers = {'Authorization': 'Bearer ' + read_token(self.token_file),
                   'Accept': 'application/json', 'Accept-Encoding': 'identity'}
        if protocol: headers['x-t3-orchestration-protocol'] = '2'
        try:
            with self.opener.open(urllib.request.Request(self.base + path, headers=headers), timeout=self.timeout) as r:
                if r.headers.get_content_type() != 'application/json': raise T3Error('not_json')
                if r.headers.get('Content-Encoding', 'identity').lower() not in ('identity', ''):
                    raise T3Error('unsupported_encoding')
                raw = r.read(MAX_BYTES + 1)
                if len(raw) > MAX_BYTES: raise T3Error('too_large')
                return json.loads(raw)
        except urllib.error.HTTPError as e:
            code = 'unauthorized' if e.code in (401,403) else 'http_' + str(e.code)
            e.close()
            raise T3Error(code) from None
        except (urllib.error.URLError, OSError): raise T3Error('unreachable') from None
        except (UnicodeError, ValueError): raise T3Error('malformed') from None

    def threads(self, environment_id):
        return normalize_shell(self.get('/api/orchestration/shell'), environment_id)

    def messages(self, thread_id):
        if not valid_id(thread_id): raise T3Error('invalid_id')
        d = self.get('/api/orchestration/threads/' + urllib.parse.quote(thread_id, safe='') + '/bounded')
        try:
            rows = d['projection']['messages']
            if not isinstance(rows, list): raise ValueError()
            messages = []
            for m in rows:
                if not isinstance(m,dict) or not isinstance(m.get('text'),str) or not isinstance(m.get('role'),str): raise ValueError()
                messages.append({'id': m.get('id',''), 'role':m['role'], 'text':m['text'],
                                 'createdAt':epoch_ms(m.get('createdAt'))})
            return {'messages':messages, 'hasMoreHistory':bool(d.get('hasMoreHistory')),
                    'payloadBudgetExceeded':bool(d.get('payloadBudgetExceeded'))}
        except (KeyError, TypeError, ValueError): raise T3Error('malformed') from None
