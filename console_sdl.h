#pragma once
#include "console.h"
#include <SDL2/SDL.h>
#include <atomic>
#include <functional>

// Extended console that runs an SDL event loop
class ConsoleSDL : public Console {
  public:
    ConsoleSDL();
    virtual ~ConsoleSDL() override;

    virtual int Run() override;
    virtual void Stop() override;

    // Allow derived subclasses explicitly hook into drawing
    SDL_Renderer *GetRenderer() const { return mRenderer; }
    void ForceRefresh() { mNeedsRefresh = true; }
    void SetDrawCallback(std::function<void(SDL_Renderer *)> cb) { mDrawCallback = cb; }

  protected:
    std::function<void(SDL_Renderer *)> mDrawCallback;
    SDL_Window *mWindow = nullptr;
    SDL_Renderer *mRenderer = nullptr;
    std::atomic<bool> mNeedsRefresh{false};
    std::atomic<bool> mQuit{false};
};
