#include "IntervalConditionModel.hpp"

#include "ExpressionBoolData.hpp"
#include "ExpressionRangeData.hpp"

#include <QtCore/QDebug>
#include <QtCore/QJsonValue>
#include <QtGui/QDoubleValidator>
#include <QtWidgets/QFormLayout>
#include <QtWidgets/QLabel>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QWidget>
IntervalConditionModel::IntervalConditionModel()
{
    _widget = new QWidget();
    auto l = new QFormLayout();
    //-------------
    _interval1Label = new QLineEdit();
    _interval1Label->setPlaceholderText("Interval1");
    connect(_interval1Label, &QLineEdit::textChanged, this, &IntervalConditionModel::onIntervalEdited);
    _interval2Label = new QLineEdit();
    _interval2Label->setPlaceholderText("Interval1");
    connect(_interval2Label, &QLineEdit::textChanged, this, &IntervalConditionModel::onIntervalEdited);
    _interval3Label = new QLineEdit();
    _interval3Label->setPlaceholderText("Interval1");
    connect(_interval3Label, &QLineEdit::textChanged, this, &IntervalConditionModel::onIntervalEdited);
    _interval4Label = new QLineEdit();
    _interval4Label->setPlaceholderText("Interval1");
    connect(_interval4Label, &QLineEdit::textChanged, this, &IntervalConditionModel::onIntervalEdited);
#if 0
  _thenLabel = new QLabel();
  _thenLabel->setMargin(3);
  _thenLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);

  //-------------

  _elseLabel = new QLabel();
  _elseLabel->setMargin(3);
  _elseLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);

  //-------------
#endif
    _variableLabel = new QLabel();
    _variableLabel->setMargin(3);
    _variableLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    _variableLabel->setTextInteractionFlags(Qt::TextSelectableByMouse);
    //-------------
    _rangeLabel = new QLabel();
    _rangeLabel->setMargin(3);
    _rangeLabel->setFrameStyle(QFrame::Panel | QFrame::Sunken);
    l->addRow("Interval1:", _interval1Label);
    l->addRow("Interval2:", _interval2Label);
    l->addRow("Interval3:", _interval3Label);
    l->addRow("Interval4:", _interval4Label);
    l->addRow("Expression:", _variableLabel);
    l->addRow("Range:", _rangeLabel);
    _widget->setLayout(l);
}
QJsonObject IntervalConditionModel::save() const
{
    QJsonObject modelJson = NodeDataModel::save();
#if 0
  if (_expression)
    modelJson["expression"] = _expression->expression();
#endif
    return modelJson;
}
void IntervalConditionModel::restore(QJsonObject const &p)
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
unsigned int IntervalConditionModel::nPorts(PortType portType) const
{
    unsigned int result = 1;
    switch (portType)
    {
        case PortType::In: result = 5; break;
        case PortType::Out: result = 1;
        default: break;
    }
    return result;
}
void IntervalConditionModel::onFunctionIndexChanged(int index)
{
    Q_UNUSED(index);
    processData();
}
void IntervalConditionModel::setInData(std::shared_ptr<NodeData> data, PortIndex portIndex)
{
    switch (portIndex)
    {
        case 0:
        {
            _controlInput = std::dynamic_pointer_cast<ExpressionRangeData>(data);
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
        case 3:
        {
            _input3 = std::dynamic_pointer_cast<ExpressionRangeData>(data);
            break;
        }
        case 4:
        {
            _input4 = std::dynamic_pointer_cast<ExpressionRangeData>(data);
            break;
        }
    }
    processData();
}
QWidget *IntervalConditionModel::embeddedWidget()
{
    return _widget;
}
QString IntervalConditionModel::convertBoolRangeToText(std::vector<bool> const &range) const
{
    QString result("(");
    for (std::size_t i = 0; i < range.size() - 1; ++i)
    {
        result = result + (range[i] ? "t" : "f") + ", ";
    }
    result = result + (range.back() ? "t" : "f") + ")";
    return result;
}
QString IntervalConditionModel::convertRangeToText(std::vector<double> const &range) const
{
    QString result("(");
    for (std::size_t i = 0; i < range.size() - 1; ++i)
    {
        result = result + QString::number(range[i]) + ", ";
    }
    result = result + QString::number(range.back()) + ")";
    return result;
}
std::vector<double> IntervalConditionModel::applyFunction(std::vector<double> const &range0, std::vector<double> const &range1,
                                                          std::vector<double> const &range2, std::vector<double> const &range3,
                                                          std::vector<double> const &range4) const
{
    std::vector<double> result;
    for (std::size_t i = 0; i < range0.size(); ++i)
    {
        result.push_back(range0[i] ? range1[i] : range2[i]);
        if (range0[i] >= _intervalRange1 && range0[i] < _intervalRange2)
        {
            result.push_back(range1[i]);
        }
        else if (range0[i] >= _intervalRange2 && range0[i] < _intervalRange3)
        {
            result.push_back(range2[i]);
        }
        else if (range0[i] >= _intervalRange3 && range0[i] < _intervalRange4)
        {
            result.push_back(range3[i]);
        }
        else
        {
            result.push_back(range4[i]);
        }
    }
    return result;
}
void IntervalConditionModel::processData()
{
    if (!processIntervals())
    {
        _expression = std::make_shared<ExpressionRangeData>();
        emit dataInvalidated(0);
        return;
    }
    // std::cout << " borders " << _intervalRange1<< "   " << _intervalRange2<< "
    // " << _intervalRange3 << "   " << _intervalRange4 << std::endl;
    auto n0 = _controlInput.lock(); // control
    auto n1 = _input1.lock();
    auto n2 = _input2.lock();
    auto n3 = _input3.lock();
    auto n4 = _input4.lock();
    if (n0 && n1 && n2 && n3 && n4)
    {
        QString input0 = n0->expression();
        QString input1 = n1->expression();
        QString input2 = n2->expression();
        QString input3 = n3->expression();
        QString input4 = n4->expression();
        std::vector<double> const &inputRange0 = n0->range();
        std::vector<double> const &inputRange1 = n1->range();
        std::vector<double> const &inputRange2 = n2->range();
        std::vector<double> const &inputRange3 = n3->range();
        std::vector<double> const &inputRange4 = n4->range();
#if 0
    _ifLabel->setText(convertBoolRangeToText(inputRange0));
    _input1Label->setText(convertRangeToText(inputRange1));
    _inout2Label->setText(convertRangeToText(inputRange2));
    _inout3Label->setText(convertRangeToText(inputRange3));
#endif
        if ((inputRange1.size() != inputRange2.size()) || (inputRange0.size() != inputRange2.size()) ||
            (inputRange3.size() != inputRange2.size()) || (inputRange4.size() != inputRange2.size()))
        {
            _expression = std::make_shared<ExpressionRangeData>();
            emit dataInvalidated(0);
            return;
        }
        std::vector<double> modifiedRange = applyFunction(inputRange0, inputRange1, inputRange2, inputRange3, inputRange4);
        // QString tt(" ( (%1) ? %2 : %3 ) ");
        QString tt(" ( (%1 >= %2 && %1 < %3) ? %4 : ((%1 >= %3 && %1 < %5) ? %6 : "
                   "((%1 >= %5 && %1 <= %7) ? %8 : %9 ) ) ) ");
        _expression = std::make_shared<ExpressionRangeData>(tt.arg(input0, QString::number(_intervalRange1), QString::number(_intervalRange2),
                                                                   input1, QString::number(_intervalRange3), input2,
                                                                   QString::number(_intervalRange4), input3, input4),
                                                            modifiedRange);
        _variableLabel->setText(_expression->expression());
        _variableLabel->adjustSize();
        _rangeLabel->setText(convertRangeToText(modifiedRange));
        _rangeLabel->adjustSize();
        emit dataUpdated(0);
    }
}
std::shared_ptr<NodeDataType> IntervalConditionModel::dataType(PortType portType, PortIndex index) const
{
    switch (portType)
    {
        case PortType::In: return ExpressionRangeData().type(); break;
        case PortType::Out: return ExpressionRangeData().type(); break;
    }
}
std::shared_ptr<NodeData> IntervalConditionModel::outData(PortIndex)
{
    return _expression;
}
void IntervalConditionModel::onIntervalEdited(QString const &string)
{
    Q_UNUSED(string);
    processData();
}
bool IntervalConditionModel::processIntervals()
{
    bool isOk = true;
    bool ook;
    QString i1 = _interval1Label->text();
    if (!i1.isEmpty())
    {
        _intervalRange1 = i1.toDouble(&ook);
        isOk = isOk && ook;
    }
    QString i2 = _interval2Label->text();
    if (!i2.isEmpty())
    {
        _intervalRange2 = i2.toDouble(&ook);
        isOk = isOk && ook;
    }
    QString i3 = _interval3Label->text();
    if (!i3.isEmpty())
    {
        _intervalRange3 = i3.toDouble(&ook);
        isOk = isOk && ook;
    }
    QString i4 = _interval4Label->text();
    if (!i4.isEmpty())
    {
        _intervalRange4 = i4.toDouble(&ook);
        isOk = isOk && ook;
    }
    return isOk;
}
