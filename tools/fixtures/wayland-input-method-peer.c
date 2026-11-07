// Synthetic IME on a private compositor only. It never grabs the keyboard,
// opens a popup, logs surrounding text, or connects to the desktop's display.
#include "input-method-client.h"
#include <errno.h>
#include <poll.h>
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
static uint32_t serial;
static int active, pending_active;
static void activate(void *p, struct zwp_input_method_v2 *m) {
  (void)p;
  (void)m;
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
  (void)text;
  (void)cursor;
  (void)anchor;
}
static void cause(void *p, struct zwp_input_method_v2 *m, uint32_t value) {
  (void)p;
  (void)m;
  (void)value;
}
static void content(void *p, struct zwp_input_method_v2 *m, uint32_t hints,
                    uint32_t purpose) {
  (void)p;
  (void)m;
  (void)hints;
  (void)purpose;
}
static void done(void *p, struct zwp_input_method_v2 *m) {
  (void)p;
  (void)m;
  serial++;
  active = pending_active;
  printf("%s %u\n", active ? "active" : "inactive", serial);
  fflush(stdout);
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
int main(void) {
  display = wl_display_connect(NULL);
  if (!display)
    return 2;
  struct wl_registry *registry = wl_display_get_registry(display);
  wl_registry_add_listener(registry, &globals, NULL);
  if (wl_display_roundtrip(display) < 0 || !seat || !manager)
    return 2;
  method = zwp_input_method_manager_v2_get_input_method(manager, seat);
  zwp_input_method_v2_add_listener(method, &methods, NULL);
  if (wl_display_roundtrip(display) < 0)
    return 2;
  puts("ready");
  fflush(stdout);
  char input[128];
  size_t used = 0;
  int stopping = 0;
  while (!stopping) {
    while (wl_display_prepare_read(display) != 0)
      if (wl_display_dispatch_pending(display) < 0)
        return 2;
    if (wl_display_flush(display) < 0 && errno != EAGAIN) {
      wl_display_cancel_read(display);
      return 2;
    }
    struct pollfd fds[2] = {{wl_display_get_fd(display), POLLIN, 0},
                            {STDIN_FILENO, POLLIN, 0}};
    if (poll(fds, 2, 15000) <= 0) {
      wl_display_cancel_read(display);
      return 2;
    }
    if (fds[0].revents & POLLIN) {
      if (wl_display_read_events(display) < 0)
        return 2;
    } else
      wl_display_cancel_read(display);
    if (wl_display_dispatch_pending(display) < 0)
      return 2;
    if (fds[0].revents & (POLLERR | POLLHUP))
      return 2;
    if (fds[1].revents & POLLIN) {
      char bytes[64];
      ssize_t count = read(STDIN_FILENO, bytes, sizeof bytes);
      if (count <= 0)
        break;
      for (ssize_t at = 0; at < count; at++) {
        if (bytes[at] != '\n') {
          if (used + 1 >= sizeof input)
            return 2;
          input[used++] = bytes[at];
          continue;
        }
        input[used] = 0;
        used = 0;
        if (!strcmp(input, "quit")) {
          stopping = 1;
          break;
        }
        if (!active)
          return 2;
        if (!strcmp(input, "preedit")) {
          const char *text = "にほん";
          zwp_input_method_v2_set_preedit_string(
              method, text, (int32_t)strlen(text), (int32_t)strlen(text));
        } else if (!strcmp(input, "commit")) {
          zwp_input_method_v2_commit_string(method, "日本");
          zwp_input_method_v2_set_preedit_string(method, "", 0, 0);
        } else
          return 2;
        zwp_input_method_v2_commit(method, serial);
        wl_display_flush(display);
        printf("sent %s\n", input);
        fflush(stdout);
      }
    } else if (fds[1].revents & (POLLERR | POLLHUP))
      break;
  }
  zwp_input_method_v2_destroy(method);
  zwp_input_method_manager_v2_destroy(manager);
  wl_seat_destroy(seat);
  wl_registry_destroy(registry);
  wl_display_flush(display);
  wl_display_disconnect(display);
  return 0;
}
