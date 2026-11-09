// Native libwayland marshalling oracle on an owned socket pair, not a compositor.
#define _POSIX_C_SOURCE 200809L
#include "text-input-v3.h"
#include "xdg-shell.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <wayland-client.h>

int main(int argc, char **argv) {
  if (argc != 2)
    return 2;
  struct wl_display *display = wl_display_connect_to_fd(atoi(argv[1]));
  if (!display)
    return 2;
  struct wl_registry *registry = wl_display_get_registry(display);
  struct wl_compositor *compositor = wl_registry_bind(registry, 11, &wl_compositor_interface, 4);
  struct wl_seat *seat = wl_registry_bind(registry, 12, &wl_seat_interface, 7);
  struct xdg_wm_base *shell = wl_registry_bind(registry, 13, &xdg_wm_base_interface, 6);
  struct zwp_text_input_manager_v3 *manager =
      wl_registry_bind(registry, 14, &zwp_text_input_manager_v3_interface, 1);
  struct wl_surface *surface = wl_compositor_create_surface(compositor);
  struct xdg_surface *xdg_surface = xdg_wm_base_get_xdg_surface(shell, surface);
  struct xdg_toplevel *toplevel = xdg_surface_get_toplevel(xdg_surface);
  xdg_toplevel_set_title(toplevel, "owned-context-wire-nonce");
  struct zwp_text_input_v3 *input = zwp_text_input_manager_v3_get_text_input(manager, seat);
  if (wl_display_flush(display) < 0)
    return 2;
  printf("{\"kind\":\"ids\",\"surface\":%u,\"input\":%u,\"toplevel\":%u}\n",
         wl_proxy_get_id((struct wl_proxy *)surface),
         wl_proxy_get_id((struct wl_proxy *)input),
         wl_proxy_get_id((struct wl_proxy *)toplevel));
  fflush(stdout);
  char command[128];
  while (fgets(command, sizeof command, stdin)) {
    if (!strcmp(command, "quit\n"))
      break;
    if (!strcmp(command, "state\n")) {
      zwp_text_input_v3_enable(input);
      zwp_text_input_v3_set_surrounding_text(input, "A😀日本ZQ", 13, 13);
      zwp_text_input_v3_set_content_type(input, 4, 0);
      zwp_text_input_v3_set_cursor_rectangle(input, -3, 17, 0, 18);
      zwp_text_input_v3_commit(input);
    } else if (!strcmp(command, "move\n")) {
      zwp_text_input_v3_set_surrounding_text(input, "A😀日本ZQ", 11, 13);
      zwp_text_input_v3_set_cursor_rectangle(input, 12, 34, 1, 19);
      zwp_text_input_v3_commit(input);
    } else if (!strcmp(command, "empty\n")) {
      zwp_text_input_v3_set_surrounding_text(input, "", 0, 0);
      zwp_text_input_v3_commit(input);
    } else if (!strcmp(command, "rectangle-only\n")) {
      zwp_text_input_v3_enable(input);
      zwp_text_input_v3_set_cursor_rectangle(input, 22, 33, 0, 18);
      zwp_text_input_v3_commit(input);
    } else if (!strcmp(command, "rename\n")) {
      xdg_toplevel_set_title(toplevel, "foreign-window-title");
    } else if (!strcmp(command, "disable\n")) {
      zwp_text_input_v3_disable(input);
      zwp_text_input_v3_commit(input);
    } else if (!strcmp(command, "bad-boundary\n")) {
      zwp_text_input_v3_set_surrounding_text(input, "A😀日本ZQ", 2, 13);
      zwp_text_input_v3_commit(input);
    } else if (!strcmp(command, "destroy-context\n")) {
      zwp_text_input_v3_destroy(input);
      input = NULL;
    } else {
      return 2;
    }
    if (wl_display_flush(display) < 0)
      return 2;
    puts("{\"kind\":\"sent\"}");
    fflush(stdout);
  }
  wl_display_disconnect(display);
  return 0;
}
