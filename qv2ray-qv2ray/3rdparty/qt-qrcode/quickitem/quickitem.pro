TEMPLATE = app
TARGET = QuickItem

QT       += qml quick widgets

LIBS += -L../lib -lqtqrcode

DEFINES +=

SOURCES += main.cpp \
    QtQrCodeQuickItem.cpp

HEADERS += \
    QtQrCodeQuickItem.hpp

RESOURCES += qml.qrc

# Additional import path used to resolve QML modules in Qt Creator's code model
QML_IMPORT_PATH =

include(../defaults.pri)
# Default rules for deployment.
include(deployment.pri)
