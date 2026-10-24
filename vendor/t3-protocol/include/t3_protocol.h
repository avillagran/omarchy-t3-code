#ifndef T3_PROTOCOL_H
#define T3_PROTOCOL_H
#define T3_PROTOCOL_ABI_VERSION 3
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Run blocking calls on a worker thread. Host owns login and secure storage.
 * Calls may be concurrent. Never free a handle concurrently with a call.
 * JSON: connectBearer, connectRelay, request, requestOk, rpc, rpcWithTimeout,
 * disconnect, subscribe, pollSubscription, cancelSubscription, wakeSubscriptions,
 * fetchDescriptor, pairBearer, boundedThread. ABI v2 additions are optional
 * fields and new ops (capabilities lists them with every limit; see Streams):
 * connectBearer/connectRelay take "options" {requestTimeoutMs, connectTimeoutMs,
 * poolIdleTimeoutMs, maxBytes}; request takes "maxBytes". fetchDescriptor and
 * pairBearer need no connection; pairBearer returns the Bearer token for the
 * host's secure store and, with "connect":true, also switches the handle.
 * Result: {"ok":true,"data":...} or {"ok":false,"error":"code"}; a failure
 * may add "message" with the server's one-line reason. "error" stays a code.
 * Input must be valid for len bytes; the caller owns and clears its credentials.
 *
 * ABI 3 adds the typed API (additive; every op above is unchanged):
 *   {"op":"describe"} -> {"apiVersion","contract","functions":[{"fn","module",
 *     "tag","kind","replay","scope","timeoutMs","stream","args":[...]}]};
 *     needs no connection.
 *   {"op":"call","fn":"vcs.refreshStatus","args":{"cwd":"/r"},"timeoutMs"?}
 *     validates locally before any socket ("unknown_function", "invalid_input",
 *     "invalid_<field>", "invalid_rpc_timeout"), then sends exactly once.
 *   "write_not_authorized" failures add "requiredScope" (e.g. "settings:write").
 */
/* Streams (wake, boundaries, idle and completionMarker are additive to ABI 3).
 * {"op":"subscribe","tag":T,"payload":{...}} -> {subscriptionId}.
 * Up to 8 per handle, each on its own WebSocket; at most 2 may be
 * orchestration.subscribeThread with "acceptBoundedSnapshot":true. A ninth (or
 * a third bounded) fails with "subscription_limit"; an ended stream keeps its
 * slot until cancelSubscription. Each queue holds 64 items and 8 MiB of JSON;
 * a bounded thread stream takes one item up to 64 MiB in a 72 MiB queue, so a
 * handle queues at most 192 MiB. Overflow ends the stream with
 * "stream_backpressure", keeping what was queued. One consumer polls each stream:
 * {"op":"pollSubscription","subscriptionId":S,"timeoutMs":0..1000} (default 1000)
 * returns as soon as there is something new (items, a chunk end, idle turning
 * true, or the end), else after timeoutMs, with
 *   {"items":[...],"boundaries":[n,...],"idle":bool,"completionMarker":bool,
 *    "closed":bool,"error":"code"|null,"errorMessage":"reason"|null}
 * items: native T3 stream items in server order; never dropped or reordered.
 * boundaries: strictly ascending end offsets into this batch's items. Each n
 *   means a server chunk ended after items[0..n): apply/paint there. Items after
 *   the last boundary belong to a chunk that continues in the next poll; n = 0
 *   means the chunk whose items ended the previous batch ended right there.
 * idle: the last chunk is complete and no new frame completed during one 100 ms
 *   read tick: caught up for now (a large frame may still be in transit, and it
 *   can be true before the first item). It stays true until the next item;
 *   turning true wakes one waiting poll. Without requestCompletionMarker it is
 *   the only catch-up signal (the rule ot3 uses); with it, wait for completionMarker.
 * completionMarker: only when the payload had "requestCompletionMarker":true;
 *   true from the batch carrying the server's {"kind":"synchronized"} item (kept
 *   in items, at its position) onward: the initial snapshot/replay is complete.
 * closed/error: once closed the stream delivers nothing more; error is a stable
 *   code (null after cancel), errorMessage an optional one-line reason.
 * {"op":"wakeSubscriptions"} (optionally "subscriptionId":S) -> {"probed":n}:
 * after a resume from suspend or a network change, each open stream pings at
 * once (on its next 100 ms read tick) and ends with "stream_timeout" unless a
 * frame arrives within 3 s of this call. n counts open streams probed; an
 * unknown id fails with "unknown_subscription". It never reconnects: on
 * stream_timeout the host cancels, then subscribes again from its cursor. */
void *t3_client_new(void);
char *t3_client_call(void *handle, const uint8_t *input, size_t len);
/* Upload read/release callbacks may run on an HTTP worker. With both callbacks
 * present, ownership of context transfers even on invalid input. release runs
 * exactly once when Rust drops the reader. Return 0 for EOF or -1 for error;
 * never return more than capacity. Callbacks must not unwind or throw. */
typedef ptrdiff_t (*t3_read_callback)(void *context, uint8_t *buffer, size_t capacity);
typedef void (*t3_release_callback)(void *context);
char *t3_client_upload(void *handle, const uint8_t *metadata, size_t len,
    t3_read_callback read, t3_release_callback release, void *context);
/* Metadata: {id,name,mimeType}. Validated image bytes are borrowed during the
 * callback, on the calling thread. Copy before returning; host owns context.
 * Same call, other signed assets (never Base64 in JSON):
 *   {"op":"assetBytes","resource":{...assets.createUrl resource},"maxBytes":N,"timeoutMs":T}
 *     -> {sizeBytes,mimeType}; maxBytes 1..64 MiB, timeoutMs optional (30 s).
 *   {"op":"projectFavicon","cwd":"/repo","path":"favicon.svg"}
 *     -> {found,sizeBytes,mimeType}; no callback when found is false.
 * Through t3_client_call these two ops fail with "binary_operation". */
typedef void (*t3_image_callback)(void *context, const uint8_t *bytes, size_t len);
char *t3_client_image(void *handle, const uint8_t *metadata, size_t len,
    t3_image_callback receive, void *context);
void t3_string_free(char *response);
void t3_client_free(void *handle);
#ifdef __cplusplus
}
#endif
#endif
