lessThan(QT_MAJOR_VERSION, 5) {
    error("Cannot build with Qt version $${QT_VERSION}, this project requires at least Qt 5")
}

TEMPLATE = subdirs

DEFINES +=

SUBDIRS += \
    widget \
    quickitem \
    lib

widget.depends = lib
quickitem.depends = lib

OTHER_FILES += \
    defaults.pri
