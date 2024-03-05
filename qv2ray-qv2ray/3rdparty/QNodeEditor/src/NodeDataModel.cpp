#include "NodeDataModel.hpp"

#include "StyleCollection.hpp"
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeStyle;
using QtNodes::PortIndex;
using QtNodes::PortType;
NodeDataModel::NodeDataModel() : m_wembed(false), _nodeStyle(StyleCollection::nodeStyle())
{
    // Derived classes can initialize specific style here
}
QJsonObject NodeDataModel::save() const
{
    QJsonObject modelJson;
    modelJson["name"] = name();
    return modelJson;
}
NodeDataModel::ConnectionPolicy NodeDataModel::portConnectionPolicy(PortType portType, PortIndex portIndex) const
{
    ConnectionPolicy result = ConnectionPolicy::One;
    switch (portType)
    {
        case PortType::In: result = portInConnectionPolicy(portIndex); break;
        case PortType::Out: result = portOutConnectionPolicy(portIndex); break;
        default: break;
    }
    return result;
}
NodeStyle const &NodeDataModel::nodeStyle() const
{
    return _nodeStyle;
}
void NodeDataModel::setNodeStyle(NodeStyle const &style)
{
    _nodeStyle = style;
}
void NodeDataModel::setInData(std::vector<std::shared_ptr<NodeData>> nodeData, PortIndex port)
{
    if (portInConnectionPolicy(port) == QtNodes::NodeDataModel::ConnectionPolicy::One)
    {
        if (nodeData.empty())
            setInData(nullptr, port);
        else
            // Sends the first piece of data since ConnectionPolicy == One
            setInData(nodeData[0], port);
    }
    else
    {
        Q_ASSERT_X(false, Q_FUNC_INFO, "Please override this function if you want to support multi-in data.");
    }
}
bool NodeDataModel::wembed() const
{
    return m_wembed;
}
void NodeDataModel::setWembed(bool wembed)
{
    m_wembed = wembed;
}
