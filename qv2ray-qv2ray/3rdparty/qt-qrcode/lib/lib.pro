TARGET = qtqrcode
TEMPLATE = lib

DEFINES += \
    QTQRCODE_LIBRARY \
    QTQRCODE_PLUS_FEATURES

contains(DEFINES, QTQRCODE_PLUS_FEATURES) {
    QT      += gui svg

    SOURCES += \
        qtqrcodepainter.cpp

    HEADERS +=\
        qtqrcodepainter.h

} else {
    QT      -= gui
}

SOURCES += \
    qtqrcode.cpp

HEADERS +=\
    qtqrcode_global.h \
    qtqrcode.h

INCLUDEPATH += $$PWD

include(../defaults.pri)
include(libqrencode.pri)
# Default rules for deployment.
include(deployment.pri)

unix {
    target.path = /usr/lib
    INSTALLS += target
}
