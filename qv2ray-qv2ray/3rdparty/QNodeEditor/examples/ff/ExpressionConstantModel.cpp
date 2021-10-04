#include "ExpressionConstantModel.hpp"

#include "ExpressionRangeData.hpp"

#include <QtCore/QDebug>
#include <QtCore/QJsonValue>
#include <QtGui/QDoubleValidator>
#include <QtWidgets/QFormLayout>
#include <QtWidgets/QHBoxLayout>
#include <QtWidgets/QLineEdit>
#include <QtWidgets/QSpinBox>
#include <QtWidgets/QWidget>
#include <algorithm>
ExpressionConstantModel::ExpressionConstantModel()
{
    _widget = new QWidget();
    auto l = new QFormLayout();
    _variableEdit = new QLineEdit();
    _variableEdit->setPlaceholderText("Variable name");
    _numberEdit = new QLineEdit();
    _numberEdit->setPlaceholderText("Number");
    _numberEdit->setValidator(new QDoubleValidator());
    _numberEdit->setText("0.0");
    _spinBox = new QSpinBox();
    _spinBox->setMinimum(1);
    _spinBox->setMaximum(100);
    _spinBox->setValue(1);
    auto ll = new QHBoxLayout();
    ll->addWidget(_numberEdit);
    ll->addWidget(_spinBox);
    _rangeEdit = new QLineEdit();
    _rangeEdit->setReadOnly(true);
    l->addRow("Variable", _variableEdit);
    l->addRow("Constant x N", ll);
    l->addRow("Range", _rangeEdit);
    _widget->setLayout(l);
    connect(_variableEdit, &QLineEdit::textChanged, this, &ExpressionConstantModel::onVariableEdited);
    connect(_numberEdit, &QLineEdit::textChanged, this, &ExpressionConstantModel::onVariableEdited);
    connect(_spinBox, SIGNAL(valueChanged(QString)), this, SLOT(onVariableEdited(QString)));
}
QJsonObject ExpressionConstantModel::save() const
{
    QJsonObject modelJson = NodeDataModel::save();
    if (_expression)
        modelJson["expression"] = _expression->expression();
    return modelJson;
}
void ExpressionConstantModel::restore(QJsonObject const &p)
{
    QJsonValue v = p["expression"];
    if (!v.isUndefined())
    {
        QString str = v.toString();
        std::vector<double> range;
        range.push_back(0.0);
        _expression = std::make_shared<ExpressionRangeData>(str, range);
        _variableEdit->setText(str);
    }
}
unsigned int ExpressionConstantModel::nPorts(PortType portType) const
{
    unsigned int result = 1;
    switch (portType)
    {
        case PortType::In: result = 0; break;
        case PortType::Out: result = 1;
        default: break;
    }
    return result;
}
std::vector<double> ExpressionConstantModel::processRangeText(QString const &numberText, int times) const
{
    std::vector<double> result;
    QStringList numbers = numberText.split(",", QString::SkipEmptyParts);
    bool ok = true;
    double d = numberText.toDouble(&ok);
    if (!ok)
        return result;
    result.resize(times);
    std::fill_n(result.begin(), times, d);
    return result;
}
QString ExpressionConstantModel::convertRangeToText(std::vector<double> const &range) const
{
    QString result("(");
    for (std::size_t i = 0; i < range.size() - 1; ++i)
    {
        result = result + QString::number(range[i]) + ", ";
    }
    result = result + QString::number(range.back()) + ")";
    return result;
}
void ExpressionConstantModel::processData()
{
    bool ok = false;
    QString text = _variableEdit->text();
    QString prefix("${%1}");
    std::vector<double> range = processRangeText(_numberEdit->text(), _spinBox->value());
    if (!text.isEmpty() && (range.size() > 0))
    {
        _expression = std::make_shared<ExpressionRangeData>(prefix.arg(text), range);
        _rangeEdit->setText(convertRangeToText(range));
        emit dataUpdated(0);
    }
    else
    {
        emit dataInvalidated(0);
    }
}
void ExpressionConstantModel::onVariableEdited(QString const &string)
{
    Q_UNUSED(string);
    processData();
}
void ExpressionConstantModel::onRangeEdited(QString const &string)
{
    Q_UNUSED(string);
    processData();
}
std::shared_ptr<NodeDataType> ExpressionConstantModel::dataType(PortType, PortIndex) const
{
    return ExpressionRangeData().type();
}
std::shared_ptr<NodeData> ExpressionConstantModel::outData(PortIndex)
{
    return _expression;
}
QWidget *ExpressionConstantModel::embeddedWidget()
{
    return _widget;
}
