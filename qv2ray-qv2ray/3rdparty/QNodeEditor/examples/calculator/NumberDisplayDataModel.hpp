#pragma once
#include <QtCore/QObject>
#include <QtWidgets/QLabel>
#include <iostream>
#include <nodes/NodeDataModel>
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeDataType;
using QtNodes::NodeValidationState;
using QtNodes::PortIndex;
using QtNodes::PortType;
/// The model dictates the number of inputs and outputs for the Node.
/// In this example it has no logic.
class NumberDisplayDataModel : public NodeDataModel
{
    Q_OBJECT
  public:
    NumberDisplayDataModel();
    virtual ~NumberDisplayDataModel()
    {
    }

  public:
    QString caption() const override
    {
        return QStringLiteral("Result");
    }
    bool captionVisible() const override
    {
        return false;
    }
    QString name() const override
    {
        return QStringLiteral("Result");
    }

  public:
    unsigned int nPorts(PortType portType) const override;
    std::shared_ptr<NodeDataType> dataType(PortType portType, PortIndex portIndex) const override;
    std::shared_ptr<NodeData> outData(PortIndex port) override;
    void setInData(std::shared_ptr<NodeData> data, int) override;
    QWidget *embeddedWidget() override
    {
        return _label;
    }
    NodeValidationState validationState() const override;
    QString validationMessage() const override;
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<NumberDisplayDataModel>();
    }

  private:
    NodeValidationState modelValidationState = NodeValidationState::Warning;
    QString modelValidationError = QStringLiteral("Missing or incorrect inputs");
    QLabel *_label;
};
