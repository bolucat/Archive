/****************************************************************************
 *
 * Copyright (c) 2015 Daniel San, All rights reserved.
 * 
 *    Contact: daniel.samrocha@gmail.com
 *       File: MainWindow.cpp
 *     Author: daniel
 * Created on: 03/02/2015
 *    Version: 
 *
 * This file is part of the Qt QRCode library.
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 3.0 of the License, or (at your option) any later version.
 * 
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU
 * Lesser General Public License for more details.
 * 
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library.
 *
 ***************************************************************************/

#include "MainWindow.hpp"
#include "ui_MainWindow.h"

#include "QtQrCodeWidget.hpp"

#include <QDebug>

MainWindow::MainWindow(QWidget *parent) :
    QMainWindow(parent),
    ui(new Ui::MainWindow)
{
    ui->setupUi(this);

    QtQrCodeWidget *qrCodeWidget = new QtQrCodeWidget(this);
    qrCodeWidget->setBackground(Qt::transparent);
    qrCodeWidget->setData(windowTitle().toUtf8());
    qrCodeWidget->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Expanding);

    ui->centralWidget->layout()->addWidget(qrCodeWidget);
}

MainWindow::~MainWindow()
{
    delete ui;
}
