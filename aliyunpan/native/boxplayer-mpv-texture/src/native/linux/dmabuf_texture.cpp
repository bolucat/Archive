// EGL render targets exported as DMA-BUF. All GL/EGL calls stay on the render thread.
#ifdef __linux__
#include "../texture_share.h"
#define GL_GLEXT_PROTOTYPES
#include <GL/gl.h>
#include <GL/glext.h>
#include <EGL/egl.h>
#include <EGL/eglext.h>
#include <unistd.h>
#include <array>
#include <cstring>

namespace mpv_texture {
class DmaBufTextureShare final : public ITextureShare {
    struct Slot {
        GLuint texture = 0;
        GLuint fbo = 0;
        EGLImageKHR image = EGL_NO_IMAGE_KHR;
        std::weak_ptr<void> lease;
    };
    std::array<Slot, 3> slots{};
    EGLDisplay display = EGL_NO_DISPLAY;
    EGLContext context = EGL_NO_CONTEXT;
    EGLSurface surface = EGL_NO_SURFACE;
    PFNEGLCREATEIMAGEKHRPROC createImage = nullptr;
    PFNEGLDESTROYIMAGEKHRPROC destroyImage = nullptr;
    PFNEGLEXPORTDMABUFIMAGEQUERYMESAPROC query = nullptr;
    PFNEGLEXPORTDMABUFIMAGEMESAPROC exportImage = nullptr;
    uint32_t width = 0, height = 0;
    size_t index = 0;
    bool readback = false;

    void clearSlot(Slot& slot) {
        if (slot.image != EGL_NO_IMAGE_KHR && destroyImage) destroyImage(display, slot.image);
        if (slot.fbo) glDeleteFramebuffers(1, &slot.fbo);
        if (slot.texture) glDeleteTextures(1, &slot.texture);
        slot = {};
    }
public:
    ~DmaBufTextureShare() override { destroy(); }
    bool initialize(void*) override {
        auto platformDisplay = reinterpret_cast<PFNEGLGETPLATFORMDISPLAYEXTPROC>(eglGetProcAddress("eglGetPlatformDisplayEXT"));
        if (platformDisplay) display = platformDisplay(EGL_PLATFORM_SURFACELESS_MESA, EGL_DEFAULT_DISPLAY, nullptr);
        if (display == EGL_NO_DISPLAY) display = eglGetDisplay(EGL_DEFAULT_DISPLAY);
        if (display == EGL_NO_DISPLAY || !eglInitialize(display, nullptr, nullptr)) return false;
        const char* extensions = eglQueryString(display, EGL_EXTENSIONS);
        if (!extensions || !strstr(extensions, "EGL_MESA_image_dma_buf_export")) return false;
        if (!eglBindAPI(EGL_OPENGL_API)) return false;
        const EGLint configAttrs[] = {EGL_SURFACE_TYPE, EGL_PBUFFER_BIT, EGL_RENDERABLE_TYPE, EGL_OPENGL_BIT,
            EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8, EGL_NONE};
        EGLConfig config;
        EGLint count = 0;
        if (!eglChooseConfig(display, configAttrs, &config, 1, &count) || !count) return false;
        context = eglCreateContext(display, config, EGL_NO_CONTEXT, nullptr);
        const EGLint surfaceAttrs[] = {EGL_WIDTH, 1, EGL_HEIGHT, 1, EGL_NONE};
        surface = eglCreatePbufferSurface(display, config, surfaceAttrs);
        if (context == EGL_NO_CONTEXT || surface == EGL_NO_SURFACE || !eglMakeCurrent(display, surface, surface, context)) return false;
        createImage = reinterpret_cast<PFNEGLCREATEIMAGEKHRPROC>(eglGetProcAddress("eglCreateImageKHR"));
        destroyImage = reinterpret_cast<PFNEGLDESTROYIMAGEKHRPROC>(eglGetProcAddress("eglDestroyImageKHR"));
        query = reinterpret_cast<PFNEGLEXPORTDMABUFIMAGEQUERYMESAPROC>(eglGetProcAddress("eglExportDMABUFImageQueryMESA"));
        exportImage = reinterpret_cast<PFNEGLEXPORTDMABUFIMAGEMESAPROC>(eglGetProcAddress("eglExportDMABUFImageMESA"));
        return createImage && destroyImage && query && exportImage;
    }
    bool createTexture(uint32_t w, uint32_t h) override {
        width = w; height = h;
        for (auto& slot : slots) {
            glGenTextures(1, &slot.texture);
            glBindTexture(GL_TEXTURE_2D, slot.texture);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_LINEAR);
            glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_LINEAR);
            glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, w, h, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);
            glGenFramebuffers(1, &slot.fbo);
            glBindFramebuffer(GL_FRAMEBUFFER, slot.fbo);
            glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, slot.texture, 0);
            if (glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE) return false;
            const EGLint attrs[] = {EGL_GL_TEXTURE_LEVEL_KHR, 0, EGL_NONE};
            slot.image = createImage(display, context, EGL_GL_TEXTURE_2D_KHR,
                reinterpret_cast<EGLClientBuffer>(static_cast<uintptr_t>(slot.texture)), attrs);
            if (slot.image == EGL_NO_IMAGE_KHR) return false;
            int format = 0, planes = 0;
            EGLuint64KHR modifier = 0;
            if (!query(display, slot.image, &format, &planes, &modifier) || planes != 1) return false;
            // Packed RGBA/BGRA only. Never mislabel a driver's YUV or auxiliary planes.
            if (format != 0x34324241 && format != 0x34325241) return false; // AB24 / AR24
        }
        glBindFramebuffer(GL_FRAMEBUFFER, 0);
        return true;
    }
    bool resizeTexture(uint32_t w, uint32_t h) override {
        if (w == width && h == height) return true;
        for (auto& slot : slots) clearSlot(slot);
        return createTexture(w, h);
    }
    uint32_t getGLTexture() const override { return slots[index].texture; }
    uint32_t getGLFBO() const override { return slots[index].fbo; }
    bool lockTexture() override {
        for (size_t offset = 0; offset < slots.size(); ++offset) {
            size_t next = (index + offset) % slots.size();
            if (slots[next].lease.expired()) { index = next; return true; }
        }
        return false;
    }
    TextureInfo unlockAndExport() override {
        TextureInfo frame{};
        auto& slot = slots[index];
        frame.width = width; frame.height = height;
        if (readback) {
            frame.pixels = std::make_shared<std::vector<uint8_t>>(static_cast<size_t>(width) * height * 4);
            glBindFramebuffer(GL_FRAMEBUFFER, slot.fbo);
            glReadPixels(0, 0, width, height, GL_RGBA, GL_UNSIGNED_BYTE, frame.pixels->data());
            glBindFramebuffer(GL_FRAMEBUFFER, 0);
            frame.format = TextureFormat::RGBA8;
        } else {
            // Conservative completion fence; ownership lasts through Electron's release callback.
            glFinish();
            int format = 0, planes = 0, fd = -1, stride = 0, offset = 0;
            EGLuint64KHR modifier = 0;
            if (!query(display, slot.image, &format, &planes, &modifier) || planes != 1) { readback = true; return frame; }
            if (!exportImage(display, slot.image, &fd, &stride, &offset)) { readback = true; return frame; }
            frame.modifier = modifier;
            off_t allocationSize = lseek(fd, 0, SEEK_END);
            frame.planes.push_back({fd, static_cast<uint32_t>(stride), static_cast<uint32_t>(offset),
                allocationSize > 0 ? static_cast<uint64_t>(allocationSize) : static_cast<uint64_t>(offset) + static_cast<uint64_t>(stride) * height});
            frame.format = format == 0x34324241 ? TextureFormat::RGBA8 : TextureFormat::BGRA8;
            frame.lease = std::shared_ptr<void>(new int(fd), [](void* value) {
                int* descriptor = static_cast<int*>(value); close(*descriptor); delete descriptor;
            });
            slot.lease = frame.lease;
        }
        frame.is_valid = true;
        index = (index + 1) % slots.size();
        return frame;
    }
    void setSoftwareReadback(bool enabled) override { readback = enabled; }
    void releaseTexture() override {}
    void destroy() override {
        if (context != EGL_NO_CONTEXT) {
            eglMakeCurrent(display, surface, surface, context);
            for (auto& slot : slots) clearSlot(slot);
            eglMakeCurrent(display, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
            eglDestroyContext(display, context);
            context = EGL_NO_CONTEXT;
        }
        if (surface != EGL_NO_SURFACE) { eglDestroySurface(display, surface); surface = EGL_NO_SURFACE; }
        if (display != EGL_NO_DISPLAY) { eglTerminate(display); display = EGL_NO_DISPLAY; }
    }
};
ITextureShare* createTextureShare() { return new DmaBufTextureShare(); }
}
#endif
