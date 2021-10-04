#include "IfConditionModel.hpp"

#include "ExpressionBoolData.hpp"
#include "ExpressionRangeData.hpp"

#include <QtCore/QDebug>
#include <QtCore/QJsonValue>
#include <QtGui/QDoubleValidator>
#include <QtWidgets/QFormLayout>
#include <QtWidgets/QLabel>
#include <QtWidgets/QWidget>
IfConditionModel::IfConditionModel()
{
    _widget = new QWidget();
    auto l = new QFormLayout();
    //-------------
    _ifLabel = new QLabel();
    _ifLabel->setMargin(3);
    _ifLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    //-------------
    _thenLabel = new QLabel();
    _thenLabel->setMargin(3);
    _thenLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    //-------------
    _elseLabel = new QLabel();
    _elseLabel->setMargin(3);
    _elseLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    //-------------
    _variableLabel = new QLabel();
    _variableLabel->setMargin(3);
    _variableLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    //-------------
    _rangeLabel = new QLabel();
    _rangeLabel->setMargin(3);
    _rangeLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    l->addRow("If:", _ifLabel);
    l->addRow("Then:", _thenLabel);
    l->addRow("Else:", _elseLabel);
    l->addRow("Expression:", _variableLabel);
    l->addRow("Range:", _rangeLabel);
    _widget->setLayout(l);
}
QJsonObject IfConditionModel::save() const
{
    QJsonObject modelJson = NodeDataModel::save();
#if 0
  if (_expression)
    modelJson["expression"] = _expression->expression();
#endif
    return modelJson;
}
void IfConditionModel::restore(QJsonObject const &p)
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
unsigned int IfConditionModel::nPorts(PortType portType) const
{
    unsigned int result = 1;
    switch (portType)
    {
        case PortType::In: result = 3; break;
        case PortType::Out: result = 1;
        default: break;
    }
    return result;
}
void IfConditionModel::onFunctionIndexChanged(int index)
{
    Q_UNUSED(index);
    processData();
}
void IfConditionModel::setInData(std::shared_ptr<NodeData> data, PortIndex portIndex)
{
    switch (portIndex)
    {
        case 0:
        {
            _input0 = std::dynamic_pointer_cast<ExpressionBoolData>(data);
            break;
        }
        case 1:
        {
            _input1 = std::dynamic_pointer_cast<ExpressionRangeData>(data);
            break;
        }
        case 2:
        {
            _input2 = std::dynamic_pointer_cast<ExpressionRangeData>(data);
            break;
        }
    }
    processData();
}
QWidget *IfConditionModel::embeddedWidget()
{
    return _widget;
}
QString IfConditionModel::convertBoolRangeToText(std::vector<bool> const &range) const
{
    QString result("(");
    for (std::size_t i = 0; i < range.size() - 1; ++i)
    {
        result = result + (range[i] ? "t" : "f") + ", ";
    }
    result = result + (range.back() ? "t" : "f") + ")";
    return result;
}
QString IfConditionModel::convertRangeToText(std::vector<double> const &range) const
{
    QString result("(");
    for (std::size_t i = 0; i < range.size() - 1; ++i)
    {
        result = result + QString::number(range[i]) + ", ";
    }
    result = result + QString::number(range.back()) + ")";
    return result;
}
std::vector<double> IfConditionModel::applyFunction(std::vector<bool> const &range0, std::vector<double> const &range1,
                                                    std::vector<double> const &range2) const
{
    std::vector<double> result;
    for (std::size_t i = 0; i < range0.size(); ++i)
    {
        result.push_back(range0[i] ? range1[i] : range2[i]);
    }
    return result;
}
void IfConditionModel::processData()
{
    auto n0 = _input0.lock();
    auto n1 = _input1.lock();
    auto n2 = _input2.lock();
    if (n0 && n1 && n2)
    {
        QString input0 = n0->expression();
        QString input1 = n1->expression();
        QString input2 = n2->expression();
        std::vector<bool> const &inputRange0 = n0->range();
        std::vector<double> const &inputRange1 = n1->range();
        std::vector<double> const &inputRange2 = n2->range();
        _ifLabel->setText(convertBoolRangeToText(inputRange0));
        _thenLabel->setText(convertRangeToText(inputRange1));
        _elseLabel->setText(convertRangeToText(inputRange2));
        if ((inputRange1.size() != inputRange2.size()) || (inputRange0.size() != inputRange2.size()))
        {
            _expression = std::make_shared<ExpressionRangeData>();
            emit dataInvalidated(0);
            return;
        }
        std::vector<double> modifiedRange = applyFunction(inputRange0, inputRange1, inputRange2);
        QString tt(" ( (%1) ? %2 : %3 ) ");
        _expression = std::make_shared<ExpressionRangeData>(tt.arg(input0, input1, input2), modifiedRange);
        _variableLabel->setText(_expression->expression());
        _variableLabel->adjustSize();
        _rangeLabel->setText(convertRangeToText(modifiedRange));
        _rangeLabel->adjustSize();
        emit dataUpdated(0);
    }
}
std::shared_ptr<NodeDataType> IfConditionModel::dataType(PortType portType, PortIndex index) const
{
    switch (portType)
    {
        case PortType::In:
            if (index == 0)
                return ExpressionBoolData().type();
            else
                return ExpressionRangeData().type();
            break;
        case PortType::Out: return ExpressionRangeData().type(); break;
    }
}
std::shared_ptr<NodeData> IfConditionModel::outData(PortIndex)
{
    return _expression;
}
