DEFINES += __STATIC=static MAJOR_VERSION=3 MINOR_VERSION=9 MICRO_VERSION=0 VERSION=\'\"3.9.0\"\'

INCLUDEPATH += libqrencode
SOURCES += \
    libqrencode/bitstream.c \
    libqrencode/qrencode.c \
    libqrencode/mqrspec.c \
    libqrencode/qrinput.c \
    libqrencode/qrspec.c \
    libqrencode/split.c \
    libqrencode/rsecc.c \
    libqrencode/mmask.c \
#    libqrencode/qrenc.c \
    libqrencode/mask.c
HEADERS += \
    libqrencode/qrencode_inner.h \
    libqrencode/bitstream.h \
    libqrencode/qrencode.h \
    libqrencode/mqrspec.h \
    libqrencode/qrinput.h \
    libqrencode/qrspec.h \
    libqrencode/split.h \
    libqrencode/rsecc.h \
    libqrencode/mmask.h \
    libqrencode/mask.h
