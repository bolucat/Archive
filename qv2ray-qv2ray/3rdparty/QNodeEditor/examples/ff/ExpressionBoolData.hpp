#pragma once
#include <nodes/NodeDataModel>
using QtNodes::NodeDataType;
/// The class can potentially incapsulate any user data which
/// need to be transferred within the Node Editor graph
class ExpressionBoolData : public NodeData
{
  public:
    ExpressionBoolData()
    {
    }
    ExpressionBoolData(QString const &text, std::vector<bool> const &range) : _expression(text), _range(range)
    {
    }
    std::shared_ptr<NodeDataType> type() const override
    {
        return std::make_shared<NodeDataType>("ExpressionBool", "B");
    }
    QString const &expression() const
    {
        return _expression;
    }
    std::vector<bool> const &range() const
    {
        return _range;
    }

  private:
    QString _expression;
    std::vector<bool> _range;
};
