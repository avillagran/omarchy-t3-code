#ifndef T3_PROTOCOL_H
#define T3_PROTOCOL_H
#include <stddef.h>
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Run blocking calls on a worker thread. Host owns login and secure storage.
 * Calls on a handle serialize. Never free a handle concurrently with a call.
 * JSON: connectBearer, connectRelay, request, rpc, disconnect.
 * Result: {"ok":true,"data":...} or {"ok":false,"error":"code"}.
 * Input must be valid for len bytes; the caller owns and clears its credentials.
 */
void *t3_client_new(void);
char *t3_client_call(void *handle, const uint8_t *input, size_t len);
void t3_string_free(char *response);
void t3_client_free(void *handle);
#ifdef __cplusplus
}
#endif
#endif
