cmake_minimum_required(VERSION 3.22)
project(slang_glslang_sys LANGUAGES C CXX)

set(SLANG_TREE_DIR "" CACHE PATH "The top level of the Slang source tree")
if(NOT IS_DIRECTORY "${SLANG_TREE_DIR}")
    message(FATAL_ERROR "SLANG_TREE_DIR is not a directory: ${SLANG_TREE_DIR}")
endif()

set(CMAKE_POSITION_INDEPENDENT_CODE ON)
set(BUILD_SHARED_LIBS OFF)
set(BUILD_TESTING OFF)

set(SPIRV_TOOLS_BUILD_STATIC ON)
set(SPIRV_WERROR OFF)
set(SPIRV_SKIP_TESTS ON)
set(SPIRV_SKIP_EXECUTABLES ON)
set(SKIP_SPIRV_TOOLS_INSTALL ON)
set(SPIRV_TOOLS_USE_MIMALLOC OFF)
set(SPIRV_TOOLS_USE_MIMALLOC_IN_STATIC_BUILD OFF)

set(SKIP_GLSLANG_INSTALL ON)
set(ENABLE_OPT ON)
set(ENABLE_PCH OFF)
set(ALLOW_EXTERNAL_SPIRV_TOOLS OFF)
set(GLSLANG_ENABLE_INSTALL OFF)
set(GLSLANG_TESTS OFF)
set(ENABLE_GLSLANG_BINARIES OFF)
set(BUILD_EXTERNAL OFF)

add_subdirectory("${SLANG_TREE_DIR}/external/spirv-headers" spirv-headers EXCLUDE_FROM_ALL)
add_subdirectory("${SLANG_TREE_DIR}/external/spirv-tools" spirv-tools EXCLUDE_FROM_ALL)
add_subdirectory("${SLANG_TREE_DIR}/external/glslang" glslang EXCLUDE_FROM_ALL)

add_library(slang-glslang-static STATIC
    "${SLANG_TREE_DIR}/source/slang-glslang/slang-glslang.cpp"
    loader.cpp
)
target_include_directories(slang-glslang-static PRIVATE
    "${SLANG_TREE_DIR}/include"
    "${SLANG_TREE_DIR}/source"
    "${SLANG_TREE_DIR}/external/unordered_dense/include"
)
target_link_libraries(slang-glslang-static PRIVATE glslang SPIRV-Tools-opt SPIRV-Tools-link)
set_target_properties(slang-glslang-static PROPERTIES
    CXX_STANDARD 20
    CXX_STANDARD_REQUIRED ON
    CXX_VISIBILITY_PRESET hidden
    VISIBILITY_INLINES_HIDDEN ON
)
if(WIN32)
    target_compile_definitions(slang-glslang-static PRIVATE
        NOMINMAX WIN32_LEAN_AND_MEAN VC_EXTRALEAN UNICODE _UNICODE
    )
endif()
