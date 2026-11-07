"""Typed metadata boundary for an already-owned Wayland connection.

This observes selected public protocol messages without changing forwarded bytes.
The socket owner must separately authenticate the engine. No raw wire or text is
logged here; current text exists only in the private snapshot, never its repr.
"""
import struct


class BoundaryError(Exception):
    pass


class Snapshot:
    __slots__ = ('lease', 'object_id', 'surface', 'root_surface', 'serial',
                 'rectangle', 'text', 'cursor_byte', 'anchor_byte', 'hints', 'purpose')

    def __init__(self, lease, object_id, surface, root_surface, serial, state):
        self.lease, self.object_id = lease, object_id
        self.surface, self.root_surface, self.serial = surface, root_surface, serial
        self.rectangle = state.get('rectangle')
        surrounding = state.get('surrounding')
        self.text, self.cursor_byte, self.anchor_byte = surrounding or (None, None, None)
        self.hints, self.purpose = state.get('content_type', (None, None))

    def metadata(self):
        return {'kind': 'native-context', 'leaseKnown': self.lease is not None,
                'objectId': self.object_id, 'surfaceId': self.surface,
                'rootSurfaceId': self.root_surface, 'serial': self.serial,
                'rectangle': self.rectangle, 'coordinateSpace': 'native-surface-local',
                'surroundingAvailable': self.text is not None,
                'surroundingBytes': len(self.text.encode('utf-8')) if self.text is not None else None,
                'cursorByte': self.cursor_byte, 'anchorByte': self.anchor_byte,
                'contentHints': self.hints, 'contentPurpose': self.purpose}


class WireReader:
    def __init__(self, data):
        self.data, self.offset = data, 0

    def word(self, signed=False):
        if self.offset + 4 > len(self.data):
            raise BoundaryError('native-message-truncated')
        result = struct.unpack_from('<i' if signed else '<I', self.data, self.offset)[0]
        self.offset += 4
        return result

    def string(self):
        length = self.word()
        padded = (length + 3) & ~3
        if length == 0 or self.offset + padded > len(self.data):
            raise BoundaryError('native-string-shape')
        data = self.data[self.offset:self.offset + length]
        self.offset += padded
        if data[-1] != 0 or b'\0' in data[:-1]:
            raise BoundaryError('native-string-terminator')
        try:
            return data[:-1].decode('utf-8')
        except UnicodeError:
            raise BoundaryError('native-string-utf8') from None

    def finish(self):
        if self.offset != len(self.data):
            raise BoundaryError('native-message-shape')


class ContextBoundary:
    def __init__(self, expected_titles, publish, invalidate, object_limit=8192):
        self.expected_titles = dict(expected_titles)
        self.publish, self.invalidate = publish, invalidate
        self.object_limit = object_limit
        self.objects = {1: 'wl_display'}
        self.surfaces, self.surface_parent = set(), {}
        self.xdg_surfaces, self.toplevels, self.subsurfaces = {}, {}, {}
        self.surface_leases, self.surface_identities, self.contexts = {}, {}, {}
        self.buffers = {'client': bytearray(), 'server': bytearray()}
        self.disabled = False

    def _add(self, object_id, interface):
        if not 1 < object_id < 0xff000000 or object_id in self.objects:
            raise BoundaryError('native-object-identity')
        if len(self.objects) >= self.object_limit:
            raise BoundaryError('native-context-object-budget')
        self.objects[object_id] = interface

    def _invalidate_context(self, object_id, reason):
        context = self.contexts.get(object_id)
        if context is not None:
            self.invalidate(object_id, reason)

    def _invalidate_surface(self, surface, reason):
        for object_id, context in self.contexts.items():
            if context['surface'] == surface or self._root(context['surface']) == surface:
                self._invalidate_context(object_id, reason)
                context.update(current={}, pending={}, sent=None)

    def _forget(self, object_id):
        interface = self.objects.pop(object_id, None)
        if interface == 'zwp_text_input_v3':
            self._invalidate_context(object_id, 'native-context-destroyed')
            self.contexts.pop(object_id, None)
        if interface == 'wl_surface':
            for key, context in self.contexts.items():
                if context['surface'] == object_id or self._root(context['surface']) == object_id:
                    self._invalidate_context(key, 'native-surface-destroyed')
                    context.update(surface=None, current={}, pending={}, sent=None)
            self.surfaces.discard(object_id)
            for surface, parent in list(self.surface_parent.items()):
                if surface == object_id or parent == object_id:
                    self.surface_parent.pop(surface, None)
            self.surface_leases.pop(object_id, None)
            self.surface_identities.pop(object_id, None)
        if interface == 'wl_subsurface':
            surface = self.subsurfaces.pop(object_id, None)
            if surface is not None:
                self._invalidate_surface(surface, 'native-surface-parent-removed')
                self.surface_parent.pop(surface, None)
        if interface == 'xdg_toplevel':
            surface = self.toplevels.pop(object_id, None)
            if surface is not None:
                self.surface_leases.pop(surface, None)
                self.surface_identities.pop(surface, None)
                for key, context in self.contexts.items():
                    if self._root(context['surface']) == surface:
                        self._invalidate_context(key, 'native-window-role-destroyed')
                        context.update(current={}, pending={}, sent=None)
        if interface == 'xdg_surface':
            surface = self.xdg_surfaces.pop(object_id, None)
            if surface is not None:
                self._invalidate_surface(surface, 'native-xdg-role-destroyed')
                self.surface_leases.pop(surface, None)
                self.surface_identities.pop(surface, None)

    def _root(self, surface):
        visited = set()
        while surface in self.surface_parent:
            if surface in visited:
                raise BoundaryError('native-surface-cycle')
            visited.add(surface)
            surface = self.surface_parent[surface]
        return surface

    def close(self, reason='native-connection-closed'):
        if self.disabled:
            return
        self.disabled = True
        for object_id in self.contexts:
            self._invalidate_context(object_id, reason)
        self.contexts.clear()
        self.objects.clear()
        self.surfaces.clear()
        self.surface_parent.clear()
        self.xdg_surfaces.clear()
        self.toplevels.clear()
        self.subsurfaces.clear()
        self.surface_leases.clear()
        self.surface_identities.clear()
        self.buffers = {'client': bytearray(), 'server': bytearray()}

    def feed(self, direction, chunk):
        if self.disabled:
            return
        if direction not in self.buffers:
            raise BoundaryError('native-direction')
        if len(chunk) > 65536:
            self.close('native-read-chunk-budget')
            raise BoundaryError('native-read-chunk-budget')
        pending = self.buffers[direction]
        pending.extend(chunk)
        try:
            while len(pending) >= 8:
                object_id, header = struct.unpack_from('<II', pending)
                size, opcode = header >> 16, header & 0xffff
                if size < 8 or size > 65532 or size % 4:
                    raise BoundaryError('native-header-shape')
                if len(pending) < size:
                    break
                payload = bytes(pending[8:size])
                del pending[:size]
                self._message(direction, object_id, opcode, payload)
            if len(pending) > 65532:
                raise BoundaryError('native-incomplete-message-budget')
        except BoundaryError as error:
            self.close(str(error))
            raise

    def _message(self, direction, object_id, opcode, payload):
        interface = self.objects.get(object_id)
        reader = WireReader(payload)
        if direction == 'server':
            if interface == 'wl_display' and opcode == 1:
                forgotten = reader.word()
                if forgotten <= 1:
                    raise BoundaryError('native-server-object-identity')
                self._forget(forgotten)
                reader.finish()
            elif interface == 'wl_display' and opcode == 0:
                self.close('native-display-error')
            elif interface == 'zwp_text_input_v3':
                self._text_server(object_id, opcode, reader)
            return
        if interface == 'wl_display' and opcode == 1:
            self._add(reader.word(), 'wl_registry')
        elif interface == 'wl_registry' and opcode == 0:
            reader.word()
            name, version, new_id = reader.string(), reader.word(), reader.word()
            if name in ['wl_compositor', 'wl_subcompositor', 'xdg_wm_base', 'zwp_text_input_manager_v3']:
                if name == 'zwp_text_input_manager_v3' and version != 1:
                    raise BoundaryError('native-text-input-version')
                self._add(new_id, name)
        elif interface == 'wl_compositor' and opcode == 0:
            new_id = reader.word()
            self._add(new_id, 'wl_surface')
            self.surfaces.add(new_id)
        elif interface == 'wl_subcompositor' and opcode == 1:
            new_id, surface, parent = reader.word(), reader.word(), reader.word()
            if surface not in self.surfaces or parent not in self.surfaces:
                raise BoundaryError('native-subsurface-identity')
            self._add(new_id, 'wl_subsurface')
            self._invalidate_surface(surface, 'native-surface-parent-changed')
            self.subsurfaces[new_id], self.surface_parent[surface] = surface, parent
            self._root(surface)
        elif interface == 'xdg_wm_base' and opcode == 2:
            new_id, surface = reader.word(), reader.word()
            if surface not in self.surfaces:
                raise BoundaryError('native-xdg-surface-identity')
            self._add(new_id, 'xdg_surface')
            self.xdg_surfaces[new_id] = surface
        elif interface == 'xdg_surface' and opcode == 1:
            new_id = reader.word()
            surface = self.xdg_surfaces[object_id]
            if surface not in self.surfaces:
                raise BoundaryError('native-toplevel-surface-identity')
            self._add(new_id, 'xdg_toplevel')
            self.toplevels[new_id] = surface
        elif interface == 'xdg_toplevel' and opcode == 2:
            title = reader.string()
            surface = self.toplevels[object_id]
            old_identity = self.surface_identities.pop(surface, None)
            self.surface_leases.pop(surface, None)
            lease = self.expected_titles.get(title)
            if lease is not None:
                self.surface_leases[surface] = lease
                self.surface_identities[surface] = title
            if old_identity != (title if lease is not None else None):
                for key, context in self.contexts.items():
                    if self._root(context['surface']) == surface:
                        self._invalidate_context(key, 'native-window-identity-changed')
                        context.update(current={}, pending={}, sent=None)
        elif interface == 'zwp_text_input_manager_v3' and opcode == 1:
            new_id = reader.word()
            reader.word()  # Native seat object; authentication belongs to the socket owner.
            self._add(new_id, 'zwp_text_input_v3')
            self.contexts[new_id] = {'surface': None, 'enabled': False, 'count': 0,
                                     'current': {}, 'pending': {}, 'sent': None}
        elif interface == 'zwp_text_input_v3':
            self._text_client(object_id, opcode, reader)
            return
        elif opcode == 0 and interface in ['wl_surface', 'wl_subsurface', 'xdg_surface',
                                          'xdg_toplevel', 'xdg_wm_base',
                                          'wl_subcompositor', 'zwp_text_input_manager_v3']:
            self._forget(object_id)
        else:
            return  # Opaque unrelated protocol continues through the transport unchanged.
        reader.finish()

    def _text_client(self, object_id, opcode, reader):
        context = self.contexts[object_id]
        if opcode == 0:
            self._forget(object_id)
        elif opcode in [1, 2]:
            self._invalidate_context(object_id, 'native-context-reset')
            context.update(enabled=opcode == 1, current={}, pending={}, sent=None)
        elif opcode == 3:
            text, cursor, anchor = reader.string(), reader.word(signed=True), reader.word(signed=True)
            encoded = text.encode('utf-8')
            if len(encoded) > 4000 or not 0 <= cursor <= len(encoded) or not 0 <= anchor <= len(encoded):
                raise BoundaryError('native-surrounding-range')
            try:
                encoded[:cursor].decode('utf-8')
                encoded[:anchor].decode('utf-8')
            except UnicodeError:
                raise BoundaryError('native-surrounding-boundary') from None
            context['pending']['surrounding'] = (text, cursor, anchor)
        elif opcode == 4:
            reader.word()  # Change cause is not a fabricated cursor or query value.
        elif opcode == 5:
            context['pending']['content_type'] = (reader.word(), reader.word())
        elif opcode == 6:
            rectangle = tuple(reader.word(signed=True) for _ in range(4))
            if rectangle[2] < 0 or rectangle[3] < 0:
                raise BoundaryError('native-cursor-rectangle')
            context['pending']['rectangle'] = rectangle
        elif opcode == 7:
            context['count'] = (context['count'] + 1) & 0xffffffff
            context['current'].update(context['pending'])
            context['pending'] = {}
            # Only current and one awaiting snapshot are retained, never text history.
            context['sent'] = (context['count'], context['surface'], dict(context['current']))
        else:
            raise BoundaryError('native-text-input-request')
        reader.finish()

    def _text_server(self, object_id, opcode, reader):
        context = self.contexts[object_id]
        if opcode == 0:
            surface = reader.word()
            if surface not in self.surfaces:
                raise BoundaryError('native-enter-surface')
            if context['surface'] != surface:
                self._invalidate_context(object_id, 'native-context-enter')
                context.update(surface=surface, current={}, pending={}, sent=None)
        elif opcode == 1:
            surface = reader.word()
            if context['surface'] == surface:
                self._invalidate_context(object_id, 'native-context-leave')
                context.update(surface=None, enabled=False, current={}, pending={}, sent=None)
        elif opcode == 5:
            serial = reader.word()
            sent = context['sent']
            if sent is not None and sent[0] == serial and context['enabled'] and sent[1] == context['surface']:
                root = self._root(context['surface'])
                lease = self.surface_leases.get(root)
                if lease is not None:
                    self.publish(Snapshot(lease, object_id, context['surface'], root, serial, sent[2]))
        else:
            return  # Native preedit/commit events remain opaque to this context observer.
        reader.finish()
