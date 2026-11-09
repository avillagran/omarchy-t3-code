#define _POSIX_C_SOURCE 200809L
// Real input only in the owned private compositor fixture; never the desktop.
#include "virtual-pointer.h"
#include <wayland-client.h>
#include <linux/input-event-codes.h>
#include <sys/stat.h>
#include <time.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
static struct zwlr_virtual_pointer_manager_v1 *manager;
static void global(void *data,struct wl_registry *registry,uint32_t name,const char *interface,uint32_t version){(void)data;(void)version;if(!strcmp(interface,zwlr_virtual_pointer_manager_v1_interface.name))manager=wl_registry_bind(registry,name,&zwlr_virtual_pointer_manager_v1_interface,1);}
static void removed(void *data,struct wl_registry *registry,uint32_t name){(void)data;(void)registry;(void)name;}
static uint32_t now(void){struct timespec value;clock_gettime(CLOCK_MONOTONIC,&value);return (uint32_t)(value.tv_sec*1000+value.tv_nsec/1000000);}
int main(void){
 const char *enabled=getenv("O3_PRIVATE_INPUT_FIXTURE"),*runtime=getenv("XDG_RUNTIME_DIR"),*socket=getenv("WAYLAND_DISPLAY");struct stat directory;
 if(!enabled||strcmp(enabled,"1")||!runtime||strncmp(runtime,"/tmp/",5)||lstat(runtime,&directory)||!S_ISDIR(directory.st_mode)||directory.st_uid!=getuid()||(directory.st_mode&0777)!=0700||!socket||strchr(socket,'/'))return 2;
 char path[512];if(snprintf(path,sizeof path,"%s/%s",runtime,socket)>=(int)sizeof path||lstat(path,&directory)||!S_ISSOCK(directory.st_mode))return 2;
 struct wl_display *display=wl_display_connect(NULL);if(!display)return 2;struct wl_registry *registry=wl_display_get_registry(display);const struct wl_registry_listener listener={global,removed};wl_registry_add_listener(registry,&listener,NULL);if(wl_display_roundtrip(display)<0||!manager)return 2;
 struct zwlr_virtual_pointer_v1 *pointer=zwlr_virtual_pointer_manager_v1_create_virtual_pointer(manager,NULL);if(wl_display_roundtrip(display)<0)return 2;puts("ready");fflush(stdout);
 char line[256];while(fgets(line,sizeof line,stdin)){unsigned x,y;if(sscanf(line,"move %u %u",&x,&y)==2){if(x>1280||y>1024)return 2;zwlr_virtual_pointer_v1_motion_absolute(pointer,now(),x,y,1280,1024);}else if(!strcmp(line,"press\n"))zwlr_virtual_pointer_v1_button(pointer,now(),BTN_LEFT,WL_POINTER_BUTTON_STATE_PRESSED);else if(!strcmp(line,"release\n"))zwlr_virtual_pointer_v1_button(pointer,now(),BTN_LEFT,WL_POINTER_BUTTON_STATE_RELEASED);else if(!strcmp(line,"quit\n"))break;else return 2;zwlr_virtual_pointer_v1_frame(pointer);if(wl_display_roundtrip(display)<0)return 2;puts("ok");fflush(stdout);}
 zwlr_virtual_pointer_v1_destroy(pointer);zwlr_virtual_pointer_manager_v1_destroy(manager);wl_registry_destroy(registry);wl_display_disconnect(display);return 0;
}
