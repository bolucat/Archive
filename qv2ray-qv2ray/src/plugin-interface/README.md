# Qv2ray Plugin Interface

This repo holds necessary headers for you to write your own Qv2ray plugin.

## File Descriptions

- `Qv2rayPluginObjects.hpp`
  - Contains necessary files to write a Kernel Plugin
- `Qv2rayPluginProcessor.hpp`
  - Processor Object which receives event messages from Qv2ray
- `QvPluginInterface.hpp`
  - Interface Object contains plugin metadata.
- `QvPluginInterfaceModels.hpp`
  - Common data models and Enums for Plugin Metadata and Plugin Event Processor

## How to write a plugin

1. Firstly you need to have a Qt library project: `*.so *.dll *.dylib`
2. Add this repository as a **git submodule** or as a **sub-directory**
3. Include **`QvPluginInterface.pri`** or **`QvPluginInterface.cmake`** into your project `pro` file or `CMakeLists.txt`
4. Write your own `QObject`, inherited from `Qv2rayPlugin::Qv2rayInterface`
5. ….