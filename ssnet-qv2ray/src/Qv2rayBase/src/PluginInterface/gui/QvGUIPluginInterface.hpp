#pragma once

#include "../QvPluginBase.hpp"

#include <QDialog>
#include <QJsonObject>
#include <QList>
#include <QMenu>
#include <QWidget>
#include <utility>

namespace Qv2rayPlugin
{
    struct ProtocolInfoObject
    {
        QString Protocol;
        QString DisplayName;
    };

    class PluginSettingsWidget : public QWidget
    {
        Q_OBJECT
      public:
        explicit PluginSettingsWidget(QWidget *parent) : QWidget(parent){};
        ~PluginSettingsWidget() override = default;

        virtual void SetSettings(const QJsonObject &) = 0;
        virtual QJsonObject GetSettings() = 0;
    };

    class PluginMainWindowWidget : public QDialog
    {
        Q_OBJECT
      public:
        explicit PluginMainWindowWidget(QWidget *parent) : QDialog(parent){};
        ~PluginMainWindowWidget() override = default;
    };

#define PLUGIN_EDITOR_LOADING_SCOPE(t)                                                                                                                                   \
    isLoading = true;                                                                                                                                                    \
    t;                                                                                                                                                                   \
    isLoading = false;

#define PLUGIN_EDITOR_LOADING_GUARD                                                                                                                                      \
    if (this->isLoading)                                                                                                                                                 \
        return;

    template<typename T>
    inline bool GetProperty(const T *widget, const char *name)
    {
        const auto prop = widget->property(name);
        return prop.isValid() && prop.toBool();
    }

    class QvPluginEditor : public QWidget
    {
        Q_OBJECT
      public:
        explicit QvPluginEditor(QWidget *parent = nullptr) : QWidget(parent){};
        virtual ~QvPluginEditor() = default;

        virtual void SetHostAddress(const QString &address, int port) = 0;
        virtual QPair<QString, int> GetHostAddress() const = 0;

        virtual void SetContent(const QJsonObject &) = 0;
        virtual const QJsonObject GetContent() const = 0;

      protected:
        QJsonObject content;
        bool isLoading = false;
    };

    template<typename T>
    inline QPair<ProtocolInfoObject, T *> make_editor_info(const QString &protocol, const QString &displayName)
    {
        return { ProtocolInfoObject{ protocol, displayName }, new T() };
    }

    class Qv2rayGUIInterface
    {
      public:
        using typed_plugin_editor = QPair<ProtocolInfoObject, QvPluginEditor *>;

        explicit Qv2rayGUIInterface() = default;
        virtual ~Qv2rayGUIInterface() = default;

        virtual QIcon Icon() const = 0;
        virtual QList<QV2RAY_PLUGIN_GUI_COMPONENT_TYPE> GetComponents() const = 0;
        virtual std::unique_ptr<PluginSettingsWidget> GetSettingsWidget() const final
        {
            return createSettingsWidgets();
        }
        virtual QList<typed_plugin_editor> GetInboundEditors() const final
        {
            return createInboundEditors();
        }
        virtual QList<typed_plugin_editor> GetOutboundEditors() const final
        {
            return createOutboundEditors();
        }
        virtual std::unique_ptr<PluginMainWindowWidget> GetMainWindowWidget() const final
        {
            return createMainWindowWidget();
        }

      protected:
        virtual std::unique_ptr<PluginSettingsWidget> createSettingsWidgets() const = 0;
        virtual QList<typed_plugin_editor> createInboundEditors() const = 0;
        virtual QList<typed_plugin_editor> createOutboundEditors() const = 0;
        virtual std::unique_ptr<PluginMainWindowWidget> createMainWindowWidget() const = 0;
    };

} // namespace Qv2rayPlugin
