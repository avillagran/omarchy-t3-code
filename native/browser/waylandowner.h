#pragma once
/* Admission for an app-private compositor, never a desktop-wide policy.
 * Only backend sockets CONNECTED by its actual supervisor may use globals.
 * The supervisor separately authenticates the engine's frontend socket and
 * delegates already-connected descriptors to its owned GUI/helper actors. */
#include <stdbool.h>
#include <stddef.h>
#include <fcntl.h>
#include <poll.h>
#include <sys/socket.h>
#include <sys/syscall.h>
#include <sys/types.h>
#include <unistd.h>
#include <wayland-server-core.h>

struct o3_wayland_rejected {
    struct wl_client *client;
    struct wl_listener destroyed;
};
struct o3_wayland_owner {
    pid_t supervisor;
    uid_t uid;
    bool installed;
    int supervisor_fd;
    struct wl_event_source *supervisor_watch;
    struct wl_listener created;
    struct wl_listener destroyed;
    struct wl_display *display;
    struct wl_event_source *pending;
    struct o3_wayland_rejected rejected[32];
};
static inline bool o3_wayland_owner_alive(const struct o3_wayland_owner *owner) {
    struct pollfd watch={owner->supervisor_fd,POLLIN,0};
    return owner->installed && owner->supervisor_fd>=0 && poll(&watch,1,0)==0;
}
static inline bool o3_wayland_owner_credentials(const struct o3_wayland_owner *owner,
        pid_t pid,uid_t uid) {
    return o3_wayland_owner_alive(owner) && pid==owner->supervisor && uid==owner->uid;
}
static inline bool o3_wayland_owner_admits_fd(const struct o3_wayland_owner *owner,int fd) {
    struct ucred credentials={0};socklen_t length=sizeof credentials;
    return getsockopt(fd,SOL_SOCKET,SO_PEERCRED,&credentials,&length)==0 &&
        length==sizeof credentials && o3_wayland_owner_credentials(owner,credentials.pid,credentials.uid);
}
static inline bool o3_wayland_owner_admits(const struct o3_wayland_owner *owner,
        const struct wl_client *client) {
    pid_t pid=0;uid_t uid=(uid_t)-1;gid_t gid=(gid_t)-1;
    wl_client_get_credentials(client,&pid,&uid,&gid);
    return o3_wayland_owner_credentials(owner,pid,uid);
}
static inline int o3_wayland_owner_parent_lost(int fd,uint32_t mask,void *data) {
    (void)fd;(void)mask;
    struct o3_wayland_owner *owner=data;
    wl_display_terminate(owner->display);
    return 0;
}
static inline void o3_wayland_owner_finish(struct o3_wayland_owner *owner) {
    if (!owner->installed) return;
    owner->installed=false;
    if (owner->supervisor_watch) wl_event_source_remove(owner->supervisor_watch);
    owner->supervisor_watch=NULL;
    close(owner->supervisor_fd);owner->supervisor_fd=-1;
    if (owner->pending) wl_event_source_remove(owner->pending);
    owner->pending=NULL;
    for (size_t index=0;index<32;index++) {
        struct o3_wayland_rejected *slot=&owner->rejected[index];
        if (slot->client) {
            wl_list_remove(&slot->destroyed.link);
            wl_list_init(&slot->destroyed.link);
            slot->client=NULL;
        }
    }
    wl_list_remove(&owner->created.link);
    wl_list_init(&owner->created.link);
    wl_list_remove(&owner->destroyed.link);
    wl_list_init(&owner->destroyed.link);
    owner->display=NULL;
}
static inline void o3_wayland_owner_destroyed(struct wl_listener *listener,void *data) {
    (void)data;
    struct o3_wayland_owner *owner=wl_container_of(listener,owner,destroyed);
    o3_wayland_owner_finish(owner);
}
static inline void o3_wayland_rejected_destroyed(struct wl_listener *listener,void *data) {
    (void)data;
    struct o3_wayland_rejected *slot=wl_container_of(listener,slot,destroyed);
    wl_list_remove(&slot->destroyed.link);
    wl_list_init(&slot->destroyed.link);
    slot->client=NULL;
}
static inline void o3_wayland_reject_idle(void *data) {
    struct o3_wayland_owner *owner=data;
    owner->pending=NULL;
    for (size_t index=0;index<32;index++)
        if (owner->rejected[index].client)
            wl_client_destroy(owner->rejected[index].client);
}
static inline void o3_wayland_owner_created(struct wl_listener *listener,void *data) {
    struct o3_wayland_owner *owner=wl_container_of(listener,owner,created);
    struct wl_client *client=data;
    if (!o3_wayland_owner_admits(owner,client)) {
        /* Post a fatal error, not wl_client_destroy inside its creation signal.
         * The global filter independently denies both advertisement and bind. */
        wl_client_post_implementation_error(client,"Private compositor client is not admitted");
        struct o3_wayland_rejected *slot=NULL;
        for (size_t index=0;index<32;index++)
            if (!owner->rejected[index].client) {slot=&owner->rejected[index];break;}
        if (!slot) {
            // No unbounded rejection queue; fail this private cohort closed.
            wl_display_terminate(owner->display);
            return;
        }
        slot->client=client;
        wl_list_init(&slot->destroyed.link);
        slot->destroyed.notify=o3_wayland_rejected_destroyed;
        wl_client_add_destroy_listener(client,&slot->destroyed);
        if (!owner->pending)
            owner->pending=wl_event_loop_add_idle(wl_display_get_event_loop(owner->display),o3_wayland_reject_idle,owner);
        if (!owner->pending) wl_display_terminate(owner->display);
    }
}
static inline bool o3_wayland_owner_init(struct o3_wayland_owner *owner,
        struct wl_display *display) {
    if (owner->installed || getppid()<=1) return false;
    owner->supervisor=getppid();owner->uid=getuid();
    owner->supervisor_fd=(int)syscall(SYS_pidfd_open,owner->supervisor,0);
    if (owner->supervisor_fd<0) return false;
    struct pollfd live={owner->supervisor_fd,POLLIN,0};
    if (getppid()!=owner->supervisor || poll(&live,1,0)!=0 ||
            fcntl(owner->supervisor_fd,F_SETFD,FD_CLOEXEC)<0) {
        close(owner->supervisor_fd);owner->supervisor_fd=-1;return false;
    }
    owner->supervisor_watch=wl_event_loop_add_fd(wl_display_get_event_loop(display),
        owner->supervisor_fd,WL_EVENT_READABLE,o3_wayland_owner_parent_lost,owner);
    if (!owner->supervisor_watch) {
        close(owner->supervisor_fd);owner->supervisor_fd=-1;return false;
    }
    owner->display=display;
    wl_list_init(&owner->created.link);wl_list_init(&owner->destroyed.link);
    owner->created.notify=o3_wayland_owner_created;
    owner->destroyed.notify=o3_wayland_owner_destroyed;
    owner->installed=true;
    wl_display_add_client_created_listener(display,&owner->created);
    wl_display_add_destroy_listener(display,&owner->destroyed);
    return true;
}
