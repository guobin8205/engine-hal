# cocos-demo/CMakeLists.txt
#
# POC-B1 的顶层构建：把 C++ facade + cocos-demo + Rust hal-runtime + libcocos2d 链接成可执行文件。
#
# 构建流程：
#   1. 先用 cargo build --target i686-pc-windows-msvc 编译 hal-runtime → hal_runtime.lib
#   2. CMake 编译 cocos-bridge (facade) + cocos-demo (main)
#   3. 链接 hal_runtime.lib + libcocos2d.lib + 所有 Cocos 依赖
#   4. 拷贝 libcocos2d.dll 等到 exe 目录
#
# 平台要求：
#   - cmake -G "Visual Studio 17 2022" -A Win32 （必须 Win32，不是 x64）
#   - 因为 Rust 用 i686-pc-windows-msvc，libcocos2d 也是 Win32

cmake_minimum_required(VERSION 3.14)
project(hal_cocos_demo CXX)

# POC-B 强制 Release（与 Rust release /MD CRT 一致）
if(NOT CMAKE_BUILD_TYPE)
    set(CMAKE_BUILD_TYPE Release CACHE STRING "Build type" FORCE)
endif()

set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# MSVC Release 用 /MD（动态 release CRT）
set(CMAKE_MSVC_RUNTIME_LIBRARY "MultiThreadedDLL")

# Cocos2d-x 路径（POC-B 用 Release 配置，与 Rust release CRT 一致）
set(COCOS2DX_ROOT "E:/repos/cocos/cocos2d-x-3.17.2")
set(COCOS_LIB_DIR "${COCOS2DX_ROOT}/cocos/2d/Release.win32")

# Rust hal-runtime 路径（Release，与 C++ Release /MD 一致）
set(HAL_RUNTIME_ROOT "${CMAKE_CURRENT_SOURCE_DIR}/../hal-runtime")
set(HAL_RUNTIME_LIB_DIR "${HAL_RUNTIME_ROOT}/../../target/i686-pc-windows-msvc/release")

# cxx 运行时头文件路径（cxx-1.0.197/include/cxx.h）
set(CXX_INCLUDE_DIR "C:/Users/guobi/.cargo/registry/src/rsproxy.cn-e3de039b2554c837/cxx-1.0.197/include")

# 源文件
set(HAL_BRIDGE_SOURCES
    ${CMAKE_CURRENT_SOURCE_DIR}/../cocos-bridge/src/hal_bridge.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/../cocos-bridge/src/hal_bridge_generated.cc
)

set(DEMO_SOURCES
    Classes/AppDelegate.cpp
    ${CMAKE_CURRENT_SOURCE_DIR}/win32_main.cpp
)

# ---- 可执行文件 ----
add_executable(hal_cocos_demo WIN32 ${DEMO_SOURCES} ${HAL_BRIDGE_SOURCES})

target_include_directories(hal_cocos_demo PRIVATE
    Classes
    ${CMAKE_CURRENT_SOURCE_DIR}/../cocos-bridge/include
    ${CXX_INCLUDE_DIR}
    ${COCOS2DX_ROOT}/cocos
    ${COCOS2DX_ROOT}/cocos/editor-support
    ${COCOS2DX_ROOT}/cocos/platform/win32
    ${COCOS2DX_ROOT}/cocos/platform/desktop
    ${COCOS2DX_ROOT}/extensions
    # 完整复制 libcocos2d.vcxproj 的 external include 路径
    ${COCOS2DX_ROOT}/external
    ${COCOS2DX_ROOT}/external/Box2D/include
    ${COCOS2DX_ROOT}/external/sqlite3/include
    ${COCOS2DX_ROOT}/external/tinyxml2
    ${COCOS2DX_ROOT}/external/edtaa3func
    ${COCOS2DX_ROOT}/external/png/include/win32
    ${COCOS2DX_ROOT}/external/jpeg/include/win32
    ${COCOS2DX_ROOT}/external/tiff/include/win32
    ${COCOS2DX_ROOT}/external/webp/include/win32
    ${COCOS2DX_ROOT}/external/freetype2/include/win32
    ${COCOS2DX_ROOT}/external/win32-specific/OpenalSoft/include
    ${COCOS2DX_ROOT}/external/win32-specific/MP3Decoder/include
    ${COCOS2DX_ROOT}/external/win32-specific/OggDecoder/include
    ${COCOS2DX_ROOT}/external/win32-specific/icon/include
    ${COCOS2DX_ROOT}/external/win32-specific/zlib/include
    ${COCOS2DX_ROOT}/external/win32-specific/gles/include/OGLES
    ${COCOS2DX_ROOT}/external/glfw3/include/win32
    ${COCOS2DX_ROOT}/external/chipmunk/include
    ${COCOS2DX_ROOT}/external/xxhash
    ${COCOS2DX_ROOT}/external/ConvertUTF
    ${COCOS2DX_ROOT}/external/curl/include/win32
    ${COCOS2DX_ROOT}/external/websockets/include/win32
    ${COCOS2DX_ROOT}/external/poly2tri
    ${COCOS2DX_ROOT}/external/poly2tri/common
    ${COCOS2DX_ROOT}/external/poly2tri/sweep
    ${COCOS2DX_ROOT}/external/bullet/include
    ${COCOS2DX_ROOT}/external/bullet/include/bullet
)

# 预处理器定义（Cocos Win32 必需）
target_compile_definitions(hal_cocos_demo PRIVATE
    NOMINMAX
    _CRT_SECURE_NO_WARNINGS
    _SCL_SECURE_NO_WARNINGS
    CC_ENABLE_CHIPMUNK_INTEGRATION=1
    USING_WIN32
    _WIN32
    DEBUG=0
    COCOS2D_DEBUG=0
    NDEBUG
)

# 链接库
target_link_directories(hal_cocos_demo PRIVATE
    ${COCOS_LIB_DIR}
    ${HAL_RUNTIME_LIB_DIR}
    ${COCOS2DX_ROOT}/external/win32/x86/lib
    ${COCOS2DX_ROOT}/external/win32/x86/lib/Debug
    ${COCOS2DX_ROOT}/external/win10-specific/zlib/prebuilt/win32
)

target_link_libraries(hal_cocos_demo PRIVATE
    hal_runtime            # Rust 静态库
    libcocos2d             # Cocos 导入库（DLL 模式）
    # Cocos 依赖（Win32）
    libSpine
    libbullet
    librecast
    libbox2d
    # 第三方
    libcurl
    websockets
    libcrypto
    libssl
    OpenAL32
    glew32
    zlib
    opengl32
    # Windows 系统
    ws2_32
    winmm
    version
    user32
    gdi32
    ole32
    imm32
    # Rust std 需要的系统库（避免 LNK2019 未解析符号）
    ntdll        # NtReadFile, NtCreateFile 等
    userenv      # GetUserProfileDirectoryW
    bcrypt       # Rust 同步原语
    synchronization  # Rust ONCE 初始化
)

# 编译选项
if(MSVC)
    target_compile_options(hal_cocos_demo PRIVATE /W3 /bigobj /EHsc /utf-8)
    # Rust 符号用 C calling convention，下划线前缀（32 位 MSVC）
    target_link_options(hal_cocos_demo PRIVATE /MACHINE:X86)
endif()

# 拷贝 DLL 到 exe 目录（POC 简化：用 post-build）
add_custom_command(TARGET hal_cocos_demo POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_directory
        ${COCOS_LIB_DIR}
        $<TARGET_FILE_DIR:hal_cocos_demo>
    COMMENT "拷贝 Cocos DLL 到输出目录"
)

message(STATUS "=== hal_cocos_demo 配置 ===")
message(STATUS "Cocos2d-x root: ${COCOS2DX_ROOT}")
message(STATUS "Cocos lib dir: ${COCOS_LIB_DIR}")
message(STATUS "hal-runtime lib: ${HAL_RUNTIME_LIB_DIR}/hal_runtime.lib")
message(STATUS "cxx include: ${CXX_INCLUDE_DIR}")
