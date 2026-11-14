cmake_minimum_required(VERSION 3.22)
project(anicode C)
# Writes build/compile_commands.json so clangd finds the MLT headers.
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)
# Debug builds of the engine are too slow to scrub in Shotcut.
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release)
endif()

include(FetchContent)
FetchContent_Declare(
    Corrosion
    GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
    GIT_TAG v0.6.1
)
FetchContent_MakeAvailable(Corrosion)
corrosion_import_crate(MANIFEST_PATH Cargo.toml CRATES anicode-ffi anicode-cli)

find_package(PkgConfig REQUIRED)
pkg_check_modules(MLT REQUIRED mlt-framework-7)

add_library(mltanicode MODULE
    mlt-module/factory.c
    mlt-module/producer_anicode.c
)
target_include_directories(mltanicode PRIVATE ${MLT_INCLUDE_DIRS})
target_link_libraries(mltanicode PRIVATE anicode_ffi)
# MLT modules are always .so, also on macOS.
set_target_properties(mltanicode PROPERTIES PREFIX "lib" SUFFIX ".so")

if(APPLE)
    # Do not link Homebrew's libmlt. The module resolves MLT symbols from
    # the host at load time, so inside Shotcut it uses Shotcut's own MLT
    # instead of loading a second copy.
    target_link_options(mltanicode PRIVATE -undefined dynamic_lookup)
else()
    target_link_libraries(mltanicode PRIVATE ${MLT_LINK_LIBRARIES})
endif()
