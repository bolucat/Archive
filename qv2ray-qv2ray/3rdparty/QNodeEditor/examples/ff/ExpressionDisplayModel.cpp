#include "ExpressionDisplayModel.hpp"

#include "ExpressionRangeData.hpp"

#include <QtCore/QDebug>
#include <QtCore/QJsonValue>
#include <QtGui/QDoubleValidator>
#include <QtWidgets/QFormLayout>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QWidget>
ExpressionDisplayModel::ExpressionDisplayModel()
{
    _widget = new QWidget();
    // auto pal = _widget->palette();
    // pal.setColor(QPalette::Background, nodeStyle().GradientColor2);
    //_widget->setAutoFillBackground(false);
    //_widget->setPalette(pal);
    auto l = new QFormLayout();
    _variableLabel = new QLineEdit();
    _variableLabel->setReadOnly(true);
    _rangeLabel = new QLineEdit();
    _rangeLabel->setReadOnly(true);
    _rangeLabel->setMaximumWidth(400);
    l->addRow("Variable:", _variableLabel);
    l->addRow("Range:", _rangeLabel);
    _widget->setLayout(l);
}
QJsonObject ExpressionDisplayModel::save() const
{
    QJsonObject modelJson = NodeDataModel::save();
    if (_expression)
        modelJson["expression"] = _expression->expression();
    return modelJson;
}
void ExpressionDisplayModel::restore(QJsonObject const &p)
{
    QJsonValue v = p["expression"];
    if (!v.isUndefined())
    {
        QString str = v.toString();
        std::vector<double> d;
        d.push_back(0.0);
        _expression = std::make_shared<ExpressionRangeData>(str, d);
        _variableLabel->setText(str);
    }
}
unsigned int ExpressionDisplayModel::nPorts(PortType portType) const
{
    unsigned int result = 1;
    switch (portType)
    {
        case PortType::In: result = 1; break;
        case PortType::Out: result = 1;
        default: break;
    }
    return result;
}
std::shared_ptr<NodeDataType> ExpressionDisplayModel::dataType(PortType, PortIndex) const
{
    return ExpressionRangeData().type();
}
std::shared_ptr<NodeData> ExpressionDisplayModel::outData(PortIndex)
{
    return _expression;
}
QString ExpressionDisplayModel::convertRangeToText(std::vector<double> const &range) const
{
    QString result("(");
    for (std::size_t i = 0; i < range.size() - 1; ++i)
    {
        result = result + QString::number(range[i]) + ", ";
    }
    result = result + QString::number(range.back()) + ")";
    return result;
}
void ExpressionDisplayModel::setInData(std::shared_ptr<NodeData> nodeData, PortIndex portIndex)
{
    _expression = std::static_pointer_cast<ExpressionRangeData>(nodeData);
    if (_expression)
    {
        _variableLabel->setText(_expression->expression());
        _rangeLabel->setText(convertRangeToText(_expression->range()));
        emit dataUpdated(0);
    }
}
QWidget *ExpressionDisplayModel::embeddedWidget()
{
    return _widget;
}
