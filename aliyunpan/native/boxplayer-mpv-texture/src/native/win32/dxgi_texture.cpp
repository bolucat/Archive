// OpenGL/D3D11 shared textures; unsupported adapters fall back to software.
#ifdef _WIN32

#include "../texture_share.h"
#include <windows.h>
#include <d3d11.h>
#include <dxgi.h>
#include <dxgi1_2.h>  // For IDXGIResource1 (NT shared handles)
#include <gl/GL.h>
#include <iostream>

// WGL_NV_DX_interop extension functions
typedef BOOL(WINAPI* PFNWGLDXSETRESOURCESHAREHANDLENVPROC)(void*, HANDLE);
typedef HANDLE(WINAPI* PFNWGLDXOPENDEVICENVPROC)(void*);
typedef BOOL(WINAPI* PFNWGLDXCLOSEDEVICENVPROC)(HANDLE);
typedef HANDLE(WINAPI* PFNWGLDXREGISTEROBJECTNVPROC)(HANDLE, void*, GLuint, GLenum, GLenum);
typedef BOOL(WINAPI* PFNWGLDXUNREGISTEROBJECTNVPROC)(HANDLE, HANDLE);
typedef BOOL(WINAPI* PFNWGLDXOBJECTACCESSNVPROC)(HANDLE, GLenum);
typedef BOOL(WINAPI* PFNWGLDXLOCKOBJECTSNVPROC)(HANDLE, GLint, HANDLE*);
typedef BOOL(WINAPI* PFNWGLDXUNLOCKOBJECTSNVPROC)(HANDLE, GLint, HANDLE*);

// OpenGL extension functions
typedef void(APIENTRY* PFNGLGENFRAMEBUFFERSPROC)(GLsizei, GLuint*);
typedef void(APIENTRY* PFNGLDELETEFRAMEBUFFERSPROC)(GLsizei, const GLuint*);
typedef void(APIENTRY* PFNGLBINDFRAMEBUFFERPROC)(GLenum, GLuint);
typedef void(APIENTRY* PFNGLFRAMEBUFFERTEXTURE2DPROC)(GLenum, GLenum, GLenum, GLuint, GLint);
typedef GLenum(APIENTRY* PFNGLCHECKFRAMEBUFFERSTATUSPROC)(GLenum);
typedef void(APIENTRY* PFNGLGENTEXTURESPROC)(GLsizei, GLuint*);
typedef void(APIENTRY* PFNGLDELETETEXTURESPROC)(GLsizei, const GLuint*);
typedef void(APIENTRY* PFNGLBINDTEXTUREPROC)(GLenum, GLuint);

// OpenGL constants
#define GL_FRAMEBUFFER 0x8D40
#define GL_COLOR_ATTACHMENT0 0x8CE0
#define GL_FRAMEBUFFER_COMPLETE 0x8CD5
#define GL_TEXTURE_2D 0x0DE1
#define GL_RGBA8 0x8058
#define GL_RGBA 0x1908
#define GL_UNSIGNED_BYTE 0x1401

// WGL_NV_DX_interop constants
#define WGL_ACCESS_READ_WRITE_NV 0x0001
#define WGL_ACCESS_READ_ONLY_NV 0x0000
#define WGL_ACCESS_WRITE_DISCARD_NV 0x0002

namespace mpv_texture {

static const int BUFFER_COUNT = 3;

// Per-texture resources for triple buffering
struct TextureSlot {
    ID3D11Texture2D* d3dTexture = nullptr;
    IDXGIKeyedMutex* keyedMutex = nullptr;
    HANDLE sharedHandle = nullptr;
    GLuint glTexture = 0;
    GLuint glFBO = 0;
    HANDLE wglDxObject = nullptr;
    std::weak_ptr<void> lease;
};

class DXGITextureShare : public ITextureShare {
public:
    DXGITextureShare() = default;
    ~DXGITextureShare() override { destroy(); }

    bool initialize(void* gl_context) override {
        m_hglrc = static_cast<HGLRC>(gl_context);

        // Load WGL extension functions
        if (!loadWGLExtensions()) {
            std::cerr << "[DXGI] Failed to load WGL_NV_DX_interop extension" << std::endl;
            return false;
        }

        // Load OpenGL extension functions
        if (!loadGLExtensions()) {
            std::cerr << "[DXGI] Failed to load OpenGL extensions" << std::endl;
            return false;
        }

        // Probe every adapter against the current WGL context. Never select
        // a vendor by name: hybrid GPUs must share the same rendering device.
        IDXGIFactory1* factory = nullptr;
        if (FAILED(CreateDXGIFactory1(__uuidof(IDXGIFactory1), (void**)&factory))) return false;
        for (UINT index = 0; !m_wglDxDevice; ++index) {
            IDXGIAdapter1* adapter = nullptr;
            if (factory->EnumAdapters1(index, &adapter) == DXGI_ERROR_NOT_FOUND) break;
            D3D_FEATURE_LEVEL levels[] = {D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1};
            HRESULT result = D3D11CreateDevice(adapter, D3D_DRIVER_TYPE_UNKNOWN, nullptr,
                D3D11_CREATE_DEVICE_BGRA_SUPPORT, levels, 2, D3D11_SDK_VERSION,
                &m_d3dDevice, nullptr, &m_d3dContext);
            adapter->Release();
            if (SUCCEEDED(result)) m_wglDxDevice = m_wglDXOpenDeviceNV(m_d3dDevice);
            if (!m_wglDxDevice) {
                if (m_d3dContext) { m_d3dContext->Release(); m_d3dContext = nullptr; }
                if (m_d3dDevice) { m_d3dDevice->Release(); m_d3dDevice = nullptr; }
            }
        }
        factory->Release();
        if (!m_wglDxDevice) return false;

        m_initialized = true;
        return true;
    }

    bool createTexture(uint32_t width, uint32_t height) override {
        if (!m_initialized) return false;

        m_width = width;
        m_height = height;

        for (int i = 0; i < BUFFER_COUNT; i++) {
            if (!createSlot(m_slots[i], width, height)) {
                // Clean up any slots already created
                for (int j = 0; j <= i; j++) {
                    destroySlot(m_slots[j]);
                }
                return false;
            }
        }

        m_writeIndex = 0;
        std::cout << "[DXGI] Created " << BUFFER_COUNT << " triple-buffered textures "
                  << width << "x" << height << std::endl;
        return true;
    }

    bool resizeTexture(uint32_t width, uint32_t height) override {
        if (width == m_width && height == m_height) {
            return true;
        }

        // Destroy all slots and recreate
        for (int i = 0; i < BUFFER_COUNT; i++) {
            destroySlot(m_slots[i]);
        }

        return createTexture(width, height);
    }

    uint32_t getGLTexture() const override {
        return m_slots[m_writeIndex].glTexture;
    }

    uint32_t getGLFBO() const override {
        return m_slots[m_writeIndex].glFBO;
    }

    bool lockTexture() override {
        bool available = false;
        for (int offset = 0; offset < BUFFER_COUNT; ++offset) {
            int index = (m_writeIndex + offset) % BUFFER_COUNT;
            if (m_slots[index].lease.expired()) { m_writeIndex = index; available = true; break; }
        }
        if (!available) return false;
        auto& slot = m_slots[m_writeIndex];
        if (!slot.wglDxObject) {
            std::cerr << "[DXGI] lockTexture: No DX object" << std::endl;
            return false;
        }

        if (m_locked) {
            return true;
        }

        HANDLE objects[] = { slot.wglDxObject };
        if (!m_wglDXLockObjectsNV(m_wglDxDevice, 1, objects)) {
            DWORD err = GetLastError();
            std::cerr << "[DXGI] Failed to lock DX object, error: " << err << std::endl;
            return false;
        }

        m_locked = true;
        return true;
    }

    TextureInfo unlockAndExport() override {
        TextureInfo info = {};

        if (!m_locked) {
            return info;
        }

        auto& slot = m_slots[m_writeIndex];

        if (m_softwareReadback) {
            info.pixels = std::make_shared<std::vector<uint8_t>>(static_cast<size_t>(m_width) * m_height * 4);
            m_glBindFramebuffer(GL_FRAMEBUFFER, slot.glFBO);
            glReadPixels(0, 0, m_width, m_height, GL_RGBA, GL_UNSIGNED_BYTE, info.pixels->data());
            m_glBindFramebuffer(GL_FRAMEBUFFER, 0);
        }
        glFinish();
        // Unlock via WGL interop
        HANDLE objects[] = { slot.wglDxObject };
        if (!m_wglDXUnlockObjectsNV(m_wglDxDevice, 1, objects)) {
            std::cerr << "[DXGI] Failed to unlock DX object" << std::endl;
        }

        m_locked = false;

        HANDLE leasedHandle = nullptr;
        if (!info.pixels && !DuplicateHandle(GetCurrentProcess(), slot.sharedHandle, GetCurrentProcess(),
            &leasedHandle, 0, FALSE, DUPLICATE_SAME_ACCESS)) return info;
        if (leasedHandle) {
            slot.d3dTexture->AddRef();
            auto* texture = slot.d3dTexture;
            info.lease = std::shared_ptr<void>(leasedHandle, [texture](void* handle) {
                CloseHandle(static_cast<HANDLE>(handle));
                texture->Release();
            });
            slot.lease = info.lease;
        }
        // Export this slot's handle
        info.handle = reinterpret_cast<uint64_t>(leasedHandle);
        info.width = m_width;
        info.height = m_height;
        info.format = TextureFormat::RGBA8;
        info.is_valid = true;

        // Swap to other buffer for next frame
        m_writeIndex = (m_writeIndex + 1) % BUFFER_COUNT;

        return info;
    }

    void setSoftwareReadback(bool enabled) override { m_softwareReadback = enabled; }

    void releaseTexture() override {
        // Slots are retained by the exported frame lease.
    }

    void destroy() override {
        if (m_locked) {
            auto& slot = m_slots[m_writeIndex];
            if (slot.wglDxObject) {
                HANDLE objects[] = { slot.wglDxObject };
                m_wglDXUnlockObjectsNV(m_wglDxDevice, 1, objects);
            }
            m_locked = false;
        }

        for (int i = 0; i < BUFFER_COUNT; i++) {
            destroySlot(m_slots[i]);
        }

        if (m_wglDxDevice) {
            m_wglDXCloseDeviceNV(m_wglDxDevice);
            m_wglDxDevice = nullptr;
        }

        if (m_d3dContext) {
            m_d3dContext->Release();
            m_d3dContext = nullptr;
        }

        if (m_d3dDevice) {
            m_d3dDevice->Release();
            m_d3dDevice = nullptr;
        }

        m_initialized = false;
    }

private:
    bool createSlot(TextureSlot& slot, uint32_t width, uint32_t height) {
        // Create D3D11 texture with NT shared handle (required for Electron's importSharedTexture)
        D3D11_TEXTURE2D_DESC desc = {};
        desc.Width = width;
        desc.Height = height;
        desc.MipLevels = 1;
        desc.ArraySize = 1;
        desc.Format = DXGI_FORMAT_R8G8B8A8_UNORM;
        desc.SampleDesc.Count = 1;
        desc.Usage = D3D11_USAGE_DEFAULT;
        desc.BindFlags = D3D11_BIND_RENDER_TARGET | D3D11_BIND_SHADER_RESOURCE;
        desc.MiscFlags = D3D11_RESOURCE_MISC_SHARED_NTHANDLE | D3D11_RESOURCE_MISC_SHARED;

        HRESULT hr = m_d3dDevice->CreateTexture2D(&desc, nullptr, &slot.d3dTexture);
        if (FAILED(hr)) {
            std::cerr << "[DXGI] Failed to create D3D11 texture: " << std::hex << hr << std::endl;
            return false;
        }

        // Get NT shared handle via IDXGIResource1::CreateSharedHandle
        IDXGIResource1* dxgiResource1 = nullptr;
        hr = slot.d3dTexture->QueryInterface(__uuidof(IDXGIResource1), (void**)&dxgiResource1);
        if (FAILED(hr)) {
            std::cerr << "[DXGI] Failed to get IDXGIResource1: " << std::hex << hr << std::endl;
            return false;
        }

        hr = dxgiResource1->CreateSharedHandle(
            nullptr,
            DXGI_SHARED_RESOURCE_READ | DXGI_SHARED_RESOURCE_WRITE,
            nullptr,
            &slot.sharedHandle
        );
        dxgiResource1->Release();
        if (FAILED(hr)) {
            std::cerr << "[DXGI] Failed to create NT shared handle: " << std::hex << hr << std::endl;
            return false;
        }

        // Create OpenGL texture
        glGenTextures(1, &slot.glTexture);
        glBindTexture(GL_TEXTURE_2D, slot.glTexture);
        glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, width, height, 0, GL_RGBA, GL_UNSIGNED_BYTE, nullptr);

        // Register D3D texture with OpenGL via WGL_NV_DX_interop
        slot.wglDxObject = m_wglDXRegisterObjectNV(
            m_wglDxDevice,
            slot.d3dTexture,
            slot.glTexture,
            GL_TEXTURE_2D,
            WGL_ACCESS_WRITE_DISCARD_NV
        );

        if (!slot.wglDxObject) {
            DWORD err = GetLastError();
            std::cerr << "[DXGI] Failed to register DX object with WGL, error: " << err << std::endl;
            return false;
        }

        // Lock for FBO setup
        HANDLE objects[] = { slot.wglDxObject };
        if (!m_wglDXLockObjectsNV(m_wglDxDevice, 1, objects)) {
            std::cerr << "[DXGI] Failed to lock DX object for FBO setup" << std::endl;
            return false;
        }

        // Create FBO
        m_glGenFramebuffers(1, &slot.glFBO);
        m_glBindFramebuffer(GL_FRAMEBUFFER, slot.glFBO);
        m_glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, slot.glTexture, 0);

        GLenum status = m_glCheckFramebufferStatus(GL_FRAMEBUFFER);

        // Unlock after FBO setup
        m_wglDXUnlockObjectsNV(m_wglDxDevice, 1, objects);

        if (status != GL_FRAMEBUFFER_COMPLETE) {
            std::cerr << "[DXGI] FBO incomplete: " << std::hex << status << std::endl;
            return false;
        }

        m_glBindFramebuffer(GL_FRAMEBUFFER, 0);
        return true;
    }

    void destroySlot(TextureSlot& slot) {
        if (slot.wglDxObject) {
            m_wglDXUnregisterObjectNV(m_wglDxDevice, slot.wglDxObject);
            slot.wglDxObject = nullptr;
        }
        if (slot.glFBO) {
            m_glDeleteFramebuffers(1, &slot.glFBO);
            slot.glFBO = 0;
        }
        if (slot.glTexture) {
            glDeleteTextures(1, &slot.glTexture);
            slot.glTexture = 0;
        }
        if (slot.keyedMutex) {
            slot.keyedMutex->Release();
            slot.keyedMutex = nullptr;
        }
        if (slot.sharedHandle) {
            CloseHandle(slot.sharedHandle);
            slot.sharedHandle = nullptr;
        }
        if (slot.d3dTexture) {
            slot.d3dTexture->Release();
            slot.d3dTexture = nullptr;
        }
    }

    bool loadWGLExtensions() {
        // Get wglGetProcAddress
        HMODULE opengl32 = LoadLibraryA("opengl32.dll");
        if (!opengl32) return false;

        auto wglGetProcAddress = reinterpret_cast<PROC(WINAPI*)(LPCSTR)>(
            GetProcAddress(opengl32, "wglGetProcAddress"));
        if (!wglGetProcAddress) return false;

        // Load WGL_NV_DX_interop functions
        m_wglDXSetResourceShareHandleNV = reinterpret_cast<PFNWGLDXSETRESOURCESHAREHANDLENVPROC>(
            wglGetProcAddress("wglDXSetResourceShareHandleNV"));
        m_wglDXOpenDeviceNV = reinterpret_cast<PFNWGLDXOPENDEVICENVPROC>(
            wglGetProcAddress("wglDXOpenDeviceNV"));
        m_wglDXCloseDeviceNV = reinterpret_cast<PFNWGLDXCLOSEDEVICENVPROC>(
            wglGetProcAddress("wglDXCloseDeviceNV"));
        m_wglDXRegisterObjectNV = reinterpret_cast<PFNWGLDXREGISTEROBJECTNVPROC>(
            wglGetProcAddress("wglDXRegisterObjectNV"));
        m_wglDXUnregisterObjectNV = reinterpret_cast<PFNWGLDXUNREGISTEROBJECTNVPROC>(
            wglGetProcAddress("wglDXUnregisterObjectNV"));
        m_wglDXLockObjectsNV = reinterpret_cast<PFNWGLDXLOCKOBJECTSNVPROC>(
            wglGetProcAddress("wglDXLockObjectsNV"));
        m_wglDXUnlockObjectsNV = reinterpret_cast<PFNWGLDXUNLOCKOBJECTSNVPROC>(
            wglGetProcAddress("wglDXUnlockObjectsNV"));

        // Note: wglDXSetResourceShareHandleNV may be null on older drivers, we'll check before use
        return m_wglDXOpenDeviceNV && m_wglDXCloseDeviceNV &&
               m_wglDXRegisterObjectNV && m_wglDXUnregisterObjectNV &&
               m_wglDXLockObjectsNV && m_wglDXUnlockObjectsNV;
    }

    bool loadGLExtensions() {
        HMODULE opengl32 = LoadLibraryA("opengl32.dll");
        if (!opengl32) return false;

        auto wglGetProcAddress = reinterpret_cast<PROC(WINAPI*)(LPCSTR)>(
            GetProcAddress(opengl32, "wglGetProcAddress"));
        if (!wglGetProcAddress) return false;

        m_glGenFramebuffers = reinterpret_cast<PFNGLGENFRAMEBUFFERSPROC>(
            wglGetProcAddress("glGenFramebuffers"));
        m_glDeleteFramebuffers = reinterpret_cast<PFNGLDELETEFRAMEBUFFERSPROC>(
            wglGetProcAddress("glDeleteFramebuffers"));
        m_glBindFramebuffer = reinterpret_cast<PFNGLBINDFRAMEBUFFERPROC>(
            wglGetProcAddress("glBindFramebuffer"));
        m_glFramebufferTexture2D = reinterpret_cast<PFNGLFRAMEBUFFERTEXTURE2DPROC>(
            wglGetProcAddress("glFramebufferTexture2D"));
        m_glCheckFramebufferStatus = reinterpret_cast<PFNGLCHECKFRAMEBUFFERSTATUSPROC>(
            wglGetProcAddress("glCheckFramebufferStatus"));

        return m_glGenFramebuffers && m_glDeleteFramebuffers &&
               m_glBindFramebuffer && m_glFramebufferTexture2D &&
               m_glCheckFramebufferStatus;
    }

    // State
    bool m_softwareReadback = false;
    bool m_initialized = false;
    bool m_locked = false;
    uint32_t m_width = 0;
    uint32_t m_height = 0;

    // Triple-buffered texture slots
    TextureSlot m_slots[BUFFER_COUNT];
    int m_writeIndex = 0;

    // D3D11 (shared across slots)
    ID3D11Device* m_d3dDevice = nullptr;
    ID3D11DeviceContext* m_d3dContext = nullptr;

    // OpenGL
    HGLRC m_hglrc = nullptr;

    // WGL/DX interop
    HANDLE m_wglDxDevice = nullptr;

    // WGL extension functions
    PFNWGLDXSETRESOURCESHAREHANDLENVPROC m_wglDXSetResourceShareHandleNV = nullptr;
    PFNWGLDXOPENDEVICENVPROC m_wglDXOpenDeviceNV = nullptr;
    PFNWGLDXCLOSEDEVICENVPROC m_wglDXCloseDeviceNV = nullptr;
    PFNWGLDXREGISTEROBJECTNVPROC m_wglDXRegisterObjectNV = nullptr;
    PFNWGLDXUNREGISTEROBJECTNVPROC m_wglDXUnregisterObjectNV = nullptr;
    PFNWGLDXLOCKOBJECTSNVPROC m_wglDXLockObjectsNV = nullptr;
    PFNWGLDXUNLOCKOBJECTSNVPROC m_wglDXUnlockObjectsNV = nullptr;

    // OpenGL extension functions
    PFNGLGENFRAMEBUFFERSPROC m_glGenFramebuffers = nullptr;
    PFNGLDELETEFRAMEBUFFERSPROC m_glDeleteFramebuffers = nullptr;
    PFNGLBINDFRAMEBUFFERPROC m_glBindFramebuffer = nullptr;
    PFNGLFRAMEBUFFERTEXTURE2DPROC m_glFramebufferTexture2D = nullptr;
    PFNGLCHECKFRAMEBUFFERSTATUSPROC m_glCheckFramebufferStatus = nullptr;
};

// Factory function
ITextureShare* createTextureShare() {
    return new DXGITextureShare();
}

} // namespace mpv_texture

#endif // _WIN32
