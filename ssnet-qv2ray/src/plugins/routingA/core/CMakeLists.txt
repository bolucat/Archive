cmake_minimum_required(VERSION 3.5)

project(libRoutingA LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)
set(CMAKE_INCLUDE_CURRENT_DIR ON)

find_package(Qt6 6.2 COMPONENTS Core REQUIRED)

set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

add_executable(libRoutingA
    main.cpp
    libRoutingA.hpp
    libRoutingA.cpp
    libRoutingA-ActionTable.cpp
    )

target_compile_definitions(libRoutingA PRIVATE QT_DISABLE_DEPRECATED_BEFORE=0x060200)

target_link_libraries(libRoutingA PRIVATE Qt::Core)
