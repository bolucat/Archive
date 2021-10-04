TEMPLATE = app
TARGET = Widget

greaterThan(QT_MAJOR_VERSION, 4): QT += widgets

QT       += core gui

LIBS += -L../lib -lqtqrcode

DEFINES +=

SOURCES += main.cpp\
        MainWindow.cpp \
    QtQrCodeWidget.cpp

HEADERS  += MainWindow.hpp \
    QtQrCodeWidget.hpp

FORMS    += MainWindow.ui

CONFIG += mobility
MOBILITY = 

include(../defaults.pri)
# Default rules for deployment.
include(deployment.pri)
