#include "models.hpp"

#include <QtWidgets/QApplication>
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
    ret->registerModel<NaiveDataModel>();
    /*
       We could have more models registered.
       All of them become items in the context meny of the scene.

       ret->registerModel<AnotherDataModel>();
       ret->registerModel<OneMoreDataModel>();

     */
    return ret;
}
static void setStyle()
{
    ConnectionStyle::setConnectionStyle(
        R"(
  {
    "ConnectionStyle": {
      "UseDataDefinedColors": true
    }
  }
  )");
}
//------------------------------------------------------------------------------
int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    setStyle();
    FlowScene scene(registerDataModels());
    FlowView view(&scene);
    view.setWindowTitle("Node-based flow editor");
    view.resize(800, 600);
    view.show();
    return app.exec();
}
