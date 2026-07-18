// AppDelegate.h
//
// POC-B1 的 AppDelegate 头文件。
// 从 cpp-template-default/Classes/AppDelegate.h 改造而来。

#ifndef __APP_DELEGATE_H__
#define __APP_DELEGATE_H__

#include "cocos2d.h"

class AppDelegate : public cocos2d::Application {
public:
    AppDelegate();
    virtual ~AppDelegate();

    virtual void initGLContextAttrs() override;

    virtual bool applicationDidFinishLaunching() override;
    virtual void applicationDidEnterBackground() override;
    virtual void applicationWillEnterForeground() override;
};

#endif  // __APP_DELEGATE_H__
