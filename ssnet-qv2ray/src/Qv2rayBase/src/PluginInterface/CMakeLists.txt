cmake_minimum_required(VERSION 3.20.0)

project(QvPluginInterface VERSION 5.0)

find_package(Qt6 6.2 COMPONENTS Core REQUIRED)

set(INTERFACE_HEADERS
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Common/CommonTypes.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Common/CommonSafeType.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Common/QvPluginBase.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Common/EditorCreatorDefs.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Connections/ConnectionsBase.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Utils/BindableProps.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Utils/QJsonIO.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Utils/JsonConversion.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Utils/ForEachMacros.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/PluginInterface.hpp
)

set(FEATURE_HEADERS
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Gui/QvGUIPluginInterface.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Handlers/EventHandler.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Handlers/IProfilePreprocessor.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Handlers/KernelHandler.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Handlers/OutboundHandler.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Handlers/SubscriptionHandler.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Handlers/LatencyTestHandler.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Socksify/HttpProxy.hpp
    ${CMAKE_CURRENT_LIST_DIR}/include/QvPlugin/Socksify/SocketStream.hpp
)

add_library(QvPluginInterface INTERFACE ${INTERFACE_HEADERS} ${FEATURE_HEADERS})
add_library(Qv2ray::QvPluginInterface ALIAS QvPluginInterface)

set_target_properties(QvPluginInterface PROPERTIES AUTOMOC OFF AUTOUIC OFF AUTORCC OFF VERSION ${PROJECT_VERSION} SOVERSION 0)
target_link_libraries(QvPluginInterface INTERFACE Qt::Core)

include(GNUInstallDirs)

target_include_directories(${PROJECT_NAME} INTERFACE
    "$<BUILD_INTERFACE:${CMAKE_CURRENT_LIST_DIR}/include>"
    "$<INSTALL_INTERFACE:${CMAKE_INSTALL_INCLUDEDIR}>"
)

include(CMakePackageConfigHelpers)

write_basic_package_version_file(
                ${CMAKE_CURRENT_BINARY_DIR}/QvPluginInterfaceConfigVersion.cmake
                VERSION ${PROJECT_VERSION}
                COMPATIBILITY SameMajorVersion
                ARCH_INDEPENDENT)

configure_package_config_file(${CMAKE_CURRENT_LIST_DIR}/cmake/QvPluginInterfaceConfig.cmake.in
    ${CMAKE_CURRENT_BINARY_DIR}/QvPluginInterfaceConfig.cmake
    INSTALL_DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/QvPluginInterface
)

install(TARGETS QvPluginInterface
    EXPORT QvPluginInterfaceTargets
    LIBRARY DESTINATION ${CMAKE_INSTALL_LIBDIR}
    ARCHIVE DESTINATION ${CMAKE_INSTALL_LIBDIR}
)

install(EXPORT QvPluginInterfaceTargets
    NAMESPACE Qv2ray::
    COMPONENT "development"
    EXCLUDE_FROM_ALL
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/QvPluginInterface
)

install(FILES
        ${CMAKE_CURRENT_BINARY_DIR}/QvPluginInterfaceConfig.cmake
        ${CMAKE_CURRENT_BINARY_DIR}/QvPluginInterfaceConfigVersion.cmake
    COMPONENT "development"
    EXCLUDE_FROM_ALL
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/QvPluginInterface)

install(FILES ${CMAKE_CURRENT_SOURCE_DIR}/cmake/QvPluginInterfaceMacros.cmake
    COMPONENT "development"
    EXCLUDE_FROM_ALL
    DESTINATION ${CMAKE_INSTALL_LIBDIR}/cmake/QvPluginInterface)

install(DIRECTORY ${CMAKE_CURRENT_LIST_DIR}/include/
    COMPONENT "development"
    EXCLUDE_FROM_ALL
    DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})

export(EXPORT QvPluginInterfaceTargets
    FILE "${CMAKE_CURRENT_BINARY_DIR}/cmake/QvPluginInterfaceTargets.cmake"
    NAMESPACE Qv2ray::
)

include(${CMAKE_CURRENT_SOURCE_DIR}/cmake/QvPluginInterfaceMacros.cmake)
