#include "PlotModel.hpp"

#include "ExpressionRangeData.hpp"

#include <QtCharts/QChart>
#include <QtCharts/QChartView>
#include <QtCharts/QLineSeries>
#include <QtCore/QDebug>
#include <QtCore/QEvent>
#include <QtCore/QJsonValue>
#include <QtGui/QDoubleValidator>
#include <QtWidgets/QComboBox>
#include <QtWidgets/QFormLayout>
#include <QtWidgets/QGraphicsScene>
#include <QtWidgets/QLabel>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QWidget>
#include <cmath>
using namespace QtCharts;
PlotModel::PlotModel()
{
    QChart *chart = new QChart();
    _chartView = new QChartView(chart);
    _chartView->setRenderHint(QPainter::Antialiasing);
}
void PlotModel::onFunctionIndexChanged(int index)
{
    processData();
}
QJsonObject PlotModel::save() const
{
    QJsonObject modelJson = NodeDataModel::save();
    // if (_expression)
    // modelJson["expression"] = _expression->expression();
    return modelJson;
}
void PlotModel::restore(QJsonObject const &p)
{
    QJsonValue v = p["expression"];
    // if (!v.isUndefined())
    //{
    // QString str = v.toString();
    // std::vector<double> d;
    // d.push_back(0.0);
    //_expression = std::make_shared<ExpressionRangeData>(str, d);
    //_variableLabel->setText(str);
    //}
}
unsigned int PlotModel::nPorts(PortType portType) const
{
    unsigned int result = 1;
    switch (portType)
    {
        case PortType::In: result = 2; break;
        case PortType::Out: result = 0;
        default: break;
    }
    return result;
}
bool PlotModel::eventFilter(QObject *object, QEvent *event)
{
    // if (object == _label)
    //{
    // int w = _label->width();
    // int h = _label->height();
    // if (event->type() == QEvent::Resize)
    //{
    ////QPixmap pixmap(w, h);
    ////_chart->scene()->render(&painter, 0, 0);
    ////if (d)
    ////{
    ////_label->setPixmap(d->pixmap().scaled(w, h, Qt::KeepAspectRatio));
    ////}
    //}
    //}
    return false;
}
std::shared_ptr<NodeDataType> PlotModel::dataType(PortType, PortIndex) const
{
    return ExpressionRangeData().type();
}
std::shared_ptr<NodeData> PlotModel::outData(PortIndex)
{
    return std::shared_ptr<NodeData>();
}
// QString
// PlotModel::
// convertRangeToText(std::vector<double> const &range) const
//{
// QString result("(");
// for (std::size_t i = 0; i < range.size() - 1; ++i)
//{
// result = result + QString::number(range[i]) + ", ";
//}
// result = result + QString::number(range.back()) + ")";
// return result;
//}
void PlotModel::processData()
{
    auto n1 = _input1.lock();
    auto n2 = _input2.lock();
    if (n1 && n2)
    {
        QString input1 = n1->expression();
        QString input2 = n2->expression();
        std::vector<double> const &inputRange1 = n1->range();
        std::vector<double> const &inputRange2 = n2->range();
        if (inputRange1.size() != inputRange2.size())
        {
            emit dataInvalidated(0);
            return;
        }
        QLineSeries *series = new QLineSeries();
        for (std::size_t i = 0; i < inputRange1.size(); ++i)
        {
            series->append(inputRange1[i], inputRange2[i]);
        }
        _chartView->chart()->legend()->hide();
        _chartView->chart()->removeAllSeries();
        _chartView->chart()->addSeries(series);
        _chartView->chart()->createDefaultAxes();
        _chartView->chart()->setTitle("X-Y Plot");
    }
}
void PlotModel::setInData(std::shared_ptr<NodeData> data, PortIndex portIndex)
{
    auto numberData = std::dynamic_pointer_cast<ExpressionRangeData>(data);
    if (portIndex == 0)
    {
        _input1 = numberData;
    }
    else
    {
        _input2 = numberData;
    }
    processData();
}
QWidget *PlotModel::embeddedWidget()
{
    return _chartView;
}
