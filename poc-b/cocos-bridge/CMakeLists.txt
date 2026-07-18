# cocos-bridge
#
# POC-B1 的 C++ facade：实现 cxx bridge 要求的 hal_xxx 全局函数，
# 内部调 Cocos2d-x API。编译产物是一个静态库 libhal_cocos_bridge.lib，
# 会被 cocos-demo 链接。
#
# 构建依赖：
# - libcocos2d.lib（来自 cocos2d-x 3.17.2）
# - hal_bridge_generated.cc（由 cxxbridge 命令生成）

cmake_minimum_required(VERSION 3.10)
project(hal_cocos_bridge CXX)

set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Cocos2d-x 根目录（POC-B 阶段硬编码，正式版用 CMake 变量）
set(COCOS2DX_ROOT "E:/repos/cocos/cocos2d-x-3.17.2")

# ---- 库目标 ----
add_library(hal_cocos_bridge STATIC
    src/hal_bridge.cpp
    src/hal_bridge_generated.cc
)

target_include_directories(hal_cocos_bridge
    PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/include
        ${COCOS2DX_ROOT}/cocos
        ${COCOS2DX_ROOT}/cocos/editor-support
        ${COCOS2DX_ROOT}/extensions
)

# POC-B1 的预处理器定义（避免 min/max 冲突等常见问题）
target_compile_definitions(hal_cocos_bridge PRIVATE
    NOMINMAX
    _CRT_SECURE_NO_WARNINGS
    _SCL_SECURE_NO_WARNINGS
    CC_ENABLE_CHIPMUNK_INTEGRATION=1
    USING_WIN32
    _WIN32
)

# 链接 libcocos2d.lib（POC-B1 阶段先注释掉，等 libcocos2d.lib 构建好再开）
# find_library(LIBCOCOS2D cocos2d PATHS "${COCOS2DX_ROOT}/cocos/2d/proj.win32/Release.win32")
# target_link_libraries(hal_cocos_bridge PRIVATE ${LIBCOCOS2D})

# 调试信息：打印当前配置
message(STATUS "=== hal_cocos_bridge 配置 ===")
message(STATUS "Cocos2d-x root: ${COCOS2DX_ROOT}")
message(STATUS "C++ standard: ${CMAKE_CXX_STANDARD}")
