#pragma once
#include <QtCore/QObject>
#include <functional>
#include <iostream>
#include <nodes/NodeDataModel>
#include <type_traits>
class ExpressionBoolData;
class QWidget;
class QLineEdit;
class QComboBox;
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeDataType;
using QtNodes::NodeValidationState;
using QtNodes::PortIndex;
using QtNodes::PortType;
/// The model dictates the number of inputs and outputs for the Node.
/// In this example it has no logic.
class BoolFunctionModel : public NodeDataModel
{
    Q_OBJECT
  public:
    using BoolFunctionPtr = std::function<bool(bool, bool)>;
    using NameAndBoolFunction = std::tuple<QString, QString, BoolFunctionPtr>;

  public:
    BoolFunctionModel();
    virtual ~BoolFunctionModel()
    {
    }

  public:
    QString caption() const override
    {
        return QStringLiteral("Bool Function");
    }
    bool captionVisible() const override
    {
        return true;
    }
    QString name() const override
    {
        return QStringLiteral("Bool Function");
    }
    std::unique_ptr<NodeDataModel> clone() const
    {
        return std::make_unique<BoolFunctionModel>();
    }

  public:
    QJsonObject save() const override;
    void restore(QJsonObject const &p) override;

  public:
    unsigned int nPorts(PortType portType) const override;
    std::shared_ptr<NodeDataType> dataType(PortType portType, PortIndex portIndex) const override;
    std::shared_ptr<NodeData> outData(PortIndex port) override;
    void setInData(std::shared_ptr<NodeData>, PortIndex) override;
    QWidget *embeddedWidget() override;

  private:
    QString convertBoolRangeToText(std::vector<bool> const &range) const;
    std::vector<bool> applyFunction(std::vector<bool> const &range1, std::vector<bool> const &range2) const;
    void processData();
    void createNameAndBoolFunctions();
  private slots:
    void onFunctionIndexChanged(int index);

  private:
    std::weak_ptr<ExpressionBoolData> _input1;
    std::weak_ptr<ExpressionBoolData> _input2;
    std::shared_ptr<ExpressionBoolData> _expression;
    QWidget *_widget;
    QComboBox *_functionComboBox;
    QLineEdit *_variableLabel;
    QLineEdit *_rangeLabel;
    std::vector<NameAndBoolFunction> _nameAndBoolFunctions;
};
