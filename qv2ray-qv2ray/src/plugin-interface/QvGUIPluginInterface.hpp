#pragma once
#include "QvPluginBase.hpp"

#include <QDialog>
#include <QMenu>
#include <QWidget>

namespace Qv2rayPlugin
{
    class QvPluginSettingsWidget : public QWidget
    {
        Q_OBJECT
      public:
        explicit QvPluginSettingsWidget(QWidget *parent) : QWidget(parent){};
        virtual ~QvPluginSettingsWidget(){};
        virtual void SetSettings(const QJsonObject &) = 0;
        virtual QJsonObject GetSettings() = 0;
    };

    class QvPluginMainWindowWidget : public QDialog
    {
        Q_OBJECT
      public:
        explicit QvPluginMainWindowWidget(QWidget *parent) : QDialog(parent){};
        virtual ~QvPluginMainWindowWidget(){};
        virtual const QList<QMenu *> GetMenus() = 0;
    };

#define PLUGIN_EDITOR_LOADING_SCOPE(t)                                                                                                               \
    isLoading = true;                                                                                                                                \
    t;                                                                                                                                               \
    isLoading = false;

#define PLUGIN_EDITOR_LOADING_GUARD                                                                                                                  \
    if (this->isLoading)                                                                                                                             \
        return;

    const static inline auto GetProperty = [](const auto widget, auto name) -> auto
    {
        const auto prop = widget->property(name);
        return prop.isValid() && prop.toBool();
    };

    class QvPluginEditor : public QWidget
    {
        Q_OBJECT
      public:
        explicit QvPluginEditor(QWidget *parent = nullptr) : QWidget(parent){};
        virtual ~QvPluginEditor(){};
        //
        virtual void SetHostAddress(const QString &address, int port) = 0;
        virtual QPair<QString, int> GetHostAddress() const = 0;
        //
        virtual void SetContent(const QJsonObject &) = 0;
        virtual const QJsonObject GetContent() const = 0;

      protected:
        QJsonObject content;
        bool isLoading = false;
    };

    template<typename T>
    inline QPair<ProtocolInfoObject, T *> MakeEditorInfoPair(const QString &protocol, const QString &displayName)
    {
        return { ProtocolInfoObject(protocol, displayName), new T() };
    }

    class PluginGUIInterface
    {
      public:
        using typed_plugin_editor = QPair<ProtocolInfoObject, QvPluginEditor *>;
        explicit PluginGUIInterface(){};
        virtual ~PluginGUIInterface(){};
        virtual QIcon Icon() const = 0;
        virtual QList<PluginGuiComponentType> GetComponents() const = 0;
        virtual std::unique_ptr<QvPluginSettingsWidget> GetSettingsWidget() const final
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
        virtual std::unique_ptr<QvPluginMainWindowWidget> GetMainWindowWidget() const final
        {
            return createMainWindowWidget();
        }

      protected:
        virtual std::unique_ptr<QvPluginSettingsWidget> createSettingsWidgets() const = 0;
        virtual QList<typed_plugin_editor> createInboundEditors() const = 0;
        virtual QList<typed_plugin_editor> createOutboundEditors() const = 0;
        virtual std::unique_ptr<QvPluginMainWindowWidget> createMainWindowWidget() const = 0;
    };

} // namespace Qv2rayPlugin
