#ifdef UVW_AS_LIB
#    include "dns.h"
#endif

#include "config.h"

namespace uvw {

UVW_INLINE AddrInfoEvent::AddrInfoEvent(std::unique_ptr<addrinfo, Deleter> addr)
    : data{std::move(addr)} {}

UVW_INLINE NameInfoEvent::NameInfoEvent(const char *host, const char *serv)
    : hostname{host}, service{serv} {}

UVW_INLINE void GetAddrInfoReq::addrInfoCallback(uv_getaddrinfo_t *req, int status, addrinfo *res) {
    if(auto ptr = reserve(req); status) {
        ptr->publish(ErrorEvent{status});
    } else {
        auto data = std::unique_ptr<addrinfo, void (*)(addrinfo *)>{res, [](addrinfo *addr) { uv_freeaddrinfo(addr); }};
        ptr->publish(AddrInfoEvent{std::move(data)});
    }
}

UVW_INLINE void GetAddrInfoReq::nodeAddrInfo(const char *node, const char *service, addrinfo *hints) {
    invoke(&uv_getaddrinfo, parent(), get(), &addrInfoCallback, node, service, hints);
}

UVW_INLINE auto GetAddrInfoReq::nodeAddrInfoSync(const char *node, const char *service, addrinfo *hints) {
    auto req = get();
    auto err = uv_getaddrinfo(parent(), req, nullptr, node, service, hints);
    auto data = std::unique_ptr<addrinfo, void (*)(addrinfo *)>{req->addrinfo, [](addrinfo *addr) { uv_freeaddrinfo(addr); }};
    return std::make_pair(!err, std::move(data));
}

UVW_INLINE void GetAddrInfoReq::nodeAddrInfo(const std::string &node, addrinfo *hints) {
    nodeAddrInfo(node.data(), nullptr, hints);
}

UVW_INLINE std::pair<bool, std::unique_ptr<addrinfo, GetAddrInfoReq::Deleter>> GetAddrInfoReq::nodeAddrInfoSync(const std::string &node, addrinfo *hints) {
    return nodeAddrInfoSync(node.data(), nullptr, hints);
}

UVW_INLINE void GetAddrInfoReq::serviceAddrInfo(const std::string &service, addrinfo *hints) {
    nodeAddrInfo(nullptr, service.data(), hints);
}

UVW_INLINE std::pair<bool, std::unique_ptr<addrinfo, GetAddrInfoReq::Deleter>> GetAddrInfoReq::serviceAddrInfoSync(const std::string &service, addrinfo *hints) {
    return nodeAddrInfoSync(nullptr, service.data(), hints);
}

UVW_INLINE void GetAddrInfoReq::addrInfo(const std::string &node, const std::string &service, addrinfo *hints) {
    nodeAddrInfo(node.data(), service.data(), hints);
}

UVW_INLINE std::pair<bool, std::unique_ptr<addrinfo, GetAddrInfoReq::Deleter>> GetAddrInfoReq::addrInfoSync(const std::string &node, const std::string &service, addrinfo *hints) {
    return nodeAddrInfoSync(node.data(), service.data(), hints);
}

UVW_INLINE void GetNameInfoReq::nameInfoCallback(uv_getnameinfo_t *req, int status, const char *hostname, const char *service) {
    if(auto ptr = reserve(req); status) {
        ptr->publish(ErrorEvent{status});
    } else {
        ptr->publish(NameInfoEvent{hostname, service});
    }
}

UVW_INLINE void GetNameInfoReq::nameInfo(const sockaddr &addr, int flags) {
    invoke(&uv_getnameinfo, parent(), get(), &nameInfoCallback, &addr, flags);
}

template<typename I>
UVW_INLINE void GetNameInfoReq::nameInfo(const std::string &ip, unsigned int port, int flags) {
    typename details::IpTraits<I>::Type addr;
    details::IpTraits<I>::addrFunc(ip.data(), port, &addr);
    nameInfo(reinterpret_cast<const sockaddr &>(addr), flags);
}

template<typename I>
UVW_INLINE void GetNameInfoReq::nameInfo(Addr addr, int flags) {
    nameInfo<I>(std::move(addr.ip), addr.port, flags);
}

UVW_INLINE std::pair<bool, std::pair<const char *, const char *>> GetNameInfoReq::nameInfoSync(const sockaddr &addr, int flags) {
    auto req = get();
    auto err = uv_getnameinfo(parent(), req, nullptr, &addr, flags);
    return std::make_pair(!err, std::make_pair(req->host, req->service));
}

template<typename I>
UVW_INLINE std::pair<bool, std::pair<const char *, const char *>> GetNameInfoReq::nameInfoSync(const std::string &ip, unsigned int port, int flags) {
    typename details::IpTraits<I>::Type addr;
    details::IpTraits<I>::addrFunc(ip.data(), port, &addr);
    return nameInfoSync(reinterpret_cast<const sockaddr &>(addr), flags);
}

template<typename I>
UVW_INLINE std::pair<bool, std::pair<const char *, const char *>> GetNameInfoReq::nameInfoSync(Addr addr, int flags) {
    return nameInfoSync<I>(std::move(addr.ip), addr.port, flags);
}

// explicit instantiations
#ifdef UVW_AS_LIB
template void GetNameInfoReq::nameInfo<IPv4>(const std::string &, unsigned int, int);
template void GetNameInfoReq::nameInfo<IPv6>(const std::string &, unsigned int, int);

template void GetNameInfoReq::nameInfo<IPv4>(Addr, int);
template void GetNameInfoReq::nameInfo<IPv6>(Addr, int);

template std::pair<bool, std::pair<const char *, const char *>> GetNameInfoReq::nameInfoSync<IPv4>(const std::string &, unsigned int, int);
template std::pair<bool, std::pair<const char *, const char *>> GetNameInfoReq::nameInfoSync<IPv6>(const std::string &, unsigned int, int);

template std::pair<bool, std::pair<const char *, const char *>> GetNameInfoReq::nameInfoSync<IPv4>(Addr, int);
template std::pair<bool, std::pair<const char *, const char *>> GetNameInfoReq::nameInfoSync<IPv6>(Addr, int);
#endif // UVW_AS_LIB

} // namespace uvw
