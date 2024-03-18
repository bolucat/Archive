# Automatically generated by scripts/boost/generate-ports.ps1

vcpkg_from_github(
    OUT_SOURCE_PATH SOURCE_PATH
    REPO boostorg/uuid
    REF boost-1.83.0
    SHA512 8b6d2f77b1bb3c3fa7238899a7e88772ec4094f49578102df73a46a3e7be69b8ad4df05d2a5b5322705b53f3f0f86ab1a91c495dc5820433b535bb8d5b15a195
    HEAD_REF master
)

include(${CURRENT_INSTALLED_DIR}/share/boost-vcpkg-helpers/boost-modular-headers.cmake)
boost_modular_headers(SOURCE_PATH ${SOURCE_PATH})