#pragma once

#include "ConnectionsBaseTypes.hpp"

namespace Qv2rayPlugin::connections
{
    class IConnectionManager
    {
      public:
        IConnectionManager() = default;
        virtual ~IConnectionManager() = default;

        virtual bool IsConnected(const ConnectionGroupPair &id) const = 0;
        virtual const QList<GroupId> Subscriptions() const = 0;
        virtual const QList<GroupId> GetConnectionContainedIn(const ConnectionId &connId) const = 0;

        virtual const QList<ConnectionId> GetConnections() const = 0;
        virtual const QList<ConnectionId> GetConnections(const GroupId &groupId) const = 0;
        virtual const QList<GroupId> AllGroups() const = 0;
        //
        // Connectivity Operationss
        virtual bool StartConnection(const ConnectionGroupPair &identifier) = 0;
        virtual void StopConnection() = 0;
        virtual void RestartConnection() = 0;
        //
        // Connection Operations.
        virtual void ClearGroupUsage(const GroupId &id) = 0;
        virtual void ClearConnectionUsage(const ConnectionGroupPair &id) = 0;
        //
        virtual const ConnectionGroupPair CreateConnection(const CONFIGROOT &root, const QString &displayName, const GroupId &groupId = DefaultGroupId,
                                                           bool skipSaveConfig = false) = 0;
        virtual bool UpdateConnection(const ConnectionId &id, const CONFIGROOT &root, bool skipRestart = false) = 0;
        virtual const std::optional<QString> RenameConnection(const ConnectionId &id, const QString &newName) = 0;
        //
        // Connection - Group binding
        virtual bool RemoveConnectionFromGroup(const ConnectionId &id, const GroupId &gid) = 0;
        virtual bool MoveConnectionFromToGroup(const ConnectionId &id, const GroupId &sourceGid, const GroupId &targetGid) = 0;
        virtual bool LinkConnectionWithGroup(const ConnectionId &id, const GroupId &newGroupId) = 0;
        //
        // Get Conncetion Property
        virtual const CONFIGROOT GetConnectionRoot(const ConnectionId &id) const = 0;
        //
        // Group Operations
        virtual const GroupId CreateGroup(const QString &displayName, bool isSubscription) = 0;
        virtual const std::optional<QString> DeleteGroup(const GroupId &id) = 0;
        virtual const std::optional<QString> RenameGroup(const GroupId &id, const QString &newName) = 0;
        virtual const GroupRoutingId GetGroupRoutingId(const GroupId &id) = 0;
    };

} // namespace Qv2rayPlugin::connections
