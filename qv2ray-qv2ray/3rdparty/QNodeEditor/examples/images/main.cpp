#include "ImageLoaderModel.hpp"
#include "ImageShowModel.hpp"

#include <QtWidgets/QApplication>
#include <nodes/FlowScene>
#include <nodes/FlowView>
#include <nodes/NodeData>
using QtNodes::DataModelRegistry;
using QtNodes::FlowScene;
using QtNodes::FlowView;
static std::shared_ptr<DataModelRegistry> registerDataModels()
{
    auto ret = std::make_shared<DataModelRegistry>();
    ret->registerModel<ImageShowModel>();
    ret->registerModel<ImageLoaderModel>();
    return ret;
}
int main(int argc, char *argv[])
{
    QApplication app(argc, argv);
    FlowScene scene(registerDataModels());
    FlowView view(&scene);
    view.setWindowTitle("Node-based flow editor");
    view.resize(800, 600);
    view.show();
    return app.exec();
}
