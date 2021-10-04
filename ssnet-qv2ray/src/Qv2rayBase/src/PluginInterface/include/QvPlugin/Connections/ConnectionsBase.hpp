#pragma once

#include "QvPlugin/Common/CommonTypes.hpp"

namespace Qv2rayPlugin::Connections
{
    class IProfileManager
    {
      public:
        IProfileManager() = default;
        virtual ~IProfileManager() = default;

        ///
        /// \brief IsConnected Checks if an id is the current connection.
        /// \param id The ProfileId to check
        ///
        virtual bool IsConnected(const ProfileId &) const = 0;

        ///
        /// \brief GetConnection Get the profile content of a connection
        /// \param id The ConnectionId
        ///
        virtual const ProfileContent GetConnection(const ConnectionId &) const = 0;

        ///
        /// \brief GetConnectionObject Get ConnectionObject containing connection metadata.
        /// \param id The ConnectionId
        ///
        virtual const ConnectionObject GetConnectionObject(const ConnectionId &) const = 0;

        ///
        /// \brief GetGroupObject Get GroupObject containing group metadata.
        /// \param id The GroupId
        ///
        virtual const GroupObject GetGroupObject(const GroupId &) const = 0;

        ///
        /// \brief GetConnections Get all connections
        /// \return All ConnectionIds within the ProfileManager
        ///
        virtual const QList<ConnectionId> GetConnections() const = 0;

        ///
        /// \brief GetConnections Get all connections within a group.
        /// \param groupId The GroupId
        /// \return All ConnectionIds in that group
        ///
        virtual const QList<ConnectionId> GetConnections(const GroupId &) const = 0;

        ///
        /// \brief GetGroups Get all groups
        /// \return All GroupIds
        ///
        virtual const QList<GroupId> GetGroups() const = 0;

        ///
        /// \brief GetGroups Get all groups that contains a ConnectionId
        /// \param connId The ConnectionId to check.
        /// \return All groups which the ConnectionId is in.
        ///
        virtual const QList<GroupId> GetGroups(const ConnectionId &) const = 0;

        ///
        /// \brief StartConnection Start a connection.
        /// \param identifier The ProfileId to start
        /// \return A boolean indicating whether the start is successful.
        ///
        virtual bool StartConnection(const ProfileId &) = 0;

        ///
        /// \brief StopConnection Stop current connection.
        ///
        virtual void StopConnection() = 0;

        ///
        /// \brief RestartConnection Restart current connection.
        /// \return A boolean indicating whether the start is successful.
        ///
        virtual bool RestartConnection() = 0;

        ///
        /// \brief CreateConnection Create a connection within a group.
        /// \param root The connection content
        /// \param name The name of new connection.
        /// \param groupId The GroupId
        /// \return A ProfileId that can locate to this connection.
        ///
        virtual const ProfileId CreateConnection(const ProfileContent &, const QString &, const GroupId & = DefaultGroupId) = 0;

        ///
        /// \brief RenameConnection
        /// \param id The connection id to rename.
        /// \param newName The new name.
        ///
        virtual void RenameConnection(const ConnectionId &id, const QString &newName) = 0;

        ///
        /// \brief SetConnectionTags
        /// \param id The connection id
        /// \param tags The new tags
        ///
        virtual void SetConnectionTags(const ConnectionId &id, const QStringList &tags) = 0;
        ///
        /// \brief UpdateConnection
        /// \param id The connection id to update.
        /// \param root The new content.
        ///
        virtual void UpdateConnection(const ConnectionId &id, const ProfileContent &root) = 0;

        ///
        /// \brief RemoveFromGroup Tries to remove a connection from a group, in case of that's
        /// the last group which the connection is contained in, the connection will be deleted.
        /// \param id The connection id
        /// \param gid The group id to remove.
        /// \return
        ///
        virtual bool RemoveFromGroup(const ConnectionId &id, const GroupId &gid) = 0;

        ///
        /// \brief MoveToGroup Moves a connection from one group to another.
        /// \param id The connection id
        /// \param sourceGid The group which that connection is currently in.
        /// \param targetGid The destination group id.
        /// \return True if the movement succeeded.
        ///
        virtual bool MoveToGroup(const ConnectionId &id, const GroupId &sourceGid, const GroupId &targetGid) = 0;

        ///
        /// \brief LinkWithGroup Links a connection to another group, in this case, a connection may appear in many different groups.
        /// Allowing to use different route settings provided by each groups.
        /// \param id The connection id.
        /// \param newGroupId The new group id to be linked with.
        /// \return True if the linkage succeeded.
        ///
        virtual bool LinkWithGroup(const ConnectionId &id, const GroupId &newGroupId) = 0;

        ///
        /// \brief CreateGroup Creates a new group.
        /// \param displayName The name of new group.
        /// \return The newly created id of that group.
        ///
        virtual const GroupId CreateGroup(const QString &displayName) = 0;

        ///
        /// \brief DeleteGroup Delete a specified group.
        /// \param id The id of a group to be removed.
        /// \param alsoRemoveConnections When set to true, also clean up the connections in that group instead of moving them to the default group.
        /// \return True if the removal succeeded.
        ///
        virtual bool DeleteGroup(const GroupId &id, bool alsoRemoveConnections) = 0;

        ///
        /// \brief RenameGroup Rename a group.
        /// \param id The id of a group to be renamed.
        /// \param newName The new name.
        /// \return True if the renaming succeeded.
        ///
        virtual bool RenameGroup(const GroupId &id, const QString &newName) = 0;

        ///
        /// \brief GetGroupRoutingId Get the routing id of a group.
        /// \param id The group id;
        /// \return The routing id associated with that group.
        ///
        virtual const RoutingId GetGroupRoutingId(const GroupId &id) = 0;

        ///
        /// \brief GetRouting Get the routing object by id
        /// \param id The RoutingId
        /// \return A routing object, if there's no routingobject for the ID, the routingobject with DefaultRoutingId will be returned.
        ///
        virtual RoutingObject GetRouting(const RoutingId &id) const = 0;

        ///
        /// \brief UpdateRouting Stores the routing object.
        /// \param id The id of routing object/
        /// \param o The routing object.
        ///
        virtual void UpdateRouting(const RoutingId &id, const RoutingObject &o) = 0;
    };

} // namespace Qv2rayPlugin::Connections
