# Qt QRCode
Qt/C++ library for encoding and visualization of data in a QR Code symbol. This library consists of a Qt wrapper for libqrencode, and Qt components that are able to visualize the result.

The official libqrencode site is [this](http://fukuchi.org/works/qrencode/). And its official repository can be found [here](https://github.com/fukuchi/libqrencode).

## Author

This Qt library wrapper was developed by **Daniel San F. da Rocha**. Any questions or suggestions don't hesitate to contact me, I'll be very happy to receive messages and suggestions to this project.

## License

Copyright (C) 2015 [Daniel San F. da Rocha](http://www.danielsan.com.br)

Copyright (C) 2006-2012 [Kentaro Fukuchi](http://fukuchi.org/)

> This library is free software; you can redistribute it and/or modify it under the terms of the GNU Lesser General Public License as published by the Free Software Foundation; either version 3.0 of the License, or (at your option) any later version.
>
> This library is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Lesser General Public License for more details.
>
> You should have received a copy of the [GNU Lesser General Public License](https://github.com/danielsanfr/qt-qrcode/blob/master/LICENSE) along with this library.

## Features

* Qt wrapper to libqrencode (QtQrCode and QtQrCodePainter)
* Qt QuickItem (QtQrCodeQuickItem)
* Qt Widget (QtQrCodeWidget)

## How to build and use in you project

* You only need [Qt 5.3+](https://www.qt.io/download/) and download the source code:

 > git clone --recursive https://github.com/danielsanfr/qt-qrcode.git

* Build the lib project using Qt Creator (without QTQRCODE_PLUS_FEATURES define), you will be able to use only the class QtQrCode.
* Build the lib project using Qt Creator (without QTQRCODE_PLUS_FEATURES define), you will be able to use all the classes contained in this library.
* Include `LIBS += -L{PAHT_TO_LIB} -lqtqrcode` in your **.pro**.
* Include `include({PAHT_TO_LIB}../defaults.pri)` in your **.pro**.
* Copy the classes you will need for your project (or QtQrCodeWidget QtQrCodeQuickItem).
* Finally, if you are chosen to use QuickItem, be sure to use `QtQrCodeQuickItem::registerQmlTypes();` before calling the `load` method of `QQmlApplicationEngine` class.

**Ready, now you have everything you need to use this library.**