#pragma once
#include "QJsonStruct.hpp"

#include <QHash>
#include <QHashFunctions>
#include <QString>
#include <QtCore>

namespace Qv2rayPlugin::connections::types
{
    template<typename placeholder, typename BASETYPE_T>
    class __QV_SAFETYPE_IMPL : public BASETYPE_T
    {
      public:
        template<class... Args>
        explicit __QV_SAFETYPE_IMPL(Args... args) : BASETYPE_T(args...){};
        const BASETYPE_T &raw() const
        {
            return *this;
        }
    };

#define SAFE_TYPEDEF(BASE, CLASS)                                                                                                                                        \
    class __##CLASS##__;                                                                                                                                                 \
    typedef __QV_SAFETYPE_IMPL<__##CLASS##__, BASE> CLASS;

    // To prevent anonying QJsonObject misuse
    SAFE_TYPEDEF(QJsonObject, INBOUNDSETTING);
    SAFE_TYPEDEF(QJsonObject, OUTBOUNDSETTING);
    SAFE_TYPEDEF(QJsonObject, INBOUND);
    SAFE_TYPEDEF(QJsonObject, OUTBOUND);
    SAFE_TYPEDEF(QJsonObject, CONFIGROOT);
    SAFE_TYPEDEF(QJsonObject, ROUTING);
    SAFE_TYPEDEF(QJsonObject, ROUTERULE);
    //
    SAFE_TYPEDEF(QJsonArray, OUTBOUNDS);
    SAFE_TYPEDEF(QJsonArray, INBOUNDS);

    enum class IOBOUND
    {
        DISPLAYNAME = 0,
        PROTOCOL = 1,
        ADDRESS = 2,
        PORT = 3,
        SNI = 4
    };
    typedef QMap<IOBOUND, QVariant> PluginIOBoundData;

    template<typename T>
    class IDType
    {
      public:
        explicit IDType() : m_id("null"){};
        explicit IDType(const QString &id) : m_id(id){};
        friend bool operator==(const IDType<T> &lhs, const IDType<T> &rhs)
        {
            return lhs.m_id == rhs.m_id;
        }
        friend bool operator!=(const IDType<T> &lhs, const IDType<T> &rhs)
        {
            return lhs.m_id != rhs.m_id;
        }
        const QString toString() const
        {
            return m_id;
        }
        void loadJson(const QJsonValue &d)
        {
            m_id = d.toString("null");
        }
        QJsonValue toJson() const
        {
            return m_id;
        }
        bool isEmpty() const
        {
            return m_id == "null";
        }

      private:
        QString m_id;
    };

    // Define several safetypes to prevent misuse of QString.
#define DECL_IDTYPE(type)                                                                                                                                                \
    class __##type;                                                                                                                                                      \
    typedef IDType<__##type> type

    DECL_IDTYPE(GroupId);
    DECL_IDTYPE(ConnectionId);
    DECL_IDTYPE(GroupRoutingId);
    const inline GroupId DefaultGroupId{ "000000000000" };

    inline const static ConnectionId NullConnectionId;
    inline const static GroupId NullGroupId;
    inline const static GroupRoutingId NullRoutingId;

    struct ConnectionGroupPair
    {
      public:
        ConnectionGroupPair(){};
        ConnectionGroupPair(const ConnectionGroupPair &another)
        {
            *this = another;
        }
        ConnectionGroupPair &operator=(const ConnectionGroupPair &another)
        {
            connectionId = another.connectionId;
            groupId = another.groupId;
            return *this;
        }
        friend bool operator==(const ConnectionGroupPair &one, const ConnectionGroupPair &another)
        {
            return one.connectionId == another.connectionId && one.groupId == another.groupId;
        }
        friend bool operator!=(const ConnectionGroupPair &one, const ConnectionGroupPair &another)
        {
            return !(one == another);
        }
        ConnectionGroupPair(const ConnectionId &conn, const GroupId &group)
        {
            connectionId = conn;
            groupId = group;
        }

        ConnectionId connectionId = NullConnectionId;
        GroupId groupId = NullGroupId;

        void clear()
        {
            connectionId = NullConnectionId;
            groupId = NullGroupId;
        }

      public:
        bool isEmpty() const
        {
            return groupId == NullGroupId || connectionId == NullConnectionId;
        }
        QJS_PLAIN_JSON(connectionId, groupId)
    };

    template<typename T>
    inline size_t qHash(IDType<T> key)
    {
        return ::qHash(key.toString());
    }
    inline size_t qHash(const ConnectionGroupPair &pair)
    {
        return ::qHash(pair.connectionId.toString() + pair.groupId.toString());
    }
} // namespace Qv2rayPlugin::connections

Q_DECLARE_METATYPE(Qv2rayPlugin::connections::types::ConnectionGroupPair)
Q_DECLARE_METATYPE(Qv2rayPlugin::connections::types::ConnectionId)
Q_DECLARE_METATYPE(Qv2rayPlugin::connections::types::GroupId)
Q_DECLARE_METATYPE(Qv2rayPlugin::connections::types::GroupRoutingId)

using namespace Qv2rayPlugin::connections::types;
