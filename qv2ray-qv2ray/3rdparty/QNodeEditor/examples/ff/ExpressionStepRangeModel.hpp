#pragma once
#include <QtCore/QObject>
#include <iostream>
#include <nodes/NodeDataModel>
class ExpressionRangeData;
class QWidget;
class QLineEdit;
class QSpinBox;
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeDataType;
using QtNodes::NodeValidationState;
using QtNodes::PortIndex;
using QtNodes::PortType;
class ExpressionStepRangeModel : public NodeDataModel
{
    Q_OBJECT
  public:
    ExpressionStepRangeModel();
    virtual ~ExpressionStepRangeModel()
    {
    }

  public:
    QString caption() const override
    {
        return QStringLiteral("Expression Step Range");
    }
    bool captionVisible() const override
    {
        return true;
    }
    QString name() const override
    {
        return QStringLiteral("Expression Step Range");
    }
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<ExpressionStepRangeModel>();
    }

  public:
    QJsonObject save() const override;
    void restore(QJsonObject const &p) override;

  public:
    unsigned int nPorts(PortType portType) const override;
    std::shared_ptr<NodeDataType> dataType(PortType portType, PortIndex portIndex) const override;
    std::shared_ptr<NodeData> outData(PortIndex port) override;
    void setInData(std::shared_ptr<NodeData>, int) override
    {
    }
    QWidget *embeddedWidget() override;
  private slots:
    void onVariableEdited(QString const &string);
    void onRangeEdited(QString const &string);
    void processData();

  private:
    std::vector<double> processRangeText(QString const &numberText, QString const &stepText, int times) const;
    QString convertRangeToText(std::vector<double> const &range) const;

  private:
    std::shared_ptr<ExpressionRangeData> _expression;
    QWidget *_widget;
    QLineEdit *_variableEdit;
    QLineEdit *_numberEdit;
    QSpinBox *_spinBox;
    QLineEdit *_stepEdit;
    QLineEdit *_rangeEdit;
};
