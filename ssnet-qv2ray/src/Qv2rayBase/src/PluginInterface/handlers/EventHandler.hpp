#pragma once
#include <QMap>

namespace Qv2rayPlugin::handlers::event
{
    struct SystemProxy
    {
        enum SystemProxyStateType
        {
            SetProxy,
            ClearProxy
        };
        enum SystemProxyType
        {
            SystemProxy_HTTP,
            SystemProxy_SOCKS
        };
        struct EventObject
        {
            QMap<SystemProxyType, int> PortSettings;
            SystemProxyStateType State;
            EventObject(){};
            EventObject(const QMap<SystemProxyType, int> &port, const SystemProxyStateType &type) : PortSettings(port), State(type){};
        };
    };

    struct ConnectionStats
    {
        struct EventObject
        {
            QString DisplayName;
            long UpLinkSpeed;
            long DownLinkSpeed;
            quint64 UpLinkTotalData;
            quint64 DownLinkTotalData;
        };
    };

    struct Connectivity
    {
        enum EventType
        {
            Connecting,
            Connected,
            Disconnecting,
            Disconnected
        };
        struct EventObject
        {
            EventType Type;
            QString DisplayName;
            QMap<QString, int> InboundPorts;
            EventObject(){};
            EventObject(const QString &name, const QMap<QString, int> &ports, const EventType &event) : Type(event), DisplayName(name), InboundPorts(ports){};
        };
    };

    struct ConnectionEntry
    {
        enum EventType
        {
            Created,
            Edited,
            Renamed,
            LinkedWithGroup,
            RemovedFromGroup,
            FullyRemoved
        };
        struct EventObject
        {
            EventType Type;
            QString CurrentName;
            QString OriginalName;
        };
    };

    namespace _details
    {
        template<typename... T>
        class Qp
        {
          public:
            void ProcessEvent(){};
        };

        template<typename T1, typename... T2>
        class Qp<T1, T2...> : public Qp<T2...>
        {
          public:
            using Qp<T2...>::ProcessEvent;
            virtual void ProcessEvent(const T1 &pluginEvent){ Q_UNUSED(pluginEvent) };
        };
    } // namespace _details

    class PluginEventHandler : public _details::Qp<Connectivity::EventObject, SystemProxy::EventObject, ConnectionEntry::EventObject, ConnectionStats::EventObject>
    {
    };
} // namespace Qv2rayPlugin::handlers::event

using namespace Qv2rayPlugin::handlers::event;
