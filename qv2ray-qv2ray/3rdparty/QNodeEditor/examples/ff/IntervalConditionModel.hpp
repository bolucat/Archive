#pragma once
#include <QtCore/QObject>
#include <functional>
#include <iostream>
#include <nodes/NodeDataModel>
class ExpressionRangeData;
class ExpressionBoolData;
class QLabel;
class QWidget;
class QLineEdit;
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeDataType;
using QtNodes::NodeValidationState;
using QtNodes::PortIndex;
using QtNodes::PortType;
using BoolFunctionPtr = std::function<bool(double, double)>;
// name , template , function
using NameAndBoolFunction = std::tuple<QString, QString, BoolFunctionPtr>;
class IntervalConditionModel : public NodeDataModel
{
    Q_OBJECT
  public:
    IntervalConditionModel();
    virtual ~IntervalConditionModel()
    {
    }

  public:
    bool portCaptionVisible(PortType, PortIndex) const override
    {
        return true;
    }
    QString portCaption(PortType portType, PortIndex portIndex) const override
    {
        switch (portType)
        {
            case PortType::In:
                if (portIndex == 0)
                    return QStringLiteral("Control E");
                else
                    return QStringLiteral("E");
                break;
            case PortType::Out: return QStringLiteral("E");
            default: break;
        }
        return QString();
    }
    QString caption() const override
    {
        return QStringLiteral("Interval Condition");
    }
    bool captionVisible() const override
    {
        return true;
    }
    QString name() const override
    {
        return QStringLiteral("Interval Condition");
    }
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<IntervalConditionModel>();
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
  private slots:
    void onFunctionIndexChanged(int index);
    void processData();
    std::vector<double> applyFunction(std::vector<double> const &range0, std::vector<double> const &range1, std::vector<double> const &range2,
                                      std::vector<double> const &range3, std::vector<double> const &range4) const;
    QString convertBoolRangeToText(std::vector<bool> const &range) const;
    QString convertRangeToText(std::vector<double> const &range) const;
    void onIntervalEdited(QString const &string);
    bool processIntervals();

  private:
    std::weak_ptr<ExpressionRangeData> _controlInput;
    std::weak_ptr<ExpressionRangeData> _input1;
    std::weak_ptr<ExpressionRangeData> _input2;
    std::weak_ptr<ExpressionRangeData> _input3;
    std::weak_ptr<ExpressionRangeData> _input4;
    std::shared_ptr<ExpressionRangeData> _expression;
    std::vector<NameAndBoolFunction> _nameAndBoolFunctions;
    QWidget *_widget;
    QLabel *_controlLabel;
    QLineEdit *_interval1Label;
    QLineEdit *_interval2Label;
    QLineEdit *_interval3Label;
    QLineEdit *_interval4Label;
    QLabel *_variableLabel; // output
    QLabel *_rangeLabel;
    double _intervalRange1;
    double _intervalRange2;
    double _intervalRange3;
    double _intervalRange4;
};
