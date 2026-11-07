// The inherited automation socket never becomes a page API. Admit and mark it
// close-on-exec before the main process constructs any guest webContents.
#define _GNU_SOURCE
#include <node_api.h>
#include <fcntl.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
#include <sys/syscall.h>
#include <string.h>

static int confine(napi_env env, napi_callback_info info, int socket_kind) {
  size_t count = 2;
  napi_value args[2];
  int32_t fd = -1, expected_pid = -1;
  double fd_number = -1, pid_number = -1;
  struct ucred peer;
  struct sockaddr_un address;
  socklen_t peer_size = sizeof(peer), address_size = sizeof(address);
  int kind = 0;
  socklen_t kind_size = sizeof(kind);
  if (napi_get_cb_info(env, info, &count, args, NULL, NULL) != napi_ok || count != 2 ||
      napi_get_value_double(env, args[0], &fd_number) != napi_ok ||
      napi_get_value_double(env, args[1], &pid_number) != napi_ok ||
      napi_get_value_int32(env, args[0], &fd) != napi_ok ||
      napi_get_value_int32(env, args[1], &expected_pid) != napi_ok ||
      fd < 64 || expected_pid < 1 || fd_number != fd || pid_number != expected_pid ||
      getsockopt(fd, SOL_SOCKET, SO_TYPE, &kind, &kind_size) || kind != socket_kind ||
      getsockopt(fd, SOL_SOCKET, SO_PEERCRED, &peer, &peer_size) ||
      peer.pid != expected_pid || peer.uid != geteuid() ||
      getsockname(fd, (struct sockaddr *)&address, &address_size) || address.sun_family != AF_UNIX) {
    napi_throw_error(env, "invalid_browser_private_descriptor", "Private browser socket admission failed");
    return -1;
  }
  int flags = fcntl(fd, F_GETFD);
  if (flags < 0 || fcntl(fd, F_SETFD, flags | FD_CLOEXEC) ||
      !(fcntl(fd, F_GETFD) & FD_CLOEXEC)) {
    napi_throw_error(env, "invalid_browser_private_descriptor", "Private browser socket confinement failed");
    return -1;
  }
  return fd;
}
static napi_value admit(napi_env env, napi_callback_info info) {
  if (confine(env, info, SOCK_STREAM) < 0) return NULL;
  napi_value result;napi_get_boolean(env,true,&result);return result;
}
static napi_value identify(napi_env env, napi_callback_info info) {
  int fd=confine(env,info,SOCK_SEQPACKET);if(fd<0)return NULL;
  int pidfd=(int)syscall(SYS_pidfd_open,getpid(),0);
  if(pidfd<0){napi_throw_error(env,"browser_engine_identity_unavailable","Native engine identity unavailable");return NULL;}
  static const char identity[]="o3-native-engine-identity-v1";
  struct iovec payload={(void *)identity,sizeof(identity)-1};
  union {struct cmsghdr alignment;unsigned char bytes[CMSG_SPACE(sizeof(int))];} ancillary;
  memset(&ancillary,0,sizeof ancillary);struct msghdr message;memset(&message,0,sizeof message);
  message.msg_iov=&payload;message.msg_iovlen=1;message.msg_control=ancillary.bytes;message.msg_controllen=sizeof ancillary.bytes;
  struct cmsghdr *header=CMSG_FIRSTHDR(&message);header->cmsg_level=SOL_SOCKET;header->cmsg_type=SCM_RIGHTS;header->cmsg_len=CMSG_LEN(sizeof(int));memcpy(CMSG_DATA(header),&pidfd,sizeof pidfd);
  const ssize_t count=sendmsg(fd,&message,MSG_NOSIGNAL);close(pidfd);close(fd);
  if(count!=(ssize_t)(sizeof(identity)-1)){napi_throw_error(env,"browser_engine_identity_unavailable","Native engine identity unavailable");return NULL;}
  napi_value result;napi_get_boolean(env,true,&result);return result;
}
static napi_value initialize(napi_env env, napi_value exports) {
  napi_value method;
  napi_create_function(env, "admit", NAPI_AUTO_LENGTH, admit, NULL, &method);
  napi_set_named_property(env, exports, "admit", method);
  napi_create_function(env,"identify",NAPI_AUTO_LENGTH,identify,NULL,&method);
  napi_set_named_property(env,exports,"identify",method);
  return exports;
}
NAPI_MODULE(NODE_GYP_MODULE_NAME, initialize)
