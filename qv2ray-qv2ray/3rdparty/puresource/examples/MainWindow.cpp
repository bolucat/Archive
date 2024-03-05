#include "MainWindow.h"

#include "PureJson.hpp"
#include "ui_MainWindow.h"

MainWindow::MainWindow(QWidget *parent) : QMainWindow(parent), ui(new Ui::MainWindow) { ui->setupUi(this); }

MainWindow::~MainWindow() { delete ui; }

void MainWindow::on_pushButton_clicked() { ui->targetTxt->setPlainText(RemoveComment(ui->sourceTxt->toPlainText())); }
