file(GLOB_RECURSE CPPS  ./*.cpp )

add_executable(ff ${CPPS})

find_package(Qt5 COMPONENTS
             Charts)

target_link_libraries(ff nodes Qt5::Charts)
