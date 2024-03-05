"""This module provides common build config options"""

_default_copts = select({
    "//build_config:windows_x86_64": [
        "/std:c++20",
    ],
    "//conditions:default": [
        "-std=c++20",
        "-fno-strict-aliasing",
    ],
})

_strings_hdrs = select({
    "//build_config:windows_x86_64": ["strings/string_util_win.h"],
    "//conditions:default": ["strings/string_util_posix.h"],
})

_url_linkopts = select({
    "//build_config:with_system_icu": ["-licuuc"],
    "//conditions:default": [],
})

build_config = struct(
    default_copts = _default_copts,
    url_linkopts = _url_linkopts,
    strings_hdrs = _strings_hdrs,
)
