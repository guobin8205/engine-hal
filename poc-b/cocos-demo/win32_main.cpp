// win32_main.cpp
//
// POC-B1 的 Win32 入口。
// 从 cpp-template-default/proj.win32/main.cpp 改造而来。
// 注意：用 WinMain（不是 _tWinMain），避免依赖 tchar.h 的 UNICODE 配置。

#include "AppDelegate.h"
#include "cocos2d.h"

USING_NS_CC;

int WINAPI WinMain(HINSTANCE hInstance,
                   HINSTANCE hPrevInstance,
                   LPSTR lpCmdLine,
                   int nCmdShow) {
    UNREFERENCED_PARAMETER(hPrevInstance);
    UNREFERENCED_PARAMETER(lpCmdLine);

    // 创建 AppDelegate 实例并运行
    AppDelegate app;
    return Application::getInstance()->run();
}
