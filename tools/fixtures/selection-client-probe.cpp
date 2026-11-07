// Independent adversarial client on the private fixture display; no OS clipboard.
#include <xcb/xcb.h>
#include <poll.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>
int main(int argc,char **argv){
 if(argc!=3){return 2;}
 int screen=0;auto *connection=xcb_connect(argv[1],&screen);if(xcb_connection_has_error(connection)){xcb_disconnect(connection);return strcmp(argv[2],"unauthenticated")==0?0:2;}
 if(strcmp(argv[2],"unauthenticated")==0){xcb_disconnect(connection);return 3;}
 auto screens=xcb_setup_roots_iterator(xcb_get_setup(connection));while(screen--)xcb_screen_next(&screens);const auto window=xcb_generate_id(connection);xcb_create_window(connection,XCB_COPY_FROM_PARENT,window,screens.data->root,0,0,1,1,0,XCB_WINDOW_CLASS_INPUT_OUTPUT,XCB_COPY_FROM_PARENT,0,nullptr);
 auto atom=[&](const char *name){auto *reply=xcb_intern_atom_reply(connection,xcb_intern_atom(connection,0,uint16_t(strlen(name)),name),nullptr);if(!reply)return xcb_atom_t(0);auto result=reply->atom;free(reply);return result;};
 const auto selection=atom("CLIPBOARD"),target=atom("UTF8_STRING"),property=atom("OWNED_PROBE");
 if(strcmp(argv[2],"synthetic")==0){auto *owner=xcb_get_selection_owner_reply(connection,xcb_get_selection_owner(connection,selection),nullptr);if(!owner)return 2;xcb_selection_request_event_t request{};request.response_type=XCB_SELECTION_REQUEST;request.owner=owner->owner;request.requestor=window;request.selection=selection;request.target=target;request.property=property;xcb_send_event(connection,0,owner->owner,XCB_EVENT_MASK_NO_EVENT,reinterpret_cast<char *>(&request));free(owner);}
 else xcb_convert_selection(connection,window,selection,target,property,XCB_CURRENT_TIME);
 xcb_flush(connection);pollfd descriptor{xcb_get_file_descriptor(connection),POLLIN,0};int result=2;
 for(int attempt=0;attempt<30;attempt++){
  while(auto *event=xcb_poll_for_event(connection)){if((event->response_type&0x7f)==XCB_SELECTION_NOTIFY){auto *reply=reinterpret_cast<xcb_selection_notify_event_t *>(event);result=reply->property==XCB_ATOM_NONE?0:3;free(event);xcb_disconnect(connection);printf("{\"rejected\":%s}\n",result==0?"true":"false");return result;}free(event);}
  if(poll(&descriptor,1,100)<0)break;
 }
 xcb_disconnect(connection);return result;
}
