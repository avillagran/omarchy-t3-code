# Physical RPC session ownership

`rpc_sessions::Owners` provides opt-in per-environment/read-or-write owners for
the published SDK's persistent unary RPC socket. Its random SDK socket ID is the
cache boundary. Private route/auth admission comparison determines when a socket
must be replaced; it never becomes a public identity or credential-derived epoch.

Catalog rebuilds reconcile only changed or removed admissions. Opening handshakes
serialize within their own environment/access mode and recheck current admission
before publishing. Retiring a pending handshake cannot resurrect its previous
socket. Unrelated environments keep their owners. Generic old stream failures do
not close a newer healthy unary owner: physical transport failures arrive through
the SDK's non-owning `closed()` observer with the exact socket ID.

The watch process forwards `rpc-session-state` events with environment, access
mode, opaque session ID and open/closed state. Consumers must ignore delayed
closed events for an older socket. The owner never retries an uncertain request.
An explicit later read may obtain a replacement socket with a new identity.

The isolated owner test exercises actual bearer WebSockets, route/auth replacement,
retirement during a held handshake, unrelated-environment preservation, physical
failure/replacement, exact close identity and removal from the environment catalog.

This foundation is opt-in. Terminal paths do not require file metadata in native
T3; file-reference/markdown metadata caching and invalidation must use their actual
consumer lifecycle. Unary ownership does not yet multiplex native subscriptions.
