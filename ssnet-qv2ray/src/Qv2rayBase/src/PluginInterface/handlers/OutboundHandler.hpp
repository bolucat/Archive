#pragma once

#include "../connections/ConnectionsBaseTypes.hpp"

#include <QJsonObject>
#include <QMap>
#include <optional>

namespace Qv2rayPlugin::handlers::outbound
{
    struct PluginOutboundDescriptor
    {
        QString ConnectionName;
        QString Protocol;
        QJsonObject Outbound;
        QJsonObject StreamSettings;
    };

    class PluginOutboundHandler
    {
      public:
        explicit PluginOutboundHandler(){};
        virtual std::optional<QString> Serialize(const PluginOutboundDescriptor &outbound) const = 0;
        virtual std::optional<PluginOutboundDescriptor> Deserialize(const QString &link) const = 0;

        virtual std::optional<PluginIOBoundData> GetOutboundInfo(const QString &protocol, const QJsonObject &outbound) const = 0;
        virtual bool SetOutboundInfo(const QString &protocol, QJsonObject &outbound, const PluginIOBoundData &info) const = 0;

        virtual QList<QString> SupportedProtocols() const = 0;
        virtual QList<QString> SupportedLinkPrefixes() const = 0;
    };
} // namespace Qv2rayPlugin::handlers::outbound

using namespace ::Qv2rayPlugin::handlers::outbound;
