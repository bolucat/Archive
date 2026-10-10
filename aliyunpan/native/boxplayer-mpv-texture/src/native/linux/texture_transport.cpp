// This addon deliberately does not link libmpv/FFmpeg. Electron can safely load it.
#include <napi.h>
#include <uv.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/stat.h>
#include <unistd.h>
#include <fcntl.h>
#include <cstring>
#include <vector>

#ifndef MSG_CMSG_CLOEXEC
#define MSG_CMSG_CLOEXEC 0
#endif
#ifndef MSG_NOSIGNAL
#define MSG_NOSIGNAL 0
#endif

namespace {
int openSocket() {
    int fd = socket(AF_UNIX, SOCK_DGRAM, 0);
    if (fd >= 0) {
        fcntl(fd, F_SETFL, O_NONBLOCK);
        fcntl(fd, F_SETFD, FD_CLOEXEC);
    }
    return fd;
}
constexpr size_t MAX_FDS = 4;
constexpr size_t MAX_MESSAGE = 4096;

bool addressFor(const std::string& path, sockaddr_un& address) {
    if (path.empty() || path.size() >= sizeof(address.sun_path)) return false;
    address.sun_family = AF_UNIX;
    std::memcpy(address.sun_path, path.c_str(), path.size() + 1);
    return true;
}

class Receiver : public Napi::ObjectWrap<Receiver> {
public:
    static Napi::Function constructor(Napi::Env env) {
        return DefineClass(env, "TextureReceiver", {InstanceMethod("stop", &Receiver::stop)});
    }
    explicit Receiver(const Napi::CallbackInfo& info) : Napi::ObjectWrap<Receiver>(info) {
        if (info.Length() < 2 || !info[0].IsString() || !info[1].IsFunction()) {
            Napi::TypeError::New(info.Env(), "socket path and callback required").ThrowAsJavaScriptException(); return;
        }
        path = info[0].As<Napi::String>().Utf8Value();
        sockaddr_un address{};
        if (!addressFor(path, address)) { fail(info.Env(), "Invalid texture socket path"); return; }
        fd = openSocket();
        if (fd < 0 || bind(fd, reinterpret_cast<sockaddr*>(&address), sizeof(address)) < 0) {
            if (fd >= 0) close(fd);
            fd = -1;
            fail(info.Env(), "Cannot bind texture socket"); return;
        }
        chmod(path.c_str(), 0600);
        uv_loop_t* loop = nullptr;
        napi_get_uv_event_loop(info.Env(), &loop);
        if (uv_poll_init(loop, &poll, fd) != 0) {
            close(fd); fd = -1; unlink(path.c_str());
            fail(info.Env(), "Cannot poll texture socket"); return;
        }
        callback = Napi::Persistent(info[1].As<Napi::Function>());
        poll.data = this;
        Ref();
        uv_poll_start(&poll, UV_READABLE, [](uv_poll_t* handle, int status, int) {
            auto* self = static_cast<Receiver*>(handle->data);
            if (status >= 0) self->receive();
        });
    }
private:
    uv_poll_t poll{};
    int fd = -1;
    bool stopped = false;
    std::string path;
    Napi::FunctionReference callback;
    static void fail(Napi::Env env, const char* message) { Napi::Error::New(env, message).ThrowAsJavaScriptException(); }
    void receive() {
        // Bound a callback's work so a busy producer cannot starve Electron's event loop.
        for (int batch = 0; batch < 4; ++batch) {
            char bytes[MAX_MESSAGE];
            alignas(cmsghdr) char control[CMSG_SPACE(sizeof(int) * MAX_FDS)]{};
            iovec vector{bytes, sizeof(bytes)};
            msghdr message{};
            message.msg_iov = &vector; message.msg_iovlen = 1;
            message.msg_control = control; message.msg_controllen = sizeof(control);
            ssize_t count = recvmsg(fd, &message, MSG_DONTWAIT | MSG_CMSG_CLOEXEC);
            if (count < 0) return;
            std::vector<int> descriptors;
            for (auto* header = CMSG_FIRSTHDR(&message); header; header = CMSG_NXTHDR(&message, header)) {
                if (header->cmsg_level != SOL_SOCKET || header->cmsg_type != SCM_RIGHTS) continue;
                auto* values = reinterpret_cast<int*>(CMSG_DATA(header));
                size_t length = (header->cmsg_len - CMSG_LEN(0)) / sizeof(int);
                descriptors.insert(descriptors.end(), values, values + length);
            }
            if ((message.msg_flags & (MSG_TRUNC | MSG_CTRUNC)) || descriptors.empty()) {
                for (int descriptor : descriptors) close(descriptor);
                continue;
            }
            for (int descriptor : descriptors) fcntl(descriptor, F_SETFD, FD_CLOEXEC);
            Napi::HandleScope scope(Env());
            auto array = Napi::Array::New(Env(), descriptors.size());
            for (size_t i = 0; i < descriptors.size(); ++i) array.Set(i, descriptors[i]);
            callback.Call({Napi::String::New(Env(), bytes, count), array});
            // JS callback always takes ownership, including malformed metadata/error paths.
            if (Env().IsExceptionPending()) return;
        }
    }
    void stop(const Napi::CallbackInfo&) {
        if (stopped || fd < 0) return;
        stopped = true;
        uv_poll_stop(&poll);
        unlink(path.c_str());
        uv_close(reinterpret_cast<uv_handle_t*>(&poll), [](uv_handle_t* handle) {
            auto* self = static_cast<Receiver*>(handle->data);
            close(self->fd); self->fd = -1;
            self->callback.Reset();
            self->Unref();
        });
    }
};

Napi::Value send(const Napi::CallbackInfo& info) {
    auto env = info.Env();
    if (info.Length() < 3 || !info[0].IsString() || !info[1].IsString() || !info[2].IsArray()) return Napi::Boolean::New(env, false);
    sockaddr_un address{};
    auto payload = info[1].As<Napi::String>().Utf8Value();
    auto array = info[2].As<Napi::Array>();
    if (!addressFor(info[0].As<Napi::String>().Utf8Value(), address) || payload.size() > MAX_MESSAGE || !array.Length() || array.Length() > MAX_FDS) return Napi::Boolean::New(env, false);
    std::vector<int> descriptors;
    for (uint32_t index = 0; index < array.Length(); ++index) descriptors.push_back(array.Get(index).As<Napi::Number>().Int32Value());
    int fd = openSocket();
    if (fd < 0) return Napi::Boolean::New(env, false);
    alignas(cmsghdr) char control[CMSG_SPACE(sizeof(int) * MAX_FDS)]{};
    iovec vector{payload.data(), payload.size()};
    msghdr message{};
    message.msg_name = &address; message.msg_namelen = sizeof(address);
    message.msg_iov = &vector; message.msg_iovlen = 1;
    message.msg_control = control; message.msg_controllen = CMSG_SPACE(descriptors.size() * sizeof(int));
    auto* header = CMSG_FIRSTHDR(&message);
    header->cmsg_level = SOL_SOCKET; header->cmsg_type = SCM_RIGHTS;
    header->cmsg_len = CMSG_LEN(descriptors.size() * sizeof(int));
    memcpy(CMSG_DATA(header), descriptors.data(), descriptors.size() * sizeof(int));
    bool sent = sendmsg(fd, &message, MSG_DONTWAIT | MSG_NOSIGNAL) == static_cast<ssize_t>(payload.size());
    close(fd);
    return Napi::Boolean::New(env, sent);
}

Napi::Object init(Napi::Env env, Napi::Object exports) {
    exports.Set("Receiver", Receiver::constructor(env));
    exports.Set("send", Napi::Function::New(env, send));
    exports.Set("closeDescriptors", Napi::Function::New(env, [](const Napi::CallbackInfo& info) {
        auto array = info[0].As<Napi::Array>();
        for (uint32_t index = 0; index < array.Length(); ++index) close(array.Get(index).As<Napi::Number>().Int32Value());
    }));
    return exports;
}
}
NODE_API_MODULE(mpv_transport, init)
