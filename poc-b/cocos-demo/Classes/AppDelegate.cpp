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

USING_NS_CC;

// Rust hal-runtime 的 C++ 桥接声明（cxx bridge 自动生成的）
// 实际符号在 libhal_runtime.a 里
extern "C" {
    // hal-runtime 的 scene_builder 暴露的入口
    // （POC-B1 简化：先用 C 接口，B2 改用 cxx bridge）
    void hal_runtime_run_demo_scene();
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

    // POC-B1: 调 Rust hal-runtime 创建演示场景
    // Rust 内部会调 C++ facade (hal_scene_create, hal_sprite_create, etc.)
    hal_runtime_run_demo_scene();

    return true;
}

void AppDelegate::applicationDidEnterBackground() {
    Director::getInstance()->stopAnimation();
}

void AppDelegate::applicationWillEnterForeground() {
    Director::getInstance()->startAnimation();
}
