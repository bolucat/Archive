#include "BoolConditionModel.hpp"
#include "BoolFunctionModel.hpp"
#include "ExpressionConstantModel.hpp"
#include "ExpressionDisplayModel.hpp"
#include "ExpressionSourceModel.hpp"
#include "ExpressionStepRangeModel.hpp"
#include "IfConditionModel.hpp"
#include "IntervalConditionModel.hpp"
#include "MathFunctionModel.hpp"
#include "PlotModel.hpp"

#include <QtWidgets/QApplication>
#include <QtWidgets/QMenuBar>
#include <QtWidgets/QVBoxLayout>
#include <nodes/ConnectionStyle>
#include <nodes/DataModelRegistry>
#include <nodes/FlowScene>
#include <nodes/FlowView>
#include <nodes/NodeData>
using QtNodes::ConnectionStyle;
using QtNodes::DataModelRegistry;
using QtNodes::FlowScene;
using QtNodes::FlowView;
static std::shared_ptr<DataModelRegistry> registerDataModels()
{
    auto ret = std::make_shared<DataModelRegistry>();
    ret->registerModel<ExpressionSourceModel>("IO");
    ret->registerModel<ExpressionConstantModel>("IO");
    ret->registerModel<ExpressionStepRangeModel>("IO");
    ret->registerModel<ExpressionDisplayModel>("IO");
    ret->registerModel<PlotModel>("IO");
    ret->registerModel<MathFunctionModel>("Operations");
    ret->registerModel<BoolConditionModel>("Operations");
    ret->registerModel<BoolFunctionModel>("Operations");
    ret->registerModel<IfConditionModel>("Operations");
    ret->registerModel<IntervalConditionModel>("Operations");
    return ret;
}
static void setStyle()
{
    ConnectionStyle::setConnectionStyle(
        R"(
  {
    "ConnectionStyle": {
      "ConstructionColor": "gray",
      "NormalColor": "black",
      "SelectedColor": "gray",
      "SelectedHaloColor": "deepskyblue",
      "HoveredColor": "deepskyblue",

      "LineWidth": 3.0,
      "ConstructionLineWidth": 2.0,
      "PointDiameter": 10.0,

      "UseDataDefinedColors": true
    }
  }
  )");
}
int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    setStyle();
    QWidget mainWidget;
    auto menuBar = new QMenuBar();
    auto saveAction = menuBar->addAction("Save..");
    auto loadAction = menuBar->addAction("Load..");
    QVBoxLayout *l = new QVBoxLayout(&mainWidget);
    l->addWidget(menuBar);
    auto scene = new FlowScene(registerDataModels());
    l->addWidget(new FlowView(scene));
    l->setContentsMargins(0, 0, 0, 0);
    l->setSpacing(0);
    QObject::connect(saveAction, &QAction::triggered, scene, &FlowScene::save);
    QObject::connect(loadAction, &QAction::triggered, scene, &FlowScene::load);
    mainWidget.setWindowTitle("Field Function constructor");
    mainWidget.resize(800, 600);
    mainWidget.showNormal();
    return app.exec();
}
