#pragma once

#include "QvPlugin/Common/CommonTypes.hpp"
#include "QvPlugin/Common/EditorCreatorDefs.hpp"

#include <QJsonObject>
#include <QList>
#include <QVariantMap>

Q_DECLARE_METATYPE(std::chrono::system_clock::time_point)

namespace Qv2rayPlugin::Subscription
{
    enum SubscriptionResultFields
    {
        /// \brief A QStringList containing shared links
        SR_Links = 0,
        /// \brief A QList<OutboundObjecct> containing outbounds
        SR_OutboundObjects = 1,
        /// \brief A QMultiMap<QString, ProfileContent> containing ProfileContents
        SR_ProfileContents = 2,
        /// \brief A QMap<QString, QStringList>
        SR_Tags = 3,
        /// \brief A RoutingObject
        SR_GroupRoutingObject = 4,
        /// \brief A std::chrono::system_clock::time_point object indicating when the subscription expires
        SR_Expires = 5,
    };

    using SubscriptionResult = EnumVariantMap<SubscriptionResultFields, std::tuple<QStringList,                          // Links
                                                                                   QMultiMap<QString, OutboundObject>,   // Outbounds
                                                                                   QMultiMap<QString, ProfileContent>,   // ProfileContents
                                                                                   QMap<QString, QStringList>,           // Tags
                                                                                   RoutingObject,                        // RoutingObject
                                                                                   std::chrono::system_clock::time_point // Expires
                                                                                   >>;

    class SubscriptionProvider
    {
      public:
        virtual ~SubscriptionProvider() = default;
        virtual SubscriptionResult DecodeSubscription(const QByteArray &data) const
        {
            Q_UNUSED(data);
            Q_UNREACHABLE();
        }
        virtual SubscriptionResult FetchDecodeSubscription(const SubscriptionProviderOptions &options) const
        {
            Q_UNUSED(options);
            Q_UNREACHABLE();
        }
    };

    enum SubscribingMode
    {
        Subscribe_Decoder,
        Subscribe_FetcherAndDecoder,
    };

    struct SubscriptionProviderInfo
    {
        SubscriptionProviderId id;
        SubscribingMode mode;
        QString displayName;
        Common::EditorCreator::EditorInfoList settings;
        std::function<std::unique_ptr<SubscriptionProvider>(void)> Creator;

        template<typename T>
        static SubscriptionProviderInfo CreateDecoder(const SubscriptionProviderId &id, const QString &name)
        {
            SubscriptionProviderInfo info;
            info.id = id;
            info.displayName = name;
            info.Creator = []() { return std::make_unique<T>(); };
            info.mode = Subscribe_Decoder;
            return info;
        }

        template<typename T>
        static SubscriptionProviderInfo CreateFetcherDecoder(const SubscriptionProviderId &id, const QString &name, const Common::EditorCreator::EditorInfoList &settings)
        {
            SubscriptionProviderInfo info;
            info.id = id;
            info.displayName = name;
            info.settings = settings;
            info.Creator = []() { return std::make_unique<T>(); };
            info.mode = Subscribe_FetcherAndDecoder;
            return info;
        }
    };

    class IPluginSubscriptionInterface
    {
      public:
        virtual QList<SubscriptionProviderInfo> GetInfo() const = 0;
        virtual ~IPluginSubscriptionInterface() {};
    };
} // namespace Qv2rayPlugin::Subscription
