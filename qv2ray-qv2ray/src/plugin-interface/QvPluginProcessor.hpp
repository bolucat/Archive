#pragma once

#include "QvPluginInterfaceModels.hpp"

#include <QJsonObject>
#include <QObject>

#define __QVPLUGIN_EVENT_HANDLER_SIG(type) const ::Qv2rayPlugin::Events::type::EventObject &pluginEvent
#define __QVPLUGIN_DECL_EVENT_HANDLER(type) void ProcessEvent_##type(__QVPLUGIN_EVENT_HANDLER_SIG(type))

#define QvPlugin_EventHandler_Decl(type) __QVPLUGIN_DECL_EVENT_HANDLER(type) override
#define QvPlugin_EventHandler(className, type) void className::ProcessEvent_##type(__QVPLUGIN_EVENT_HANDLER_SIG(type))

namespace Qv2rayPlugin
{
    class PluginOutboundHandler
    {
      public:
        explicit PluginOutboundHandler(){};
        virtual const QString SerializeOutbound(const QString &protocol,   //
                                                const QString &alias,      //
                                                const QString &groupName,  //
                                                const QJsonObject &object, //
                                                const QJsonObject &streamSettings) const = 0;
        virtual const QPair<QString, QJsonObject> DeserializeOutbound(const QString &link, QString *alias, QString *errorMessage) const = 0;
        virtual const OutboundInfoObject GetOutboundInfo(const QString &protocol, const QJsonObject &outbound) const = 0;
        virtual const void SetOutboundInfo(const QString &protocol, const OutboundInfoObject &info, QJsonObject &outbound) const = 0;
        virtual const QList<QString> SupportedProtocols() const = 0;
        virtual const QList<QString> SupportedLinkPrefixes() const = 0;
    };

    // Subscription Adapter fetches data from an online service.
    class SubscriptionDecoder
    {
      public:
        struct SubscriptionDecodeResult
        {
            QList<QString> links;
            QList<QPair<QString, QJsonObject>> connections;
        };
        virtual ~SubscriptionDecoder(){};
        virtual SubscriptionDecodeResult DecodeData(const QByteArray &) const = 0;
    };

    class SubscriptionInterface
    {
      public:
        virtual QList<ProtocolInfoObject> SupportedSubscriptionTypes() const = 0;
        virtual std::shared_ptr<SubscriptionDecoder> GetSubscriptionDecoder(const QString &type) const = 0;
    };

    class PluginKernel : public QObject
    {
        Q_OBJECT
      public:
        explicit PluginKernel() : QObject(){};
        virtual ~PluginKernel(){};
        virtual void SetConnectionSettings(const QMap<KernelOptionFlags, QVariant> &settings, const QJsonObject &connectionInfo) = 0;
        virtual bool StartKernel() = 0;
        virtual bool StopKernel() = 0;
        virtual QString GetKernelName() const = 0;
        //
      signals:
        void OnKernelCrashed(const QString &);
        void OnKernelLogAvailable(const QString &);
        void OnKernelStatsAvailable(quint64 upSpeed, quint64 downSpeed);

      private:
        QString __qvKernelId;
    };

    class PluginKernelInterface
    {
      public:
        virtual std::unique_ptr<PluginKernel> CreateKernel() const = 0;
        virtual QList<QString> GetKernelProtocols() const = 0;
    };

    class PluginEventHandler
    {
      public:
        explicit PluginEventHandler(){};
        virtual __QVPLUGIN_DECL_EVENT_HANDLER(ConnectionStats){ Q_UNUSED(pluginEvent) };
        virtual __QVPLUGIN_DECL_EVENT_HANDLER(SystemProxy){ Q_UNUSED(pluginEvent) };
        virtual __QVPLUGIN_DECL_EVENT_HANDLER(Connectivity){ Q_UNUSED(pluginEvent) };
        virtual __QVPLUGIN_DECL_EVENT_HANDLER(ConnectionEntry){ Q_UNUSED(pluginEvent) };
    };
} // namespace Qv2rayPlugin
