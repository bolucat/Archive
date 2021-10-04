#pragma once
#include <QtCore/QObject>
#include <memory>
#include <nodes/NodeData>
#include <nodes/NodeDataModel>
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeDataType;
using QtNodes::NodeValidationState;
using QtNodes::PortIndex;
using QtNodes::PortType;
/// The class can potentially incapsulate any user data which
/// need to be transferred within the Node Editor graph
class MyNodeData : public NodeData
{
  public:
    std::shared_ptr<NodeDataType> type() const override
    {
        return std::make_shared<NodeDataType>("MyNodeData", "My Node Data");
    }
};
//------------------------------------------------------------------------------
/// The model dictates the number of inputs and outputs for the Node.
/// In this example it has no logic.
class MyDataModel : public NodeDataModel
{
    Q_OBJECT
  public:
    virtual ~MyDataModel()
    {
    }

  public:
    QString caption() const override
    {
        return QString("My Data Model");
    }
    QString name() const override
    {
        return QString("MyDataModel");
    }

  public:
    QJsonObject save() const override
    {
        QJsonObject modelJson;
        modelJson["name"] = name();
        return modelJson;
    }

  public:
    unsigned int nPorts(PortType) const override
    {
        return 3;
    }
    std::shared_ptr<NodeDataType> dataType(PortType, PortIndex) const override
    {
        return MyNodeData().type();
    }
    std::shared_ptr<NodeData> outData(PortIndex) override
    {
        return std::make_shared<MyNodeData>();
    }
    void setInData(std::shared_ptr<NodeData>, int) override
    {
        //
    }
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<MyDataModel>();
    }
    QWidget *embeddedWidget() override
    {
        return nullptr;
    }
};
