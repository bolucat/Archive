QT += widgets

HEADERS = $$PWD/../src/PureJson.hpp $$PWD/MainWindow.h
SOURCES = $$PWD/MainWindow.cpp $$PWD/main.cpp
FORMS +=  $$PWD/MainWindow.ui
INCLUDEPATH += $$PWD/../src/

INSTALLS += target
