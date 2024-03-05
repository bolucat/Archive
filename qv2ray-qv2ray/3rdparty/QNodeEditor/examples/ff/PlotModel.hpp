#pragma once
#include <QtCore/QObject>
#include <functional>
#include <iostream>
#include <nodes/NodeDataModel>
#include <type_traits>
class ExpressionRangeData;
class QWidget;
class QLabel;
class QComboBox;
namespace QtCharts
{
    class QChartView;
}
using QtNodes::NodeData;
using QtNodes::NodeDataModel;
using QtNodes::NodeDataType;
using QtNodes::NodeValidationState;
using QtNodes::PortIndex;
using QtNodes::PortType;
using FunctionPtr = std::function<double(double)>;
// std::add_pointer<double(double)>::type;
using NameAndFunction = std::tuple<QString, QString, FunctionPtr>;
/// The model dictates the number of inputs and outputs for the Node.
/// In this example it has no logic.
class PlotModel : public NodeDataModel
{
    Q_OBJECT
  public:
    PlotModel();
    virtual ~PlotModel()
    {
    }

  public:
    QString caption() const override
    {
        return QStringLiteral("Plot");
    }
    bool captionVisible() const override
    {
        return true;
    }
    QString name() const override
    {
        return QStringLiteral("Plot");
    }
    std::unique_ptr<NodeDataModel> clone() const override
    {
        return std::make_unique<PlotModel>();
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
    bool resizable() const override
    {
        return true;
    }

  protected:
    bool eventFilter(QObject *object, QEvent *event) override;

  private:
    void processData();
  private slots:
    void onFunctionIndexChanged(int index);

  private:
    std::weak_ptr<ExpressionRangeData> _input1;
    std::weak_ptr<ExpressionRangeData> _input2;
    QtCharts::QChartView *_chartView;
};
