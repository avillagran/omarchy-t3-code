/* Kernel credential and listener lifetime checks. No compositor or engine. */
#include "waylandowner.h"
#include <wayland-client.h>
#include <wayland-server.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <sys/resource.h>
#include <sys/un.h>
#include <poll.h>
#include <stdarg.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>

#define CHECK(value) do {if(!(value)){fprintf(stderr,"OWNER_CHECK_FAIL line=%d\n",__LINE__);return 1;}}while(false)
struct server_state {struct wl_display *display;struct o3_wayland_owner owner;int control;unsigned binds;};
static void quiet(const char *format,va_list arguments){(void)format;(void)arguments;}
static bool filter(const struct wl_client *client,const struct wl_global *global,void *data) {
    (void)global;
    bool allowed=o3_wayland_owner_admits(data,client);
    if(allowed!=o3_wayland_owner_admits_fd(data,wl_client_get_fd((struct wl_client *)client)))abort();
    return allowed;
}
static void bind_global(struct wl_client *client,void *data,uint32_t version,uint32_t id) {
    struct server_state *state=data;state->binds++;
    wl_resource_create(client,&wl_compositor_interface,version,id);
}
static int control_ready(int fd,uint32_t mask,void *data) {
    (void)mask;struct server_state *state=data;char command=0;
    if(recv(fd,&command,1,0)!=1 || command=='q'){wl_display_terminate(state->display);return 0;}
    if(command=='s')send(fd,&state->binds,sizeof state->binds,MSG_NOSIGNAL);
    if(command=='x') {
        /* Creation happens in one callback: the idle drain cannot run first. */
        int peers[33];
        for(unsigned index=0;index<33;index++) {
            int pair[2];if(socketpair(AF_UNIX,SOCK_STREAM|SOCK_CLOEXEC,0,pair)!=0)abort();
            if(!wl_client_create(state->display,pair[0]))abort();
            peers[index]=pair[1];
        }
        unsigned occupied=0;
        for(unsigned index=0;index<32;index++)occupied+=state->owner.rejected[index].client!=NULL;
        if(occupied!=32)abort();
        for(unsigned index=0;index<33;index++)close(peers[index]);
    }
    return 0;
}
static int exhausted_init(void) {
    struct wl_display *display=wl_display_create();CHECK(display);
    struct rlimit original;CHECK(getrlimit(RLIMIT_NOFILE,&original)==0);
    struct rlimit bounded=original;bounded.rlim_cur=64;
    CHECK(setrlimit(RLIMIT_NOFILE,&bounded)==0);
    int handles[64];unsigned count=0;
    while(count<64) {int fd=open("/dev/null",O_RDONLY|O_CLOEXEC);if(fd<0)break;handles[count++]=fd;}
    CHECK(count>0 && errno==EMFILE);
    struct o3_wayland_owner owner={0};CHECK(!o3_wayland_owner_init(&owner,display));
    CHECK(!owner.installed && !owner.supervisor_watch);
    for(unsigned index=0;index<count;index++)close(handles[index]);
    CHECK(setrlimit(RLIMIT_NOFILE,&original)==0);
    wl_display_destroy(display);return 0;
}
static int server(int control,const char *directory) {
    CHECK(exhausted_init()==0);
    struct server_state state={.control=control};
    state.display=wl_display_create();CHECK(state.display);
    CHECK(o3_wayland_owner_init(&state.owner,state.display));
    CHECK(!o3_wayland_owner_init(&state.owner,state.display));
    wl_display_set_global_filter(state.display,filter,&state.owner);
    struct wl_global *global=wl_global_create(state.display,&wl_compositor_interface,1,&state,bind_global);
    CHECK(global);
    CHECK(setenv("XDG_RUNTIME_DIR",directory,1)==0);
    CHECK(wl_display_add_socket(state.display,"owned-auth")==0);
    struct wl_event_source *source=wl_event_loop_add_fd(wl_display_get_event_loop(state.display),control,WL_EVENT_READABLE,control_ready,&state);
    CHECK(source);CHECK(send(control,"r",1,MSG_NOSIGNAL)==1);
    wl_display_run(state.display);
    wl_event_source_remove(source);
    wl_display_destroy_clients(state.display);
    wl_global_destroy(global);
    wl_display_destroy(state.display);
    CHECK(!state.owner.installed && !state.owner.pending);
    o3_wayland_owner_finish(&state.owner); /* Idempotent after real destroy signal. */
    close(control);return 0;
}
struct observed {uint32_t name;unsigned globals;};
static void advertised(void *data,struct wl_registry *registry,uint32_t name,const char *interface,uint32_t version) {
    (void)registry;(void)version;struct observed *seen=data;
    if(strcmp(interface,"wl_compositor")==0){seen->name=name;seen->globals++;}
}
static void removed(void *data,struct wl_registry *registry,uint32_t name){(void)data;(void)registry;(void)name;}
static const struct wl_registry_listener registry_listener={advertised,removed};
static int foreign_bind(const char *path,uint32_t known_name) {
    struct wl_display *display=wl_display_connect(path);CHECK(display);
    struct wl_registry *registry=wl_display_get_registry(display);
    struct observed seen={0};wl_registry_add_listener(registry,&registry_listener,&seen);
    /* Guess a real advertised name without waiting for our registry reply. */
    struct wl_compositor *object=wl_registry_bind(registry,known_name,&wl_compositor_interface,1);
    CHECK(object);CHECK(wl_display_roundtrip(display)<0);CHECK(seen.globals==0);
    wl_proxy_destroy((struct wl_proxy *)object);wl_registry_destroy(registry);wl_display_disconnect(display);
    return 0;
}
static int foreign_idle(const char *path) {
    struct wl_display *display=wl_display_connect(path);CHECK(display);
    /* Send no Wayland request. Rejection must not rely on the client helping. */
    const int descriptor=wl_display_get_fd(display);size_t total=0;char bytes[256];
    for(unsigned iteration=0;iteration<20;iteration++) {
        struct pollfd ready={descriptor,POLLIN|POLLHUP|POLLERR,0};
        CHECK(poll(&ready,1,100)>0);
        const ssize_t count=recv(descriptor,bytes,sizeof bytes,MSG_DONTWAIT);
        if(count==0 || (count<0 && errno==ECONNRESET)){wl_display_disconnect(display);return 0;}
        CHECK(count>0);total+=(size_t)count;CHECK(total<=4096);
    }
    wl_display_disconnect(display);return 1;
}
static int parent_lifetime(void) {
    int pair[2];CHECK(socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,pair)==0);
    pid_t supervisor=fork();CHECK(supervisor>=0);
    if(supervisor==0) {
        close(pair[0]);int barrier[2];if(pipe2(barrier,O_CLOEXEC)!=0)_exit(2);
        pid_t child=fork();if(child<0)_exit(3);
        if(child==0) {
            close(barrier[0]);struct wl_display *display=wl_display_create();
            struct o3_wayland_owner owner={0};
            if(!display || !o3_wayland_owner_init(&owner,display))_exit(4);
            if(write(barrier[1],"r",1)!=1)_exit(5);
            close(barrier[1]);
            wl_display_run(display);
            if(o3_wayland_owner_alive(&owner))_exit(6);
            wl_display_destroy(display);
            if(owner.installed || owner.supervisor_watch || owner.supervisor_fd!=-1)_exit(7);
            if(send(pair[1],"d",1,MSG_NOSIGNAL)!=1)_exit(8);
            close(pair[1]);_exit(0);
        }
        close(barrier[1]);char ready=0;
        if(read(barrier[0],&ready,1)!=1 || ready!='r')_exit(9);
        close(barrier[0]);close(pair[1]);_exit(0);
    }
    close(pair[1]);int status=0;
    CHECK(waitpid(supervisor,&status,0)==supervisor && WIFEXITED(status) && WEXITSTATUS(status)==0);
    struct pollfd ready={pair[0],POLLIN,0};CHECK(poll(&ready,1,2000)>0);
    char done=0;CHECK(recv(pair[0],&done,1,0)==1 && done=='d');close(pair[0]);return 0;
}
int main(void) {
    struct rlimit limit;CHECK(getrlimit(RLIMIT_CORE,&limit)==0 && limit.rlim_cur==0 && limit.rlim_max==0);
    CHECK(parent_lifetime()==0);
    wl_log_set_handler_client(quiet);wl_log_set_handler_server(quiet);
    char directory[]="/tmp/o3-owned-wayland-auth-XXXXXX";CHECK(mkdtemp(directory));
    char path[108];CHECK(snprintf(path,sizeof path,"%s/owned-auth",directory)<(int)sizeof path);
    int control[2];CHECK(socketpair(AF_UNIX,SOCK_SEQPACKET|SOCK_CLOEXEC,0,control)==0);
    const pid_t compositor=fork();CHECK(compositor>=0);
    if(compositor==0){close(control[0]);_exit(server(control[1],directory));}
    close(control[1]);char ready=0;CHECK(recv(control[0],&ready,1,0)==1 && ready=='r');
    struct wl_display *display=wl_display_connect(path);CHECK(display);
    struct wl_registry *registry=wl_display_get_registry(display);struct observed seen={0};
    wl_registry_add_listener(registry,&registry_listener,&seen);
    CHECK(wl_display_roundtrip(display)>=0 && seen.globals==1 && seen.name>0);
    struct wl_compositor *object=wl_registry_bind(registry,seen.name,&wl_compositor_interface,1);CHECK(object);
    CHECK(wl_display_roundtrip(display)>=0);
    wl_proxy_destroy((struct wl_proxy *)object);wl_registry_destroy(registry);wl_display_disconnect(display);
    /* Supervisor connects, then delegates the already-connected socket. Kernel
     * SO_PEERCRED stays the connecting supervisor, not the consuming helper. */
    int delegated=socket(AF_UNIX,SOCK_STREAM|SOCK_CLOEXEC,0);CHECK(delegated>=0);
    struct sockaddr_un address={.sun_family=AF_UNIX};
    CHECK(strlen(path)<sizeof address.sun_path);strcpy(address.sun_path,path);
    CHECK(connect(delegated,(struct sockaddr *)&address,sizeof address)==0);
    const pid_t consumer=fork();CHECK(consumer>=0);
    if(consumer==0) {
        close(control[0]);
        struct wl_display *owned=wl_display_connect_to_fd(delegated);if(!owned)_exit(2);
        struct wl_registry *view=wl_display_get_registry(owned);struct observed accepted={0};
        wl_registry_add_listener(view,&registry_listener,&accepted);
        if(wl_display_roundtrip(owned)<0 || accepted.globals!=1)_exit(3);
        struct wl_compositor *bound=wl_registry_bind(view,accepted.name,&wl_compositor_interface,1);
        if(!bound || wl_display_roundtrip(owned)<0)_exit(4);
        wl_proxy_destroy((struct wl_proxy *)bound);wl_registry_destroy(view);wl_display_disconnect(owned);_exit(0);
    }
    close(delegated);int delegated_status=0;
    CHECK(waitpid(consumer,&delegated_status,0)==consumer && WIFEXITED(delegated_status) && WEXITSTATUS(delegated_status)==0);
    for(unsigned mode=0;mode<2;mode++) {
        const pid_t child=fork();CHECK(child>=0);
        if(child==0){close(control[0]);_exit(mode==0?foreign_bind(path,seen.name):foreign_idle(path));}
        int status=0;CHECK(waitpid(child,&status,0)==child && WIFEXITED(status) && WEXITSTATUS(status)==0);
    }
    CHECK(send(control[0],"s",1,MSG_NOSIGNAL)==1);unsigned binds=0;
    CHECK(recv(control[0],&binds,sizeof binds,0)==sizeof binds && binds==2);
    CHECK(send(control[0],"x",1,MSG_NOSIGNAL)==1);int status=0;
    CHECK(waitpid(compositor,&status,0)==compositor && WIFEXITED(status) && WEXITSTATUS(status)==0);
    close(control[0]);
    puts("WAYLAND_OWNER_NATIVE_CHECK_PASS kernel-parent-pidfd supervisor-death connected-delegation fd-predicate foreign-registry-guessed-bind idle-rejection rejection-pool32 fd-exhaustion destroy-listener-lifetime no-compositor-or-engine");
    return 0;
}
