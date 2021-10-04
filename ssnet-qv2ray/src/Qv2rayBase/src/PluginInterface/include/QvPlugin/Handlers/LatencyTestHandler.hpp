#pragma once

#include "QvPlugin/Common/CommonTypes.hpp"

namespace uvw
{
    class Loop;
}

namespace Qv2rayPlugin::Latency
{
    struct LatencyTestRequest
    {
        LatencyTestEngineId engine;
        ConnectionId id;
        QString host;
        int port;
    };

    struct LatencyTestResponse
    {
        LatencyTestEngineId engine;
        int total;
        int failed;
        int succeeded;
        QString error;
        long worst = LATENCY_TEST_VALUE_ERROR;
        long best = LATENCY_TEST_VALUE_ERROR;
        long avg = LATENCY_TEST_VALUE_ERROR;
    };

    class LatencyTestEngine : public QObject
    {
      public:
        explicit LatencyTestEngine() = default;
        virtual ~LatencyTestEngine() = default;
        virtual LatencyTestResponse TestLatency(const LatencyTestRequest &)
        {
            Q_UNREACHABLE();
        };
        virtual void TestLatencyAsync(std::shared_ptr<uvw::Loop>, const LatencyTestRequest &)
        {
            Q_UNREACHABLE();
        }

        virtual void OnLatencyTestFinishedSignal(const ConnectionId &, const LatencyTestResponse &) = 0;
    };

    struct LatencyTestEngineInfo
    {
        LatencyTestEngineId Id;
        bool isAsync;
        QString Name;
        QString Description;
        std::function<std::shared_ptr<LatencyTestEngine>(void)> Create;
    };

    class ILatencyHandler
    {
      public:
        virtual QList<LatencyTestEngineInfo> PluginLatencyTestEngines() const = 0;
    };

} // namespace Qv2rayPlugin::Latency
