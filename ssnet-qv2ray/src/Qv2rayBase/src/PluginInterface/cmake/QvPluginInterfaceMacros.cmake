# So high, so modern, so "cmake_path()"
cmake_minimum_required(VERSION 3.20.0)

option(QV2RAY_STATIC_PLUGINS "Create Static Plugins")

function(qv2ray_add_plugin_moc_sources TARGET)
    if(NOT QvPluginInterface_UseAsLib)
        get_filename_component(QvPluginInterface_Prefix "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../include/" ABSOLUTE)
    else()
        get_target_property(QvPluginInterface_Prefix Qv2ray::QvPluginInterface INTERFACE_INCLUDE_DIRECTORIES)
    endif()
    target_sources(${TARGET} PRIVATE ${QvPluginInterface_Prefix}/QvPlugin/Utils/BindableProps.hpp)
endfunction()

function(qv2ray_add_plugin_gui_sources TARGET)
    if(NOT QvPluginInterface_UseAsLib)
        get_filename_component(QvPluginInterface_Prefix "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../include/" ABSOLUTE)
    else()
        get_target_property(QvPluginInterface_Prefix Qv2ray::QvPluginInterface INTERFACE_INCLUDE_DIRECTORIES)
    endif()
    target_sources(${TARGET} PRIVATE ${QvPluginInterface_Prefix}/QvPlugin/Gui/QvGUIPluginInterface.hpp)
endfunction()

if(NOT TARGET _Qv2ray_AllPlugins)
    add_library(_Qv2ray_AllPlugins INTERFACE)
    add_library(Qv2ray::AllPlugins ALIAS _Qv2ray_AllPlugins)
    message(STATUS "Added Qv2ray Plugins Meta Target")
endif()

function(qv2ray_add_plugin TARGET_NAME)
    set(Stable_PluginInterface_VERSION 5)
    set(options GUI Quick Widgets NO_INSTALL NO_RPATH HTTP_TO_SOCKS STATIC DEV_INTERFACE DEBUGGING_EXECUTABLE)
    set(oneValueArgs INSTALL_PREFIX_LINUX INSTALL_PREFIX_WINDOWS INSTALL_PREFIX_MACOS INSTALL_PREFIX_ANDROID CLASS_NAME INTERFACE_VERSION)
    set(multiValueArgs EXTRA_DEPENDENCY_DIRS_WINDOWS)
    cmake_parse_arguments(QVPLUGIN "${options}" "${oneValueArgs}" "${multiValueArgs}" ${ARGN})

    # ====================================== BEGIN PARSING ARGUMENTS
    if(NOT DEFINED QVPLUGIN_Widgets)
        set(QVPLUGIN_Widgets FALSE)
    elseif(QVPLUGIN_Widgets)
        set(QVPLUGIN_GUI TRUE)
    endif()

    if(NOT DEFINED QVPLUGIN_Quick)
        set(QVPLUGIN_Quick FALSE)
    elseif(QVPLUGIN_Quick)
        set(QVPLUGIN_GUI TRUE)
    endif()

    if(NOT DEFINED QVPLUGIN_GUI)
        set(QVPLUGIN_GUI FALSE)
    endif()

    if(NOT DEFINED QVPLUGIN_NO_INSTALL)
        set(QVPLUGIN_NO_INSTALL FALSE)
    endif()

    if(NOT DEFINED QVPLUGIN_HTTP_TO_SOCKS)
        set(QVPLUGIN_HTTP_TO_SOCKS FALSE)
    endif()

    if(NOT DEFINED QVPLUGIN_NO_RPATH)
        set(QVPLUGIN_NO_RPATH FALSE)
    endif()

    if((NOT DEFINED QVPLUGIN_INSTALL_PREFIX_LINUX) OR (QVPLUGIN_INSTALL_PREFIX_LINUX STREQUAL ""))
        set(QVPLUGIN_INSTALL_PREFIX_LINUX "lib/Qv2rayBase/plugins")
    endif()

    if((NOT DEFINED QVPLUGIN_INSTALL_PREFIX_WINDOWS) OR (QVPLUGIN_INSTALL_PREFIX_WINDOWS STREQUAL ""))
        set(QVPLUGIN_INSTALL_PREFIX_WINDOWS "plugins")
    endif()

    if((NOT DEFINED QVPLUGIN_INSTALL_PREFIX_MACOS) OR (QVPLUGIN_INSTALL_PREFIX_MACOS STREQUAL ""))
        set(QVPLUGIN_INSTALL_PREFIX_MACOS "plugins")
    endif()

    if(DEFINED QVPLUGIN_INSTALL_PREFIX_ANDROID)
        message("INSTALL_PREFIX_ANDROID is not used")
    endif()

    if(${QVPLUGIN_STATIC} OR ${QV2RAY_STATIC_PLUGINS})
        set(QVPLUGIN_STATIC ON)
        set(QVPLUGIN_NO_INSTALL ON)
        if((NOT DEFINED QVPLUGIN_CLASS_NAME) OR (QVPLUGIN_CLASS_NAME STREQUAL ""))
            message(FATAL_ERROR "A static plugin must provide its main plugin class name.")
        endif()
    endif()

    if(QVPLUGIN_DEV_INTERFACE)
        if(DEFINED QVPLUGIN_INTERFACE_VERSION)
            message(FATAL_ERROR "Cannot specify INTERFACE_VERSION and DEV_INTERFACE at the same time.")
        endif()

        math(EXPR DEV_VERSION "${Stable_PluginInterface_VERSION} + 1")
        set(QVPLUGIN_INTERFACE_VERSION ${DEV_VERSION})
        message(STATUS "Use Interface version ${QVPLUGIN_INTERFACE_VERSION} (dev)")
    else()
        if(NOT DEFINED QVPLUGIN_INTERFACE_VERSION)
            set(QVPLUGIN_INTERFACE_VERSION ${Stable_PluginInterface_VERSION})
        endif()
        message(STATUS "Use Interface version ${QVPLUGIN_INTERFACE_VERSION}")
    endif()

    # ====================================== END PARSING ARGUMENTS

    if(NOT QvPluginInterface_UseAsLib)
        get_filename_component(QvPluginInterface_Prefix "${CMAKE_CURRENT_FUNCTION_LIST_DIR}/../include/" ABSOLUTE)
    else()
        get_target_property(QvPluginInterface_Prefix Qv2ray::QvPluginInterface INTERFACE_INCLUDE_DIRECTORIES)
    endif()

    if(QVPLUGIN_STATIC)
        add_library(${TARGET_NAME} STATIC)
        target_link_libraries(_Qv2ray_AllPlugins INTERFACE ${TARGET_NAME})
        target_compile_definitions(${TARGET_NAME} PRIVATE "QT_STATICPLUGIN=1")
        message(STATUS "Generating static plugin importing source code for ${TARGET_NAME}")

        get_target_property(OUT ${TARGET_NAME} BINARY_DIR)
        set(IMPORT_SRC "${OUT}/${TARGET_NAME}_qv2ray_static_plugin_import.cpp")

        # Write the file header
        file(WRITE ${IMPORT_SRC} [[
// Qv2ray Static Plugin Import File
// File Generated via CMake script during configure time.
// Please rerun CMake to update this file, this file will be overwrite at each CMake run.
#include <QtPlugin>
]]
            )
        file(APPEND ${IMPORT_SRC} "Q_IMPORT_PLUGIN(${QVPLUGIN_CLASS_NAME});")
        message("Generated at: ${IMPORT_SRC}")
        target_sources(${TARGET_NAME} INTERFACE ${IMPORT_SRC})
        set_target_properties(${TARGET_NAME} PROPERTIES CXX_VISIBILITY_PRESET hidden)
    else()
        add_library(${TARGET_NAME} SHARED)
        qv2ray_add_plugin_moc_sources(${TARGET_NAME})
    endif()

    set_target_properties(${TARGET_NAME} PROPERTIES AUTOMOC ON)
    set_property(TARGET ${TARGET_NAME} APPEND PROPERTY AUTOMOC_MACRO_NAMES "QV2RAY_PLUGIN")

    if(CMAKE_CXX_COMPILER_ID EQUAL Clang OR CMAKE_COMPILER_IS_GNUCC OR CMAKE_COMPILER_IS_GNUCXX)
        if(UNIX AND NOT APPLE)
            target_link_libraries(${TARGET_NAME} PRIVATE "-Wl,-z,defs")
        endif()
    endif()

    target_compile_definitions(${TARGET_NAME} PRIVATE -DPLUGIN_INTERFACE_VERSION=${QVPLUGIN_INTERFACE_VERSION})

    find_package(Qt6 COMPONENTS Core Network REQUIRED)
    target_link_libraries(${TARGET_NAME} PRIVATE Qt::Core Qt::Network Qv2ray::QvPluginInterface)

    if(QVPLUGIN_HTTP_TO_SOCKS)
        target_sources(${TARGET_NAME}
            PRIVATE
            ${QvPluginInterface_Prefix}/QvPlugin/Socksify/HttpProxy.hpp
            ${QvPluginInterface_Prefix}/QvPlugin/Socksify/SocketStream.hpp)
    endif()

    if(QVPLUGIN_GUI)
        find_package(Qt6 COMPONENTS Gui REQUIRED)
        target_link_libraries(${TARGET_NAME} PRIVATE Qt::Gui)
        qv2ray_add_plugin_gui_sources(${TARGET_NAME})
    endif()

    if(QVPLUGIN_Quick)
        target_link_libraries(${TARGET_NAME} PRIVATE Qt::Quick)
    endif()

    if(QVPLUGIN_Widgets)
        target_link_libraries(${TARGET_NAME} PRIVATE Qt::Widgets)
        set_target_properties(${TARGET_NAME} PROPERTIES AUTOUIC ON)
    endif()

    if(QVPLUGIN_DEBUGGING_EXECUTABLE)
        message(STATUS "Adding debugging executable.")
        set_target_properties(${TARGET_NAME} PROPERTIES ENABLE_EXPORTS 1)
        get_target_property(OUT ${TARGET_NAME} BINARY_DIR)
        set(EXEC_SOURCE "${OUT}/${TARGET_NAME}_exec.cpp")
        file(WRITE ${EXEC_SOURCE} [[
// Qv2ray Plugin Debugging Executable Helper Launcher
// File Generated via CMake script during configure time.
// Please rerun CMake to update this file, this file will be overwrite at each CMake run.
extern int plugin_main(int argc, char *argv[]);
int main(int argc, char *argv[])
{
    return plugin_main(argc, argv);
}
]])
        add_executable(${TARGET_NAME}_exec ${EXEC_SOURCE})
        target_link_libraries(${TARGET_NAME}_exec PRIVATE ${TARGET_NAME})
    endif()

    if(APPLE AND NOT QVPLUGIN_NO_RPATH)
        add_custom_command(TARGET ${TARGET_NAME}
            POST_BUILD
            COMMAND
            ${CMAKE_INSTALL_NAME_TOOL} -add_rpath "@executable_path/../Frameworks/" $<TARGET_FILE:${TARGET_NAME}>)
    endif()

    if(NOT QVPLUGIN_NO_INSTALL)
        cmake_policy(SET CMP0087 NEW)
        if(${CMAKE_SYSTEM_NAME} STREQUAL "Linux")
            install(TARGETS ${TARGET_NAME} LIBRARY DESTINATION ${QVPLUGIN_INSTALL_PREFIX_LINUX})
        elseif(WIN32)
            install(TARGETS ${TARGET_NAME} RUNTIME DESTINATION ${QVPLUGIN_INSTALL_PREFIX_WINDOWS})
            install(CODE "
set(EXTRA_DIRS \"${QVPLUGIN_EXTRA_DEPENDENCY_DIRS_WINDOWS}\")
list(APPEND EXTRA_DIRS \"$<TARGET_PROPERTY:${TARGET_NAME},BINARY_DIR>\")
set(PLUGIN_INSTALL_PREFIX \"${CMAKE_INSTALL_PREFIX}/${QVPLUGIN_INSTALL_PREFIX_WINDOWS}/libs\")
set(TARGET_NAME ${TARGET_NAME})
set(TARGET_FILE \"$<TARGET_FILE:${TARGET_NAME}>\")
")

            install(CODE [[
file(GET_RUNTIME_DEPENDENCIES
    LIBRARIES ${TARGET_FILE}
    RESOLVED_DEPENDENCIES_VAR "dependencies"
    UNRESOLVED_DEPENDENCIES_VAR "un_depenendcies_unused"
    DIRECTORIES ${EXTRA_DIRS}
    )
foreach(dll ${dependencies})
    foreach(dir ${EXTRA_DIRS})
        cmake_path(IS_PREFIX dir "${dll}" NORMALIZE FOUND)
        if(FOUND)
            message(STATUS "${TARGET_NAME}: Found dependency: '${dll}'.")
            file(COPY ${dll} DESTINATION ${PLUGIN_INSTALL_PREFIX})
            break()
        endif()
    endforeach()
endforeach()
]])
        elseif(APPLE)
            install(TARGETS ${TARGET_NAME} LIBRARY DESTINATION ${QVPLUGIN_INSTALL_PREFIX_MACOS})
        elseif(ANDROID)
            set(apk_dir "$<TARGET_PROPERTY:${TARGET_NAME},BINARY_DIR>/android-build")
            add_custom_command(TARGET ${TARGET_NAME} POST_BUILD
                COMMAND
                ${CMAKE_COMMAND} -E copy $<TARGET_FILE:${TARGET_NAME}>
                "${apk_dir}/libs/${CMAKE_ANDROID_ARCH_ABI}/$<TARGET_FILE_NAME:${TARGET_NAME}>"
                )
        else()
            message(FATAL_ERROR "This platform is not supported yet.")
        endif()
    endif()

    message(STATUS "==========================================================")
    message(STATUS "Qv2ray Plugin ${TARGET_NAME}")
    message(STATUS "   API Version: ${QVPLUGIN_INTERFACE_VERSION}")
    message(STATUS " Static Plugin: ${QVPLUGIN_STATIC}")
    message(STATUS "  Debug Helper: ${QVPLUGIN_DEBUGGING_EXECUTABLE}")
    message(STATUS "  Use QWidgets: ${QVPLUGIN_Widgets}")
    message(STATUS "   Use QtQuick: ${QVPLUGIN_Quick}")
    message(STATUS "     Use QtGui: ${QVPLUGIN_GUI}")
    message(STATUS "    HTTP2SOCKS: ${QVPLUGIN_HTTP_TO_SOCKS}")
    message(STATUS "    No Install: ${QVPLUGIN_NO_INSTALL}")
    message(STATUS "No macOS RPath: ${QVPLUGIN_NO_RPATH}")
    message(STATUS " Global Prefix: ${CMAKE_INSTALL_PREFIX}")
    message(STATUS "  Linux Prefix: ${QVPLUGIN_INSTALL_PREFIX_LINUX}")
    message(STATUS "  macOS Prefix: ${QVPLUGIN_INSTALL_PREFIX_MACOS}")
    message(STATUS "Windows Prefix: ${QVPLUGIN_INSTALL_PREFIX_WINDOWS}")
    message(STATUS "Android Prefix: ${QVPLUGIN_INSTALL_PREFIX_ANDROID}")
    message(STATUS "==========================================================")
    message("")
endfunction()
