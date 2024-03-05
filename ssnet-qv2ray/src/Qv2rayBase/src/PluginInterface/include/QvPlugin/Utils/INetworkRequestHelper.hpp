#pragma once
#include <QNetworkReply>

namespace Qv2rayPlugin::Utils
{
    class INetworkRequestHelper
    {
      public:
        typedef std::tuple<QNetworkReply::NetworkError, QString, QByteArray> GetResult;
        typedef std::function<void(QNetworkReply *)> EncryptedCallback;
        virtual GetResult Get(const QUrl &url, const EncryptedCallback &onEncrypted = {}) = 0;
    };
} // namespace Qv2rayPlugin::Utils
