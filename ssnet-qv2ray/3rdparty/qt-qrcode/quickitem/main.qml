/****************************************************************************
 *
 * Copyright (c) 2015 Daniel San, All rights reserved.
 * 
 *    Contact: daniel.samrocha@gmail.com
 *       File: main.qml
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

import QtQuick 2.4
import QtQuick.Controls 1.3
import QtQuick.Layouts 1.1

import QtQrCode.Component 1.0

ApplicationWindow {
    id: window
    title: "Hello QR Code"
    visible: true
    width: 640
    height: 480

    ColumnLayout {
        spacing: 20
        anchors.margins: 40
        anchors.fill: parent
        Text {
            text: "The content of the QR code is:<br><b>" + window.title + "</b>"
            font.pointSize: 25
            wrapMode: Text.WordWrap
            horizontalAlignment: Text.AlignHCenter
            Layout.fillWidth: true
        }
        QtQrCode {
            data: window.title
            background: "transparent"
            Layout.fillWidth: true
            Layout.fillHeight: true
        } // QtQrCode
    } // ColumnLayout
} // Window
