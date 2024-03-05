#pragma once

#include "QvPlugin/Common/QvPluginBase.hpp"
#include "QvPlugin/Connections/ConnectionsBase.hpp"
#include "QvPlugin/Handlers/EventHandler.hpp"
#include "QvPlugin/Handlers/IProfilePreprocessor.hpp"
#include "QvPlugin/Handlers/KernelHandler.hpp"
#include "QvPlugin/Handlers/LatencyTestHandler.hpp"
#include "QvPlugin/Handlers/OutboundHandler.hpp"
#include "QvPlugin/Handlers/SubscriptionHandler.hpp"
#include "QvPlugin/Utils/INetworkRequestHelper.hpp"

#include <QDir>

namespace Qv2rayBase::Plugin
{
    class PluginManagerCore;
} // namespace Qv2rayBase::Plugin

#define Qv2rayInterface_IID "com.github.Qv2ray.Qv2rayPluginInterface"

namespace Qv2rayPlugin
{
    using namespace Qv2rayPlugin::Outbound;
    using namespace Qv2rayPlugin::Kernel;
    using namespace Qv2rayPlugin::Event;
    using namespace Qv2rayPlugin::Subscription;
    using namespace Qv2rayPlugin::Latency;

    template<typename>
    class Qv2rayInterface;

    namespace Gui
    {
        class Qv2rayGUIInterface;
    }

    ///
    /// \brief The Qv2rayInterfaceImpl class is the main entry for every Qv2ray plugins.
    ///
    class Qv2rayInterfaceImpl
    {
        friend class Qv2rayBase::Plugin::PluginManagerCore;
        template<typename>
        friend class Qv2rayPlugin::Qv2rayInterface;

      public:
        /// \internal
        const int QvPluginInterfaceVersion = QV2RAY_PLUGIN_INTERFACE_VERSION;
        virtual ~Qv2rayInterfaceImpl() = default;

        ///
        /// \brief GetMetadata gets metadata of a plugin
        /// \return A QvPluginMetadata structure containing plugin information
        ///
        virtual const QvPluginMetadata GetMetadata() const = 0;

        ///
        /// \brief InitializePlugin should be reimplemented by the plugin writer, this is called only once when
        /// a plugin is found and loaded after a checking for interface version.
        /// A plugin should initialize its outboundHandler, eventHandler, kernelInterface, subscriptionInterface accordingly.
        /// In case of a GUI plugin, the guiInterface should also be initialized.
        /// \return a boolean value indicating if the initialization succeeds.
        ///
        virtual bool InitializePlugin() = 0;

        virtual std::shared_ptr<Qv2rayPlugin::Outbound::IOutboundProcessor> OutboundHandler() const final
        {
            return m_OutboundHandler;
        }
        virtual std::shared_ptr<Qv2rayPlugin::Event::IEventHandler> EventHandler() const final
        {
            return m_EventHandler;
        }
        virtual std::shared_ptr<Qv2rayPlugin::Kernel::IKernelHandler> KernelInterface() const final
        {
            return m_KernelInterface;
        }
        virtual std::shared_ptr<Qv2rayPlugin::Subscription::IPluginSubscriptionInterface> SubscriptionAdapter() const final
        {
            return m_SubscriptionInterface;
        }
        virtual std::shared_ptr<Qv2rayPlugin::Latency::ILatencyHandler> LatencyTestHandler() const final
        {
            return m_LatencyTestHandler;
        }
        virtual std::shared_ptr<Qv2rayPlugin::Profile::IProfilePreprocessor> ProfilePreprocessor() const final
        {
            return m_ProfilePreprocessor;
        }
        virtual Gui::Qv2rayGUIInterface *GetGUIInterface() const final
        {
            return m_GUIInterface;
        }
        virtual QJsonObject GetSettings() const final
        {
            return m_Settings;
        }
        virtual QJsonValue GetHostContext(const QString &key) const final
        {
            return m_PluginHostContext.value(key);
        }

        ///
        /// \brief A signal that'll be connected to Qv2ray to provide logging function
        ///
        virtual void PluginLog(QString) = 0;

        ///
        /// \brief PluginErrorMessageBox shows an error messagebox to the user with title and message.
        /// \param title The title of that messagebox
        /// \param message The content of message
        ///
        virtual void PluginErrorMessageBox(QString title, QString message) = 0;

        ///
        /// \brief SettingsUpdated will be called by Qv2ray once the plugin setting is updated.
        ///
        virtual void SettingsUpdated() = 0;

        QDir WorkingDirectory()
        {
            return m_WorkingDirectory;
        }

      protected:
        QJsonObject m_Settings;
        QDir m_WorkingDirectory;

        std::shared_ptr<Qv2rayPlugin::Profile::IProfilePreprocessor> m_ProfilePreprocessor;
        std::shared_ptr<Qv2rayPlugin::Outbound::IOutboundProcessor> m_OutboundHandler;
        std::shared_ptr<Qv2rayPlugin::Event::IEventHandler> m_EventHandler;
        std::shared_ptr<Qv2rayPlugin::Kernel::IKernelHandler> m_KernelInterface;
        std::shared_ptr<Qv2rayPlugin::Subscription::IPluginSubscriptionInterface> m_SubscriptionInterface;
        std::shared_ptr<Qv2rayPlugin::Latency::ILatencyHandler> m_LatencyTestHandler;

        // Not defined as a shared_ptr since not all plugins need QtGui
        Gui::Qv2rayGUIInterface *m_GUIInterface;

      private:
        Qv2rayPlugin::Connections::IProfileManager *m_ProfileManager;
        Qv2rayPlugin::Utils::INetworkRequestHelper *m_NetworkRequestHelper;
        QJsonObject m_PluginHostContext;
    };

    template<class Impl>
    class Qv2rayInterface : public Qv2rayInterfaceImpl
    {
      public:
        static inline Impl *PluginInstance;
        static void Log(const QString &msg)
        {
            PluginInstance->PluginLog(msg);
        }
        static void ShowMessageBox(const QString &title, const QString &message)
        {
            PluginInstance->PluginErrorMessageBox(title, message);
        }

        static Qv2rayPlugin::Connections::IProfileManager *ProfileManager()
        {
            return PluginInstance->m_ProfileManager;
        }

        static Qv2rayPlugin::Utils::INetworkRequestHelper *NetworkRequestHelper()
        {
            return PluginInstance->m_NetworkRequestHelper;
        }

      protected:
        explicit Qv2rayInterface(Impl *impl) : Qv2rayInterfaceImpl()
        {
            PluginInstance = impl;
        }
    };
} // namespace Qv2rayPlugin

#define QV2RAY_PLUGIN(CLASS)                                                                                                                                             \
    Q_INTERFACES(Qv2rayPlugin::Qv2rayInterfaceImpl)                                                                                                                      \
    Q_PLUGIN_METADATA(IID Qv2rayInterface_IID)                                                                                                                           \
  public:                                                                                                                                                                \
    explicit CLASS() : QObject(), Qv2rayInterface(this){};                                                                                                               \
    ~CLASS(){};                                                                                                                                                          \
                                                                                                                                                                         \
    Q_SIGNAL void PluginLog(QString) override;                                                                                                                           \
    Q_SIGNAL void PluginErrorMessageBox(QString, QString) override;

QT_BEGIN_NAMESPACE
Q_DECLARE_INTERFACE(Qv2rayPlugin::Qv2rayInterfaceImpl, Qv2rayInterface_IID)
QT_END_NAMESPACE
