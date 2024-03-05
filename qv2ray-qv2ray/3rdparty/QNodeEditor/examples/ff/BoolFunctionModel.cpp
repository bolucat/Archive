#include "BoolFunctionModel.hpp"

#include "ExpressionBoolData.hpp"

#include <QtCore/QDebug>
#include <QtCore/QJsonValue>
#include <QtGui/QDoubleValidator>
#include <QtWidgets/QComboBox>
#include <QtWidgets/QFormLayout>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QWidget>
#include <cmath>
BoolFunctionModel::BoolFunctionModel()
{
    createNameAndBoolFunctions();
    _widget = new QWidget();
    auto l = new QFormLayout();
    _functionComboBox = new QComboBox;
    for (auto const &f : _nameAndBoolFunctions)
    {
        _functionComboBox->addItem(std::get<0>(f));
    }
    _variableLabel = new QLineEdit();
    _variableLabel->setReadOnly(true);
    _rangeLabel = new QLineEdit();
    _variableLabel->setReadOnly(true);
    _rangeLabel->setMaximumWidth(200);
    l->addRow("Function:", _functionComboBox);
    l->addRow("Variable:", _variableLabel);
    l->addRow("Range:", _rangeLabel);
    _widget->setLayout(l);
    connect(_functionComboBox, SIGNAL(currentIndexChanged(int)), this, SLOT(onFunctionIndexChanged(int)));
}
void BoolFunctionModel::onFunctionIndexChanged(int index)
{
    processData();
}
QJsonObject BoolFunctionModel::save() const
{
    QJsonObject modelJson = NodeDataModel::save();
    if (_expression)
        modelJson["expression"] = _expression->expression();
    return modelJson;
}
void BoolFunctionModel::restore(QJsonObject const &p)
{
    QJsonValue v = p["expression"];
    if (!v.isUndefined())
    {
        QString str = v.toString();
        std::vector<bool> d;
        d.push_back(true);
        _expression = std::make_shared<ExpressionBoolData>(str, d);
        _variableLabel->setText(str);
    }
}
unsigned int BoolFunctionModel::nPorts(PortType portType) const
{
    unsigned int result = 1;
    switch (portType)
    {
        case PortType::In: result = 2; break;
        case PortType::Out: result = 1;
        default: break;
    }
    return result;
}
std::shared_ptr<NodeDataType> BoolFunctionModel::dataType(PortType, PortIndex) const
{
    return ExpressionBoolData().type();
}
std::shared_ptr<NodeData> BoolFunctionModel::outData(PortIndex)
{
    return _expression;
}
QString BoolFunctionModel::convertBoolRangeToText(std::vector<bool> const &range) const
{
    QString result("(");
    for (std::size_t i = 0; i < range.size() - 1; ++i)
    {
        result = result + (range[i] ? "t" : "f") + ", ";
    }
    result = result + (range.back() ? "t" : "f") + ")";
    return result;
}
std::vector<bool> BoolFunctionModel::applyFunction(std::vector<bool> const &range1, std::vector<bool> const &range2) const
{
    std::vector<bool> result;
    BoolFunctionPtr const &f = std::get<2>(_nameAndBoolFunctions[_functionComboBox->currentIndex()]);
    for (std::size_t i = 0; i < range1.size(); ++i)
    {
        result.push_back(f(range1[i], range2[i]));
    }
    return result;
}
void BoolFunctionModel::createNameAndBoolFunctions()
{
    _nameAndBoolFunctions.push_back(
        std::make_tuple(QString("&&"), QString("( %1 && %2 )"), static_cast<BoolFunctionPtr>([](bool a, bool b) { return (a && b); })));
    _nameAndBoolFunctions.push_back(
        std::make_tuple(QString(" || "), QString("( %1 || %2 )"), static_cast<BoolFunctionPtr>([](bool a, bool b) { return (a || b); })));
}
void BoolFunctionModel::processData()
{
    auto inputExpression1 = _input1.lock();
    auto inputExpression2 = _input2.lock();
    if (inputExpression1 && inputExpression2)
    {
        QString input1 = inputExpression1->expression();
        QString input2 = inputExpression2->expression();
        std::vector<bool> const &inputRange1 = inputExpression1->range();
        std::vector<bool> const &inputRange2 = inputExpression2->range();
        std::vector<bool> modifiedRange = applyFunction(inputRange1, inputRange2);
        QString tt = std::get<1>(_nameAndBoolFunctions[_functionComboBox->currentIndex()]);
        _expression = std::make_shared<ExpressionBoolData>(tt.arg(input1).arg(input2), modifiedRange);
        _variableLabel->setText(_expression->expression());
        _variableLabel->adjustSize();
        _rangeLabel->setText(convertBoolRangeToText(modifiedRange));
        _rangeLabel->adjustSize();
        emit dataUpdated(0);
    }
}
void BoolFunctionModel::setInData(std::shared_ptr<NodeData> nodeData, PortIndex portIndex)
{
    auto data = std::dynamic_pointer_cast<ExpressionBoolData>(nodeData);
    if (portIndex == 0)
    {
        _input1 = data;
    }
    else
    {
        _input2 = data;
    }
    processData();
}
QWidget *BoolFunctionModel::embeddedWidget()
{
    return _widget;
}
