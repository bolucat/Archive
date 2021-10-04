option(USE_SYSTEM_LIBUV "Use system libuv" OFF)
option(USE_SYSTEM_UVW "Use system libuvw" OFF)

if(USE_SYSTEM_UVW)
    set(USE_SYSTEM_LIBUV ON)
endif()

if(USE_SYSTEM_LIBUV)
    # Special package name from vcpkg
    find_package(unofficial-libuv CONFIG)
    if(${unofficial-libuv_FOUND})
        add_library(Qv2ray::libuv ALIAS unofficial::libuv::libuv)
    else()
        find_package(LibUV REQUIRED)
        add_library(Qv2ray::libuv ALIAS LibUV::LibUV)
    endif()
else()
    add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/libuv EXCLUDE_FROM_ALL)
    set_target_properties(uv_a PROPERTIES EXCLUDE_FROM_ALL TRUE POSITION_INDEPENDENT_CODE 1)
    add_library(Qv2ray::libuv ALIAS uv_a)

    # BEGIN - the hack to install libuv as static libraries
    if(NOT BUILD_SHARED_LIBS)
        install(TARGETS uv_a
            COMPONENT development
            EXCLUDE_FROM_ALL
            EXPORT libuvTargets
            RUNTIME DESTINATION "lib/"
            ARCHIVE DESTINATION "lib/")
        install(EXPORT libuvTargets
            COMPONENT development
            EXCLUDE_FROM_ALL
            FILE libuvConfig.cmake
            NAMESPACE LibUV::
            DESTINATION "lib/cmake/LibUV")
        export(TARGETS uv_a
            FILE _dummy-uv-a.cmake)
    endif()
    # END - the hack to install libuv as static libraries
endif()

if(USE_SYSTEM_UVW)
    find_package(uvw CONFIG REQUIRED)
    add_library(Qv2ray::libuvw ALIAS uvw::uvw)
else()
    set(UVW_SOURCES
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/async.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/check.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/dns.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/emitter.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/fs.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/fs_event.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/fs_poll.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/idle.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/lib.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/loop.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/pipe.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/poll.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/prepare.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/process.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/signal.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/stream.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/tcp.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/thread.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/timer.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/tty.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/util.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/work.cpp
        ${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src/uvw/udp.cpp
        )

    add_library(uvw STATIC ${UVW_SOURCES})
    set_target_properties(uvw PROPERTIES CXX_STANDARD 17)
    target_compile_definitions(uvw PRIVATE UVW_AS_LIB)
    target_link_libraries(uvw Qv2ray::libuv)
    set_target_properties(uvw PROPERTIES EXCLUDE_FROM_ALL TRUE)

    target_include_directories(uvw PUBLIC
        PUBLIC
            "$<BUILD_INTERFACE:${CMAKE_CURRENT_SOURCE_DIR}/3rdparty/uvw/src>"
            "$<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDE_DIR}/uvw/>")

    add_library(Qv2ray::libuvw ALIAS uvw)

    # BEGIN - the hack to install libuvw as static libraries
    if(NOT BUILD_SHARED_LIBS)
        install(TARGETS uvw
            COMPONENT development
            EXCLUDE_FROM_ALL
            EXPORT uvwTargets
            RUNTIME DESTINATION "lib/"
            ARCHIVE DESTINATION "lib/")
        install(EXPORT uvwTargets
            COMPONENT development
            EXCLUDE_FROM_ALL
            FILE uvwConfig.cmake
            NAMESPACE UVW::
            DESTINATION "lib/cmake/uvw")
        export(TARGETS uvw
            FILE _dummy-uvw.cmake)
    endif()
    # END  - the hack to install libuvw as static libraries
endif()
