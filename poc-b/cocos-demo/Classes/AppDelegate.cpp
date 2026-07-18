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
    // POC-B1: 最小演示（硬编码 sprite）
    void hal_runtime_run_demo_scene();
    // POC-B2: 端到端（从 .tscn 构建）
    unsigned long long hal_runtime_run_tscn_scene(const char* path, size_t len);
}

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

    // POC-B2: 从 test_scene.tscn 构建端到端场景
    // .tscn 由 hal-poc (POC-A) 解析，scene_builder 遍历节点调 C++ facade
    const char* tscn_path = "test_scene.tscn";
    unsigned long long result = hal_runtime_run_tscn_scene(
        tscn_path, std::char_traits<char>::length(tscn_path));
    if (result == 0) {
        // fallback 到 B1 demo 场景
        hal_runtime_run_demo_scene();
    }

    return true;
}

void AppDelegate::applicationDidEnterBackground() {
    Director::getInstance()->stopAnimation();
}

void AppDelegate::applicationWillEnterForeground() {
    Director::getInstance()->startAnimation();
}
