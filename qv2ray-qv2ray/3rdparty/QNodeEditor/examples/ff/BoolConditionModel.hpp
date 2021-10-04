#pragma once
#include <QtCore/QObject>
#include <functional>
#include <iostream>
#include <nodes/NodeDataModel>
class ExpressionRangeData;
class ExpressionBoolData;
class QLabel;
class QComboBox;
class QWidget;
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeDataType;
using QtNodes::NodeValidationState;
using QtNodes::PortIndex;
using QtNodes::PortType;
using BoolFunctionPtr = std::function<bool(double, double)>;
// name , template , function
using NameAndBoolFunction = std::tuple<QString, QString, BoolFunctionPtr>;
/// The model dictates the number of inputs and outputs for the Node.
/// In this example it has no logic.
class BoolConditionModel : public NodeDataModel
{
    Q_OBJECT
  public:
    BoolConditionModel();
    virtual ~BoolConditionModel()
    {
    }

  public:
    QString caption() const override
    {
        return QStringLiteral("Bool Condition");
    }
    bool captionVisible() const override
    {
        return true;
    }
    QString name() const override
    {
        return QStringLiteral("Bool Condition");
    }
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<BoolConditionModel>();
    }

  public:
    QJsonObject save() const override;
    void restore(QJsonObject const &p) override;

  public:
    unsigned int nPorts(PortType portType) const override;
    std::shared_ptr<NodeDataType> dataType(PortType portType, PortIndex portIndex) const override;
    std::shared_ptr<NodeData> outData(PortIndex port) override;
    void setInData(std::shared_ptr<NodeData> data, PortIndex portIndex) override;
    QWidget *embeddedWidget() override;

  private:
    void createNameAndBoolFunctions();
  private slots:
    void onFunctionIndexChanged(int index);
    void processData();
    std::vector<bool> applyFunction(std::vector<double> const &range1, std::vector<double> const &range2) const;
    QString convertRangeToText(std::vector<bool> const &range) const;

  private:
    std::weak_ptr<ExpressionRangeData> _input1;
    std::weak_ptr<ExpressionRangeData> _input2;
    std::shared_ptr<ExpressionBoolData> _expression;
    std::vector<NameAndBoolFunction> _nameAndBoolFunctions;
    QWidget *_widget;
    QComboBox *_functionComboBox;
    QLabel *_variableLabel;
    QLabel *_rangeLabel;
};
