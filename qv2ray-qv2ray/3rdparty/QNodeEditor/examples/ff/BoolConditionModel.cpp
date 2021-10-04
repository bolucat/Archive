#include "BoolConditionModel.hpp"

#include "ExpressionBoolData.hpp"
#include "ExpressionRangeData.hpp"

#include <QtCore/QDebug>
#include <QtCore/QJsonValue>
#include <QtGui/QDoubleValidator>
#include <QtWidgets/QComboBox>
#include <QtWidgets/QFormLayout>
#include <QtWidgets/QLabel>
#include <QtWidgets/QWidget>
BoolConditionModel::BoolConditionModel()
{
    createNameAndBoolFunctions();
    _widget = new QWidget();
    auto l = new QFormLayout();
    _functionComboBox = new QComboBox;
    for (auto const &f : _nameAndBoolFunctions)
    {
        _functionComboBox->addItem(std::get<0>(f));
    }
    //-------------
    _variableLabel = new QLabel();
    _variableLabel->setMargin(3);
    _variableLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    //-------------
    _rangeLabel = new QLabel();
    _rangeLabel->setMargin(3);
    _rangeLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    l->addRow("Function:", _functionComboBox);
    l->addRow("Expression:", _variableLabel);
    l->addRow("Range:", _rangeLabel);
    _widget->setLayout(l);
    connect(_functionComboBox, SIGNAL(currentIndexChanged(int)), this, SLOT(onFunctionIndexChanged(int)));
}
QJsonObject BoolConditionModel::save() const
{
    QJsonObject modelJson = NodeDataModel::save();
#if 0
  if (_expression)
    modelJson["expression"] = _expression->expression();
#endif
    return modelJson;
}
void BoolConditionModel::restore(QJsonObject const &p)
{
    QJsonValue v = p["expression"];
#if 0
  if (!v.isUndefined())
  {
    QString str = v.toString();

    _expression = std::make_shared<ExpressionRangeData>(str);
    _lineEdit->setText(str);
  }
#endif
}
unsigned int BoolConditionModel::nPorts(PortType portType) const
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
void BoolConditionModel::onFunctionIndexChanged(int index)
{
    Q_UNUSED(index);
    processData();
    // bool ok = false;
    // QString text = _comboBox->currentText();
    // if (!text.isEmpty())
    //{
    //_boolCondition = text;
    // evaluate();
    // emit dataUpdated(0);
    //}
    // else
    //{
    // emit dataInvalidated(0);
    //}
}
void BoolConditionModel::setInData(std::shared_ptr<NodeData> data, PortIndex portIndex)
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
QWidget *BoolConditionModel::embeddedWidget()
{
    return _widget;
}
void BoolConditionModel::createNameAndBoolFunctions()
{
    _nameAndBoolFunctions.push_back(
        std::make_tuple(QString(" < "), QString("( %1 < %2 )"), static_cast<BoolFunctionPtr>([](double a, double b) { return (a < b); })));
    _nameAndBoolFunctions.push_back(
        std::make_tuple(QString(" > "), QString("( %1 > %2 )"), static_cast<BoolFunctionPtr>([](double a, double b) { return (a > b); })));
    _nameAndBoolFunctions.push_back(
        std::make_tuple(QString(" == "), QString("( %1 == %2 )"), static_cast<BoolFunctionPtr>([](double a, double b) { return (a == b); })));
    _nameAndBoolFunctions.push_back(
        std::make_tuple(QString(" != "), QString("( %1 != %2 )"), static_cast<BoolFunctionPtr>([](double a, double b) { return (a != b); })));
}
QString BoolConditionModel::convertRangeToText(std::vector<bool> const &range) const
{
    QString result("(");
    for (std::size_t i = 0; i < range.size() - 1; ++i)
    {
        result = result + (range[i] ? "t" : "f") + ", ";
    }
    result = result + (range.back() ? "t" : "f") + ")";
    return result;
}
std::vector<bool> BoolConditionModel::applyFunction(std::vector<double> const &range1, std::vector<double> const &range2) const
{
    std::vector<bool> result;
    BoolFunctionPtr const &f = std::get<2>(_nameAndBoolFunctions[_functionComboBox->currentIndex()]);
    for (std::size_t i = 0; i < range1.size(); ++i)
    {
        result.push_back(f(range1[i], range2[i]));
    }
    return result;
}
void BoolConditionModel::processData()
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
            _expression = std::make_shared<ExpressionBoolData>();
            emit dataInvalidated(0);
            return;
        }
        std::vector<bool> modifiedRange = applyFunction(inputRange1, inputRange2);
        QString tt = std::get<1>(_nameAndBoolFunctions[_functionComboBox->currentIndex()]);
        _expression = std::make_shared<ExpressionBoolData>(tt.arg(input1).arg(input2), modifiedRange);
        _variableLabel->setText(_expression->expression());
        _variableLabel->adjustSize();
        _rangeLabel->setText(convertRangeToText(modifiedRange));
        _rangeLabel->adjustSize();
        emit dataUpdated(0);
    }
}
std::shared_ptr<NodeDataType> BoolConditionModel::dataType(PortType portType, PortIndex) const
{
    switch (portType)
    {
        case PortType::In: return ExpressionRangeData().type(); break;
        case PortType::Out: return ExpressionBoolData().type(); break;
    }
}
std::shared_ptr<NodeData> BoolConditionModel::outData(PortIndex)
{
    return _expression;
}
