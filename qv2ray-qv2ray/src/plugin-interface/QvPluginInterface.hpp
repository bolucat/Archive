#pragma once

#include "QvPluginBase.hpp"
#include "QvPluginProcessor.hpp"

namespace Qv2rayPlugin
{
    class PluginGUIInterface;
    class QvPluginEditor;
    class QvPluginSettingsWidget;
    class PluginKernelInterface;
    class Qv2rayInterface;

    class Qv2rayInterface
    {
        friend class PluginOutboundHandler;
        friend class PluginKernel;
        friend class PluginEventHandler;
        friend class PluginGUIInterface;

      public:
        const int QvPluginInterfaceVersion = QV2RAY_PLUGIN_INTERFACE_VERSION;

        virtual ~Qv2rayInterface() = default;
        virtual const QvPluginMetadata GetMetadata() const = 0;
        virtual bool InitializePlugin(const QString &, const QJsonObject &) = 0;
        //
        virtual std::shared_ptr<PluginOutboundHandler> GetOutboundHandler() const final
        {
            return outboundHandler;
        }
        virtual std::shared_ptr<PluginEventHandler> GetEventHandler() const final
        {
            return eventHandler;
        }
        virtual std::shared_ptr<PluginKernelInterface> GetKernel() const final
        {
            return kernelInterface;
        }
        virtual std::shared_ptr<SubscriptionInterface> GetSubscriptionAdapter() const final
        {
            return subscriptionAdapter;
        }
        virtual PluginGUIInterface *GetGUIInterface() const final
        {
            return guiInterface;
        }
        //
        // Signals
        virtual void PluginLog(const QString &) const = 0;
        virtual void PluginErrorMessageBox(const QString &title, const QString &message) const = 0;
        virtual void UpdateSettings(const QJsonObject &_settings) final
        {
            settings = _settings;
        }
        virtual const QJsonObject GetSettngs() const final
        {
            return settings;
        }

      protected:
        explicit Qv2rayInterface(){};
        QJsonObject settings;
        std::shared_ptr<PluginOutboundHandler> outboundHandler;
        std::shared_ptr<PluginEventHandler> eventHandler;
        std::shared_ptr<PluginKernelInterface> kernelInterface;
        std::shared_ptr<SubscriptionInterface> subscriptionAdapter;
        PluginGUIInterface *guiInterface;
    };
} // namespace Qv2rayPlugin

#define DECLARE_PLUGIN_INSTANCE(CLASS) inline CLASS *CLASS##Instance

QT_BEGIN_NAMESPACE
#define Qv2rayInterface_IID "com.github.Qv2ray.Qv2rayPluginInterface"
Q_DECLARE_INTERFACE(Qv2rayPlugin::Qv2rayInterface, Qv2rayInterface_IID)
QT_END_NAMESPACE
