// AppDelegate.cpp
//
// POC-B 的 Cocos 入口：初始化 Director，调 Rust hal-runtime 创建场景。
//
// 关键改动（相对 cpp-template-default）：
// - 不再调 HelloWorld::createScene()
// - 改为调 hal_runtime::scene_builder::create_demo_scene()
//   （Rust 通过 cxx bridge 调 Cocos facade 创建 sprite 并显示）

#include "AppDelegate.h"
#include "cocos2d.h"

#include <cstddef>
#include <string>
#include <cstring>

USING_NS_CC;

// Rust hal-runtime 的 C++ 桥接声明
extern "C" {
    // POC-B1: 最小演示（硬编码 sprite）—— Rust #[no_mangle] extern "C"
    void hal_runtime_run_demo_scene();
    // POC-B2: 端到端（从 .tscn 构建）—— Rust #[no_mangle] extern "C"
    unsigned long long hal_runtime_run_tscn_scene(const char* path, size_t len);
}

// hal_node_registry_count 是 cxx bridge 的 extern "C++" 函数（C++ linkage），
// 由 hal_bridge_generated.cc 定义，不需要 extern "C"
size_t hal_node_registry_count();

static cocos2d::Size designResolutionSize = cocos2d::Size(960, 640);

AppDelegate::AppDelegate() {}
AppDelegate::~AppDelegate() {}

void AppDelegate::initGLContextAttrs() {
    GLContextAttrs glContextAttrs = {8, 8, 8, 8, 24, 8, 0};
    GLView::setGLContextAttrs(glContextAttrs);
}

bool AppDelegate::applicationDidFinishLaunching() {
    auto director = Director::getInstance();
    auto glview = director->getOpenGLView();
    if (!glview) {
        glview = GLViewImpl::createWithRect(
            "Engine-HAL POC-B",
            cocos2d::Rect(0, 0, designResolutionSize.width, designResolutionSize.height));
        director->setOpenGLView(glview);
    }

    director->setDisplayStats(true);
    director->setAnimationInterval(1.0f / 60);

    glview->setDesignResolutionSize(
        designResolutionSize.width,
        designResolutionSize.height,
        ResolutionPolicy::NO_BORDER);

    // 设置资源搜索路径：把 exe 所在目录加进去
    // （Cocos 默认搜索路径可能找不到 exe 同目录的资源）
    auto fileUtils = cocos2d::FileUtils::getInstance();
    auto workingDir = fileUtils->getDefaultResourceRootPath();
    cocos2d::log("POC-B2: 默认资源根路径 = %s", workingDir.c_str());
    // 把当前工作目录显式加为搜索路径
    fileUtils->addSearchPath(".");

    // POC-B2: 从 complex_scene.tscn 构建复杂多节点场景
    // 注意：Rust std::fs 用相对路径，相对 exe 的工作目录
    // Cocos FileUtils 的搜索路径是 Resources/，但 Rust 不是
    // 所以这里用 Resources/complex_scene.tscn
    const char* tscn_path = "Resources/complex_scene.tscn";
    cocos2d::log("POC-B2: 准备加载 %s", tscn_path);

    unsigned long long result = hal_runtime_run_tscn_scene(
        tscn_path, std::char_traits<char>::length(tscn_path));

    if (result == 0) {
        cocos2d::log("POC-B2: tscn 加载失败，fallback 到 demo 场景");
        hal_runtime_run_demo_scene();
    } else {
        cocos2d::log("POC-B2: tscn 场景加载成功，scene handle = %llu", result);
    }

    // 打印注册的节点数（验证 facade 真的创建了节点）
    cocos2d::log("POC-B2: 当前注册节点数 = %zu", hal_node_registry_count());

    return true;
}

void AppDelegate::applicationDidEnterBackground() {
    Director::getInstance()->stopAnimation();
}

void AppDelegate::applicationWillEnterForeground() {
    Director::getInstance()->startAnimation();
}
