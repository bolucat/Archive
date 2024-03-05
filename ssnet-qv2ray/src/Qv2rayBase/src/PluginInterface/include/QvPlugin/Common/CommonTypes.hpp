#pragma once

#include "CommonSafeType.hpp"
#include "QvPlugin/Utils/JsonConversion.hpp"

#include <QHash>
#include <QJsonArray>
#include <QJsonObject>
#include <QString>
#include <chrono>

template<>
struct QJsonStructSerializer<system_clock::time_point>
{
    static void Deserialize(system_clock::time_point &t, const QJsonValue &d)
    {
        t = system_clock::from_time_t(d.toInteger());
    }
    static QJsonValue Serialize(const system_clock::time_point &t)
    {
        return QJsonValue((qint64) system_clock::to_time_t(t));
    }
};

namespace Qv2rayPlugin::Common::_base_types
{
    constexpr auto LATENCY_TEST_VALUE_ERROR = 99999;
    constexpr auto LATENCY_TEST_VALUE_NODATA = LATENCY_TEST_VALUE_ERROR - 1;

    const static inline ConnectionId NullConnectionId;

    const static inline GroupId NullGroupId;
    const static inline GroupId DefaultGroupId{ u"000000000000"_qs };

    const static inline RoutingId NullRoutingId;
    const static inline RoutingId DefaultRoutingId{ u"000000000000"_qs };

    const static inline KernelId NullKernelId;

    struct ProfileId
    {
        ConnectionId connectionId = NullConnectionId;
        GroupId groupId = NullGroupId;

        // clang-format off
        ProfileId(){};
        ProfileId(const ConnectionId &conn, const GroupId &group) { connectionId = conn, groupId = group; }
        inline bool operator==(const ProfileId &rhs) const { return connectionId == rhs.connectionId && groupId == rhs.groupId; }
        inline bool operator!=(const ProfileId &rhs) const { return !(*this == rhs); }
        inline void clear() { connectionId = NullConnectionId, groupId = NullGroupId; }
        inline bool isNull() const { return groupId == NullGroupId || connectionId == NullConnectionId; }
        // clang-format on
        QJS_JSON(F(connectionId, groupId))
    };

    struct StatisticsObject
    {
        enum StatisticsType
        {
            DIRECT,
            PROXY,
            ALL
        };

        quint64 directUp = 0;
        quint64 directDown = 0;
        quint64 proxyUp = 0;
        quint64 proxyDown = 0;
        void clear()
        {
            directUp = 0;
            directDown = 0;
            proxyUp = 0;
            proxyDown = 0;
        }
        QJS_JSON(F(directUp, directDown, proxyUp, proxyDown))
    };

    struct BaseTaggedObject
    {
        QString name;
        QJsonObject options;
        QJS_JSON(F(name, options))
    };

    struct BaseConfigTaggedObject : public BaseTaggedObject
    {
        system_clock::time_point created = system_clock::now();
        system_clock::time_point updated = system_clock::now();
        QJS_JSON(F(created, updated), B(BaseTaggedObject))
    };

    struct ConnectionObject : public BaseConfigTaggedObject
    {
        system_clock::time_point last_connected;
        QSet<QString> tags;
        StatisticsObject statistics;
        int latency = LATENCY_TEST_VALUE_NODATA;
        int _group_ref = 0;
        QJS_JSON(F(last_connected, tags, statistics, latency), B(BaseConfigTaggedObject))
    };

    struct SubscriptionConfigObject : public BaseTaggedObject
    {
        enum FilterRelation
        {
            RELATION_AND = 0,
            RELATION_OR = 1
        };
        bool isSubscription;

        QString address;
        SubscriptionProviderId providerId{};
        SubscriptionProviderOptions providerSettings{};

        float updateInterval = 10;

        QList<QString> includeKeywords;
        FilterRelation includeRelation = RELATION_OR;

        QList<QString> excludeKeywords;
        FilterRelation excludeRelation = RELATION_AND;

        QJS_JSON(F(isSubscription, address, providerId, providerSettings, updateInterval, includeKeywords, excludeKeywords, includeRelation, excludeRelation),
                 B(BaseTaggedObject))
    };

    struct GroupObject : public BaseConfigTaggedObject
    {
        QList<ConnectionId> connections;
        RoutingId route_id = NullRoutingId;
        SubscriptionConfigObject subscription_config;
        QJS_JSON(F(connections, route_id, subscription_config), B(BaseConfigTaggedObject))
    };

    struct PortRange
    {
        int from = 0, to = 0;

        // clang-format off
        PortRange() = default;
        PortRange(int i) : from(i), to(i) {};
        void operator=(const int i) { from = to = i; }
        bool operator==(const PortRange &another) const { return from == another.from && to == another.to; }
        operator QString() const { return from == to ? QString::number(from) : QString::number(from) +  u'-' + QString::number(to); }
        // clang-format on

        QJS_JSON(F(from, to))
    };

    struct RuleObject : public BaseTaggedObject
    {
        bool enabled = true;

        QStringList inboundTags;
        QString outboundTag;

        QStringList sourceAddresses;
        QStringList targetDomains;
        QStringList targetIPs;

        PortRange sourcePort;
        PortRange targetPort;

        QStringList networks;
        QStringList protocols;

        QStringList processes;

        RuleExtraSettings extraSettings{};
        QJS_JSON(F(enabled, inboundTags, outboundTag, sourceAddresses, targetDomains, targetIPs, sourcePort, targetPort, networks, protocols, processes, extraSettings))
    };

    struct RoutingObject
    {
        bool overrideRules = false;
        QList<RuleObject> rules;

        bool overrideDNS = false;
        QJsonObject dns;
        QJsonObject fakedns;

        QJsonObject extraOptions;
        QJS_JSON(F(overrideRules, rules, overrideDNS, dns, fakedns, extraOptions))
    };

    struct MultiplexerObject
    {
        bool enabled = false;
        int concurrency = 8;
        QJS_JSON(F(enabled, concurrency))
    };

    struct IOConnectionSettings
    {
        QString protocol;
        QString address;
        PortRange port;
        IOProtocolSettings protocolSettings{};
        IOStreamSettings streamSettings{};
        MultiplexerObject muxSettings{};
        QJS_JSON(F(protocol, address, port, protocolSettings, streamSettings, muxSettings))
    };

    struct InboundObject : public BaseTaggedObject
    {
        IOConnectionSettings inboundSettings;
        static InboundObject Create(QString name, QString proto, QString addr, int port, IOProtocolSettings protocol = IOProtocolSettings{},
                                    IOStreamSettings stream = IOStreamSettings{})
        {
            InboundObject in;
            in.name = name;
            in.inboundSettings.address = addr;
            in.inboundSettings.port = port;
            in.inboundSettings.protocol = proto;
            in.inboundSettings.protocolSettings = protocol;
            in.inboundSettings.streamSettings = stream;
            return in;
        }
        QJS_JSON(F(inboundSettings), B(BaseTaggedObject))
    };

    struct BalancerSettings : public BaseTaggedObject
    {
        QString selectorType;
        BalancerSelectorSettings selectorSettings{};
        QJS_JSON(F(selectorType, selectorSettings))
    };

    struct ChainSettings : public BaseTaggedObject
    {
        int chaining_port = 15490;
        QStringList chains;
        QJS_JSON(F(chains, chaining_port), B(BaseTaggedObject))
    };

    struct OutboundObject : public BaseTaggedObject
    {
        enum OutboundObjectType
        {
            ORIGINAL,
            EXTERNAL,
            BALANCER,
            CHAIN
        };

        explicit OutboundObject(){};
        OutboundObject(const IOConnectionSettings &settings) : objectType(ORIGINAL), outboundSettings(settings){};
        OutboundObject(const ConnectionId &external) : objectType(EXTERNAL), externalId(external){};
        OutboundObject(const BalancerSettings &balancer) : objectType(BALANCER), balancerSettings(balancer){};
        OutboundObject(const ChainSettings &chain) : objectType(CHAIN), chainSettings(chain){};

        OutboundObjectType objectType = ORIGINAL;

        KernelId kernel = NullKernelId;

        ConnectionId externalId = NullConnectionId;
        IOConnectionSettings outboundSettings;
        BalancerSettings balancerSettings;
        ChainSettings chainSettings;

        QJS_JSON(F(objectType, kernel, externalId, outboundSettings, balancerSettings, chainSettings), B(BaseTaggedObject))
    };

    struct BasicDNSServerObject
    {
        QString address;
        int port = 53;
        QJS_JSON(F(address, port))
    };

    struct BasicDNSObject
    {
        QList<BasicDNSServerObject> servers;
        QMap<QString, QString> hosts;
        QJsonObject extraOptions;
        QJS_JSON(F(servers, hosts, extraOptions))
    };

    struct ProfileContent
    {
        ProfileContent(){};
        explicit ProfileContent(const OutboundObject &out)
        {
            outbounds << out;
        };
        KernelId defaultKernel = NullKernelId;
        QList<InboundObject> inbounds;
        QList<OutboundObject> outbounds;
        RoutingObject routing;

        QJsonObject extraOptions;

        static auto fromJson(const QJsonObject &o)
        {
            ProfileContent profile;
            profile.loadJson(o);
            return profile;
        };
        QJS_JSON(F(defaultKernel, inbounds, outbounds, routing, extraOptions))
    };

    enum IOBOUND_DATA_TYPE
    {
        // IO_DISPLAYNAME Q_DECL_DEPRECATED = 0,
        // IO_PROTOCOL Q_DECL_DEPRECATED = 1,
        // IO_ADDRESS Q_DECL_DEPRECATED = 2,
        // IO_PORT Q_DECL_DEPRECATED = 3,
        IO_SNI = 4
    };
    typedef QMap<IOBOUND_DATA_TYPE, QVariant> PluginIOBoundData;

    inline size_t qHash(const ProfileId &c, size_t seed) noexcept
    {
        return qHashMulti(seed, c.connectionId, c.groupId);
    }
    ///
    /// \brief IOBoundData <Protocol, Address, Port>
    ///
    typedef std::tuple<QString, QString, PortRange> IOBoundData;

    inline size_t qHash(const IOBoundData &c, size_t seed) noexcept
    {
        return qHashMulti(seed, std::get<0>(c), std::get<1>(c), std::get<2>(c).from, std::get<2>(c).to);
    }
} // namespace Qv2rayPlugin::Common::_base_types

// Expose all basic type decls to global namespace
using namespace Qv2rayPlugin::Common::_base_types;

Q_DECLARE_METATYPE(ProfileId)
Q_DECLARE_METATYPE(StatisticsObject)
Q_DECLARE_METATYPE(ConnectionObject)
Q_DECLARE_METATYPE(GroupObject)
Q_DECLARE_METATYPE(RoutingObject)
Q_DECLARE_METATYPE(ChainSettings)
Q_DECLARE_METATYPE(IOConnectionSettings)
Q_DECLARE_METATYPE(BalancerSettings)
Q_DECLARE_METATYPE(OutboundObject)
Q_DECLARE_METATYPE(ProfileContent)
