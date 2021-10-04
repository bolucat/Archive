#pragma once
#include <QObject>
#include <QUuid>
#include <functional>

namespace Qv2rayPlugin::handlers::kernel
{
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

    class PluginKernel : public QObject
    {
        Q_OBJECT
      public:
        explicit PluginKernel() : QObject(){};
        ~PluginKernel() override = default;
        virtual void SetConnectionSettings(const QMap<KernelOptionFlags, QVariant> &settings, const QJsonObject &connectionInfo) = 0;
        virtual bool Start() = 0;
        virtual bool Stop() = 0;
        virtual QUuid KernelId() const = 0;

      signals:
        void OnCrashed(const QString &);
        void OnKernelLog(const QString &);
        void OnStatsAvailable(quint64 upSpeed, quint64 downSpeed);
    };

    struct KernelInfo
    {
        QUuid Id;
        QString Name;
        QStringList SupportedProtocols;
        std::function<std::unique_ptr<PluginKernel>(void)> Create;
    };

    class PluginKernelInterface
    {
      public:
        virtual QList<KernelInfo> GetKernels() const = 0;
    };
} // namespace Qv2rayPlugin::handlers::kernel

using namespace Qv2rayPlugin::handlers::kernel;
