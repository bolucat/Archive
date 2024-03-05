#pragma once
#include <QtCore/QObject>
#include <functional>
#include <iostream>
#include <nodes/NodeDataModel>
#include <type_traits>
class ExpressionRangeData;
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
class MathFunctionModel : public NodeDataModel
{
    Q_OBJECT
  public:
    using FunctionPtr = std::function<double(double, double)>;
    using NameAndFunction = std::tuple<QString, QString, FunctionPtr>;

  public:
    MathFunctionModel();
    virtual ~MathFunctionModel()
    {
    }

  public:
    QString caption() const override
    {
        return QStringLiteral("Math Function");
    }
    bool captionVisible() const override
    {
        return true;
    }
    QString name() const override
    {
        return QStringLiteral("Math Function");
    }
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<MathFunctionModel>();
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
    QString convertRangeToText(std::vector<double> const &range) const;
    std::vector<double> applyFunction(std::vector<double> const &range) const;
    void processData();
    void createNameAndFunctions();
  private slots:
    void onFunctionIndexChanged(int index);
    void onTextChanged(QString);

  private:
    std::weak_ptr<ExpressionRangeData> _inputExpression;
    std::shared_ptr<ExpressionRangeData> _expression;
    QWidget *_widget;
    QComboBox *_functionComboBox;
    QLineEdit *_secondOperandEdit;
    QLineEdit *_variableLabel;
    QLineEdit *_rangeLabel;
    std::vector<NameAndFunction> _nameAndFunctions;
};
