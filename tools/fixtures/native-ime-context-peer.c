// Observe only an owned synthetic page through the native input-method protocol.
// No keyboard grab, text logging, DOM inspection, or host display connection.
#define _GNU_SOURCE
#include "input-method-client.h"
#include <errno.h>
#include <poll.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <wayland-client.h>
static struct wl_display *display;
static struct wl_seat *seat;
static struct zwp_input_method_manager_v2 *manager;
static struct zwp_input_method_v2 *method;
static struct wl_compositor *compositor;
static struct wl_shm *shm;
static struct wl_surface *surface;
static struct wl_buffer *buffer;
static struct zwp_input_popup_surface_v2 *popup;
static uint32_t serial, cursor_bytes, anchor_bytes, hints, purpose;
static int failure(void) {
  const struct wl_interface *interface = NULL;
  uint32_t object = 0;
  int error = display ? wl_display_get_error(display) : 0;
  uint32_t protocol = display ? wl_display_get_protocol_error(display, &interface, &object) : 0;
  const char *name = interface && (!strcmp(interface->name, "wl_shm") ||
                                  !strcmp(interface->name, "wl_surface") ||
                                  !strcmp(interface->name, "wl_display") ||
                                  !strcmp(interface->name, "zwp_input_method_v2"))
                         ? interface->name : "other";
  printf("{\"kind\":\"peer-error\",\"error\":%d,\"protocol\":%u,\"object\":%u,\"interface\":\"%s\"}\n",
         error, protocol, object, name);
  fflush(stdout);
  return 2;
}

static size_t text_bytes;
static int matched_text, has_surrounding;
static int active, pending_active;
static void activate(void *p, struct zwp_input_method_v2 *m) {
  (void)p;
  (void)m;
  // Protocol activation resets the previous field's pending context.
  text_bytes = 0;
  cursor_bytes = anchor_bytes = hints = purpose = 0;
  matched_text = has_surrounding = 0;
  pending_active = 1;
}
static void deactivate(void *p, struct zwp_input_method_v2 *m) {
  (void)p;
  (void)m;
  pending_active = 0;
}
static void surrounding(void *p, struct zwp_input_method_v2 *m,
                        const char *text, uint32_t cursor, uint32_t anchor) {
  (void)p;
  (void)m;
  text_bytes = strnlen(text, 4001);
  if (text_bytes > 4000 || cursor > text_bytes || anchor > text_bytes)
    exit(2);
  matched_text = !strcmp(text, "A😀日本Z") || !strcmp(text, "A😀日本ZQ");
  cursor_bytes = cursor;
  anchor_bytes = anchor;
  has_surrounding = 1;
}
static void cause(void *p, struct zwp_input_method_v2 *m, uint32_t value) {
  (void)p;
  (void)m;
  (void)value;
}
static void content(void *p, struct zwp_input_method_v2 *m, uint32_t next_hints,
                    uint32_t next_purpose) {
  (void)p;
  (void)m;
  hints = next_hints;
  purpose = next_purpose;
}
static void done(void *p, struct zwp_input_method_v2 *m) {
  (void)p;
  (void)m;
  serial++;
  active = pending_active;
  printf("{\"kind\":\"context\",\"active\":%d,\"serial\":%u,\"available\":%d,\"bytes\":%zu,\"cursor\":%u,\"anchor\":%u,\"matchesSynthetic\":%d,\"hints\":%u,\"purpose\":%u}\n",
         active, serial, has_surrounding, text_bytes, cursor_bytes, anchor_bytes,
         matched_text, hints, purpose);
  fflush(stdout);
  // Acknowledge the actual state serial without inserting or deleting text.
  // Chromium's text-input-v3 queues subsequent context until this done/commit
  // exchange completes; observing activation alone is insufficient.
  if (active) {
    zwp_input_method_v2_commit(m, serial);
    wl_display_flush(display);
  }
}
static void unavailable(void *p, struct zwp_input_method_v2 *m) {
  (void)p;
  (void)m;
  fputs("private input method unavailable\n", stderr);
  exit(2);
}
static const struct zwp_input_method_v2_listener methods = {
    activate, deactivate, surrounding, cause, content, done, unavailable};
static void capabilities(void *p, struct wl_seat *s, uint32_t value) {
  (void)p;
  (void)s;
  (void)value;
}
static void seat_name(void *p, struct wl_seat *s, const char *name) {
  (void)p;
  (void)s;
  (void)name;
}
static const struct wl_seat_listener seats = {capabilities, seat_name};
static void global(void *p, struct wl_registry *r, uint32_t name,
                   const char *interface, uint32_t version) {
  (void)p;
  if (!strcmp(interface, "wl_compositor") && !compositor)
    compositor = wl_registry_bind(r, name, &wl_compositor_interface,
                                  version < 4 ? version : 4);
  if (!strcmp(interface, "wl_shm") && !shm)
    shm = wl_registry_bind(r, name, &wl_shm_interface, 1);
  if (!strcmp(interface, "wl_seat") && !seat) {
    seat = wl_registry_bind(r, name, &wl_seat_interface,
                            version < 7 ? version : 7);
    wl_seat_add_listener(seat, &seats, NULL);
  }
  if (!strcmp(interface, "zwp_input_method_manager_v2") && !manager)
    manager =
        wl_registry_bind(r, name, &zwp_input_method_manager_v2_interface, 1);
}
static void removed(void *p, struct wl_registry *r, uint32_t name) {
  (void)p;
  (void)r;
  (void)name;
}
static const struct wl_registry_listener globals = {global, removed};
static void rectangle(void *data, struct zwp_input_popup_surface_v2 *object,
                      int32_t x, int32_t y, int32_t width, int32_t height) {
  (void)data;
  (void)object;
  printf("{\"kind\":\"rectangle\",\"x\":%d,\"y\":%d,\"width\":%d,\"height\":%d}\n",
         x, y, width, height);
  fflush(stdout);
}
static const struct zwp_input_popup_surface_v2_listener popup_events = {rectangle};
static int close_popup(void) {
  if (!popup)
    return 0;
  // Retire the observer's own role while its focused native view still exists.
  // The server roundtrip is an ownership barrier, not a timing delay.
  zwp_input_popup_surface_v2_destroy(popup);
  popup = NULL;
  wl_surface_destroy(surface);
  surface = NULL;
  wl_buffer_destroy(buffer);
  buffer = NULL;
  return wl_display_roundtrip(display) < 0 ? failure() : 0;
}
int main(void) {
  display = wl_display_connect(NULL);
  if (!display)
    return failure();
  struct wl_registry *registry = wl_display_get_registry(display);
  wl_registry_add_listener(registry, &globals, NULL);
  if (wl_display_roundtrip(display) < 0 || !seat || !manager || !compositor || !shm)
    return failure();
  method = zwp_input_method_manager_v2_get_input_method(manager, seat);
  zwp_input_method_v2_add_listener(method, &methods, NULL);
  if (wl_display_roundtrip(display) < 0)
    return failure();
  surface = wl_compositor_create_surface(compositor);
  popup = zwp_input_method_v2_get_input_popup_surface(method, surface);
  zwp_input_popup_surface_v2_add_listener(popup, &popup_events, NULL);
  int fd = memfd_create("owned-native-ime-popup", MFD_CLOEXEC | MFD_ALLOW_SEALING);
  uint32_t pixel = 0;
  if (fd < 0 || ftruncate(fd, sizeof pixel) || write(fd, &pixel, sizeof pixel) != (ssize_t)sizeof pixel ||
      // A Wayland SHM pool permits the server's writable mapping. Seal size,
      // not writes; this fixture never changes its single transparent pixel.
      fcntl(fd, F_ADD_SEALS, F_SEAL_GROW | F_SEAL_SHRINK | F_SEAL_SEAL) < 0)
    return failure();
  struct wl_shm_pool *pool = wl_shm_create_pool(shm, fd, sizeof pixel);
  buffer = wl_shm_pool_create_buffer(pool, 0, 1, 1, 4, WL_SHM_FORMAT_ARGB8888);
  wl_shm_pool_destroy(pool);
  close(fd);
  wl_surface_attach(surface, buffer, 0, 0);
  wl_surface_damage(surface, 0, 0, 1, 1);
  wl_surface_commit(surface);
  puts("{\"kind\":\"ready\"}");
  fflush(stdout);
  char input[128];
  size_t used = 0;
  int stopping = 0;
  while (!stopping) {
    while (wl_display_prepare_read(display) != 0)
      if (wl_display_dispatch_pending(display) < 0)
        return failure();
    if (wl_display_flush(display) < 0 && errno != EAGAIN) {
      wl_display_cancel_read(display);
      return failure();
    }
    struct pollfd fds[2] = {{wl_display_get_fd(display), POLLIN, 0},
                            {STDIN_FILENO, POLLIN, 0}};
    if (poll(fds, 2, 15000) <= 0) {
      wl_display_cancel_read(display);
      return failure();
    }
    if (fds[0].revents & POLLIN) {
      if (wl_display_read_events(display) < 0)
        return failure();
    } else
      wl_display_cancel_read(display);
    if (wl_display_dispatch_pending(display) < 0)
      return failure();
    if (fds[0].revents & (POLLERR | POLLHUP))
      return failure();
    if (fds[1].revents & POLLIN) {
      char bytes[64];
      ssize_t count = read(STDIN_FILENO, bytes, sizeof bytes);
      if (count <= 0)
        break;
      for (ssize_t at = 0; at < count; at++) {
        if (bytes[at] != '\n') {
          if (used + 1 >= sizeof input)
            return failure();
          input[used++] = bytes[at];
          continue;
        }
        input[used] = 0;
        used = 0;
        if (!strcmp(input, "quit")) {
          stopping = 1;
          break;
        }
        if (!strcmp(input, "close-popup")) {
          if (close_popup())
            return 2;
          puts("{\"kind\":\"popup-closed\"}");
          fflush(stdout);
          continue;
        }
        return failure();
      }
    } else if (fds[1].revents & (POLLERR | POLLHUP))
      break;
  }
  if (close_popup())
    return 2;
  wl_shm_destroy(shm);
  wl_compositor_destroy(compositor);
  zwp_input_method_v2_destroy(method);
  zwp_input_method_manager_v2_destroy(manager);
  wl_seat_destroy(seat);
  wl_registry_destroy(registry);
  wl_display_flush(display);
  wl_display_disconnect(display);
  return 0;
}

