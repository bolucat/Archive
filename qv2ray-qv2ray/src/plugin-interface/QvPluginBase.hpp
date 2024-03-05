#pragma once

#include <QJsonObject>
#include <QObject>
#include <QString>
#include <QVariant>
#include <memory>

constexpr auto QV2RAY_PLUGIN_INTERFACE_VERSION = 3;
constexpr auto QV2RAY_PLUGIN_INTERNAL_PROPERTY_KEY = "_QV2RAY_PLUGIN_OPTIONS_";

namespace Qv2rayPlugin
{
    enum GlobalPluginOptions
    {
        OPTION_SET_TLS_ALLOW_INSECURE Q_DECL_ENUMERATOR_DEPRECATED_X("Do not use this!") = 0,
        OPTION_SET_TLS_SESSION_RESUMPTION Q_DECL_ENUMERATOR_DEPRECATED_X("Do not use this!") = 1,
        OPTION_SET_TLS_DISABLE_SYSTEM_CERTS = 2
    };

    using Qv2rayPluginOption = QMap<GlobalPluginOptions, QVariant>;

    enum PluginUpdateType
    {
        UPDATE_NONE = 0,
        UPDATE_GITHUB_RELEASE = 1,
        UPDATE_URL = 2
    };

    enum PluginGuiComponentType
    {
        GUI_COMPONENT_SETTINGS = 0,
        GUI_COMPONENT_OUTBOUND_EDITOR = 1,
        GUI_COMPONENT_INBOUND_EDITOR = 2,
        GUI_COMPONENT_MAINWINDOW_WIDGET = 3
    };

    enum PluginComponentType
    {
        COMPONENT_EVENT_HANDLER = 0,
        COMPONENT_GUI = 1,
        COMPONENT_KERNEL = 2,
        COMPONENT_OUTBOUND_HANDLER = 3,
        COMPONENT_SUBSCRIPTION_ADAPTER = 4,
    };

    enum OutboundInfoFlags
    {
        INFO_DISPLAYNAME = 0,
        INFO_PROTOCOL = 1,
        INFO_SERVER = 2,
        INFO_PORT = 3,
        INFO_SNI = 4
    };

    enum KernelOptionFlags
    {
        KERNEL_HTTP_ENABLED,
        KERNEL_HTTP_PORT,
        KERNEL_SOCKS_ENABLED,
        KERNEL_SOCKS_PORT,
        KERNEL_SOCKS_UDP_ENABLED,
        KERNEL_SOCKS_LOCAL_ADDRESS,
        KERNEL_LISTEN_ADDRESS
    };

    struct ProtocolInfoObject
    {
      public:
        QString protocol;
        QString displayName;
        explicit ProtocolInfoObject(){};
        explicit ProtocolInfoObject(const QString &protocol, const QString &displayName) : protocol(protocol), displayName(displayName){};
        friend bool operator==(const ProtocolInfoObject &l, const ProtocolInfoObject &r)
        {
            return l.protocol == r.protocol && l.displayName == r.displayName;
        }
    };

    typedef QMap<OutboundInfoFlags, QVariant> OutboundInfoObject;

    struct QvPluginMetadata
    {
        QString Name;
        QString Author;
        QString InternalName;
        QString Description;
        QString VersionString;
        QString UpdateLocation;
        PluginUpdateType UpdateType;
        QList<PluginComponentType> Components;
        QvPluginMetadata(const QString &name,                                   //
                         const QString &author,                                 //
                         const QString &internalName,                           //
                         const QString &description,                            //
                         const QString &versionString,                          //
                         const QString &updateUrl,                              //
                         const QList<PluginComponentType> &supportedComponents, //
                         const PluginUpdateType updateType)                     //
            : Name(name),                                                       //
              Author(author),                                                   //
              InternalName(internalName),                                       //
              Description(description),                                         //
              VersionString(versionString),                                     //
              UpdateLocation(updateUrl),                                        //
              UpdateType(updateType),                                           //
              Components(supportedComponents){};                                //
        QvPluginMetadata(){};
    };
} // namespace Qv2rayPlugin

Q_DECLARE_METATYPE(Qv2rayPlugin::Qv2rayPluginOption);
