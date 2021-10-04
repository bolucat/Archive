#pragma once
#include <QtCore/QObject>
#include <memory>
#include <nodes/NodeData>
#include <nodes/NodeDataModel>
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeDataType;
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
class SimpleNodeData : public NodeData
{
  public:
    std::shared_ptr<NodeDataType> type() const override
    {
        return std::make_shared<NodeDataType>("SimpleData", "Simple Data");
    }
};
//------------------------------------------------------------------------------
/// The model dictates the number of inputs and outputs for the Node.
/// In this example it has no logic.
class NaiveDataModel : public NodeDataModel
{
    Q_OBJECT
  public:
    virtual ~NaiveDataModel()
    {
    }

  public:
    QString caption() const override
    {
        return QString("Naive Data Model");
    }
    QString name() const override
    {
        return QString("NaiveDataModel");
    }

  public:
    unsigned int nPorts(PortType portType) const override
    {
        unsigned int result = 1;
        switch (portType)
        {
            case PortType::In: result = 2; break;
            case PortType::Out: result = 2; break;
            case PortType::None: break;
        }
        return result;
    }
    std::shared_ptr<NodeDataType> dataType(PortType portType, PortIndex portIndex) const override
    {
        switch (portType)
        {
            case PortType::In:
                switch (portIndex)
                {
                    case 0: return MyNodeData().type();
                    case 1: return SimpleNodeData().type();
                }
                break;
            case PortType::Out:
                switch (portIndex)
                {
                    case 0: return MyNodeData().type();
                    case 1: return SimpleNodeData().type();
                }
                break;
            case PortType::None: break;
        }
        // FIXME: control may reach end of non-void function [-Wreturn-type]
        return std::make_shared<NodeDataType>();
    }
    std::shared_ptr<NodeData> outData(PortIndex port) override
    {
        if (port < 1)
            return std::make_shared<MyNodeData>();
        return std::make_shared<SimpleNodeData>();
    }
    void setInData(std::shared_ptr<NodeData>, int) override
    {
        //
    }
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<NaiveDataModel>();
    }
    QWidget *embeddedWidget() override
    {
        return nullptr;
    }
};
