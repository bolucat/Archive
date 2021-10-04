#pragma once
#include <nodes/NodeDataModel>
using QtNodes::NodeData;
using QtNodes::NodeDataType;
using QtNodes::PortIndex;
using QtNodes::PortType;
class StateNodeModel : public QtNodes::NodeDataModel
{
  public:
    static std::shared_ptr<NodeDataType> getTranstitionType();
    QString caption() const override;
    QString name() const override;
    unsigned int nPorts(PortType portType) const override;
    std::shared_ptr<NodeDataType> dataType(PortType portType, PortIndex portIndex) const override;
    ConnectionPolicy portOutConnectionPolicy(PortIndex) const override;
    ConnectionPolicy portInConnectionPolicy(PortIndex) const override;
    void setInData(std::shared_ptr<NodeData> nodeData, PortIndex port) override;
    void setInData(std::vector<std::shared_ptr<NodeData>> nodeData, PortIndex port) override;
    std::shared_ptr<NodeData> outData(PortIndex port) override;
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<StateNodeModel>();
    }
    QWidget *embeddedWidget() override
    {
        return nullptr;
    }
};
