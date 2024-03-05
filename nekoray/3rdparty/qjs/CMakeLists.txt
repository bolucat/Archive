cmake_minimum_required(VERSION 3.1)
project(quickjs)

if (shared)
else ()
    set(link_type STATIC)
endif ()

add_library(${PROJECT_NAME} ${link_type}
        quickjs.h
        quickjs-libc.h
        quickjs.c
        libregexp.c
        libunicode.c
        libbf.c
        cutils.c
        quickjs-libc.c
        nekoray_qjs.c
        )

if (LINUX)
    target_link_libraries(${PROJECT_NAME} PUBLIC dl)
endif ()

if (MSVC)
    set(MSVC_JS_STRICT_NAN_BOXING "JS_STRICT_NAN_BOXING")
endif ()

target_compile_definitions(${PROJECT_NAME}
        PRIVATE
        CONFIG_VERSION="nekoray"
        _GNU_SOURCE
        CONFIG_BIGNUM
        ${MSVC_JS_STRICT_NAN_BOXING}
        )
