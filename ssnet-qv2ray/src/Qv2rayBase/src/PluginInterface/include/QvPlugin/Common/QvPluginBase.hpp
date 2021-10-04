#pragma once

#include "QvPlugin/Common/CommonTypes.hpp"

#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QVariant>
#include <memory>
#include <utility>

namespace Qv2rayPlugin
{
    Q_NAMESPACE
    constexpr inline auto QV2RAY_PLUGIN_INTERFACE_VERSION = 5;

    ///
    /// \brief The QV2RAY_PLUGIN_COMPONENT_TYPE enum indicates different capabilities of a plugin
    ///
    enum PLUGIN_COMPONENT_TYPE
    {
        COMPONENT_EVENT_HANDLER = 0,
        COMPONENT_GUI = 1,
        COMPONENT_KERNEL = 2,
        COMPONENT_OUTBOUND_HANDLER = 3,
        COMPONENT_SUBSCRIPTION_ADAPTER = 4,
        COMPONENT_LATENCY_TEST_ENGINE = 5,
        COMPONENT_PROFILE_PREPROCESSOR = 6,
    };
    Q_ENUM_NS(PLUGIN_COMPONENT_TYPE)

    ///
    /// \brief The QV2RAY_PLUGIN_GUI_COMPONENT_TYPE enum indicates different GUI capabilities of a plugin
    ///
    enum PLUGIN_GUI_COMPONENT_TYPE
    {
        ///
        /// \brief The plugin has a settings widget.
        ///
        GUI_COMPONENT_SETTINGS = 0,
        ///
        /// \brief The plugin has an outbound editor
        ///
        GUI_COMPONENT_OUTBOUND_EDITOR = 1,
        ///
        /// \brief The plugin has an inbound editor
        ///
        GUI_COMPONENT_INBOUND_EDITOR = 2,
        ///
        /// \brief The plugin has MainWindow QActions
        ///
        GUI_COMPONENT_MAIN_WINDOW_ACTIONS = 3,
        ///
        /// \brief The plugin has tray menus
        ///
        GUI_COMPONENT_TRAY_MENUS = 4,
#if PLUGIN_INTERFACE_VERSION > 5
        ///
        /// \brief The plugin has Profile editor.
        ///
        GUI_COMPONENT_PROFILE_EDITOR = 5,
#endif
    };
    Q_ENUM_NS(PLUGIN_GUI_COMPONENT_TYPE)

    struct QvPluginMetadata
    {
        ///
        /// \brief The name of this plugin
        ///
        QString Name;

        ///
        /// \brief The author of this plugin
        ///
        QString Author;

        ///
        /// \brief The internal identifier of the plugin.
        ///
        /// \details
        /// This value is used by Qv2ray to identify different plugins, including storing and
        /// restoring settings.
        ///
        PluginId InternalID;

        ///
        /// \brief The descriptive string which will let users know what plugin does
        ///
        QString Description;

        ///
        /// \brief The URL of this plugin, if any.
        ///
        QUrl Url;

        ///
        /// \brief A List of QV2RAY_PLUGIN_COMPONENT_TYPEs to indicate what this plugin can do.
        ///
        QList<PLUGIN_COMPONENT_TYPE> Components;

        QvPluginMetadata(const QString &name, const QString &author, const PluginId &id, const QString &description, const QUrl &url,
                         const QList<PLUGIN_COMPONENT_TYPE> &comps)
            : Name(name), Author(author), InternalID(id), Description(description), Url(url), Components(comps){};
        QvPluginMetadata() = default;
    };
} // namespace Qv2rayPlugin

Q_DECLARE_METATYPE(Qv2rayPlugin::QvPluginMetadata)
