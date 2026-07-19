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
#include <cstdlib>
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

// hal_node_registry_count / hal_export_scene_nodes 是 cxx bridge 的 extern "C++" 函数
// （C++ linkage），由 hal_bridge_generated.cc 定义，不需要 extern "C"
size_t hal_node_registry_count();
void hal_export_scene_nodes(unsigned long long scene, const std::string& out_path);

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

    // POC-B2 + Phase 1: 从 .tscn 构建 UI 布局场景。
    // 默认 control_gallery，可用 HAL_SCENE 环境变量指定其他场景（多场景测试用）。
    const char* scene_env = std::getenv("HAL_SCENE");
    std::string tscn_path_str = (scene_env != nullptr && scene_env[0] != '\0')
        ? std::string("Resources/") + scene_env + ".tscn"
        : "Resources/control_gallery.tscn";
    const char* tscn_path = tscn_path_str.c_str();
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

    // 验证模式（HAL_VERIFY=1）：导出 Cocos 实际节点坐标，供 hal-verify 对比工具读取。
    // Rust 侧已经导出 cocos_export_expected.json（含 path + 期望 Cocos 坐标），
    // 这里导出 cocos_export_actual.json（含 handle + 实际 Cocos 坐标），用 handle 关联。
    const char* verify_env = std::getenv("HAL_VERIFY");
    if (verify_env != nullptr && verify_env[0] == '1' && result != 0) {
        cocos2d::log("HAL_VERIFY: 导出实际节点坐标到 cocos_export_actual.json");
        hal_export_scene_nodes(result, "cocos_export_actual.json");
        // 导出后立即退出，不进入渲染循环（支持 CI 自动化）
        cocos2d::log("HAL_VERIFY: 导出完成，退出程序");
        Director::getInstance()->end();
        return true;
    }

    return true;
}

void AppDelegate::applicationDidEnterBackground() {
    Director::getInstance()->stopAnimation();
}

void AppDelegate::applicationWillEnterForeground() {
    Director::getInstance()->startAnimation();
}
