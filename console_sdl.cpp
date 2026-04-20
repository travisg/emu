#include "console_sdl.h"
#include <iostream>

ConsoleSDL::ConsoleSDL() : Console() {
    if (SDL_Init(SDL_INIT_VIDEO) < 0) {
        std::cerr << "SDL could not initialize! SDL_Error: " << SDL_GetError() << std::endl;
        return;
    }

    mWindow = SDL_CreateWindow("Kaypro II Emulator",
                               SDL_WINDOWPOS_UNDEFINED, SDL_WINDOWPOS_UNDEFINED,
                               640, 384, SDL_WINDOW_SHOWN);

    if (!mWindow) {
        std::cerr << "Window could not be created! SDL_Error: " << SDL_GetError() << std::endl;
        return;
    }

    mRenderer = SDL_CreateRenderer(mWindow, -1, SDL_RENDERER_ACCELERATED);
    if (!mRenderer) {
        std::cerr << "Renderer could not be created! SDL_Error: " << SDL_GetError() << std::endl;
    }

    SDL_StartTextInput();
}

ConsoleSDL::~ConsoleSDL() {
    SDL_StopTextInput();
    if (mRenderer) {
        SDL_DestroyRenderer(mRenderer);
    }
    if (mWindow) {
        SDL_DestroyWindow(mWindow);
    }
    SDL_Quit();
}

int ConsoleSDL::Run() {
    SDL_Event e;
    uint32_t last_log = 0;
    printf("ConsoleSDL: Entering event loop\n");
    while (!mQuit) {
        while (SDL_PollEvent(&e)) {
            if (e.type == SDL_QUIT) {
                printf("ConsoleSDL: Quit event received\n");
                mQuit = true;
            } else if (e.type == SDL_TEXTINPUT) {
                // printf("ConsoleSDL: TextInput: %s\n", e.text.text);
                std::scoped_lock lck(mLock);
                for (int i = 0; e.text.text[i] != '\0'; i++) {
                    mInBuffer.push(e.text.text[i]);
                }
                NotifyInBufferCountAdd(mInBuffer.size());
            } else if (e.type == SDL_KEYDOWN) {
                // printf("ConsoleSDL: KeyDown: sym=%d mod=%d\n", e.key.keysym.sym, e.key.keysym.mod);
                std::scoped_lock lck(mLock);
                // Handle non-printable keys manually
                switch (e.key.keysym.sym) {
                    case SDLK_BACKSPACE:
                        mInBuffer.push(0x08);
                        break;
                    case SDLK_RETURN:
                        mInBuffer.push(0x0d);
                        break;
                    case SDLK_ESCAPE:
                        mInBuffer.push(0x1b);
                        break;
                    // Provide a way to manually exit on Ctrl-D (EOT / 0x4)
                    case SDLK_d:
                        if (e.key.keysym.mod & KMOD_CTRL) {
                            std::cout << "ctrl-d hit on SDL console, exiting" << std::endl;
                            mQuit = true;
                        }
                        break;
                }
                NotifyInBufferCountAdd(mInBuffer.size());
            }
        }

        if (mNeedsRefresh) {
            uint32_t now = SDL_GetTicks();
            if (now - last_log > 1000) {
                printf("ConsoleSDL: Frame refresh (throttled log)\n");
                last_log = now;
            }
            SDL_SetRenderDrawColor(mRenderer, 0, 0, 0, 255);
            SDL_RenderClear(mRenderer);
            if (mDrawCallback) {
                mDrawCallback(mRenderer);
            }
            SDL_RenderPresent(mRenderer);
            mNeedsRefresh = false;
        }

        SDL_Delay(16); // Cap looping to roughly 60Hz
    }

    return -1; // Same standard EOF loop escape
}

void ConsoleSDL::Stop() {
    mQuit = true;
    SDL_Event event = {};
    event.type = SDL_QUIT;
    SDL_PushEvent(&event);
}
