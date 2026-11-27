#include "fsr31-native.h"
#define NOMINMAX
#include <windows.h>
#include <d3d12.h>
#include <ffx_api/ffx_api.h>
#include <ffx_api/ffx_framegeneration.h>
#include <ffx_api/dx12/ffx_api_dx12.h>
#include <cmath>
#include <memory>
#include <algorithm>
#include <new>
#include <initializer_list>

struct VfgContext {
    HMODULE library{};
    ffxContext effect{};
    PfnFfxCreateContext create{};
    PfnFfxDestroyContext destroy{};
    PfnFfxConfigure configure{};
    PfnFfxDispatch dispatch{};
    VfgCreate desc{};
    uint64_t previous_id{};
    bool has_history{}, failed{};
    ~VfgContext() { if (library) FreeLibrary(library); }
};

extern "C" int32_t vfg_create(const wchar_t* path, const VfgCreate* d, VfgContext** out) {
    if (!out) return -1;
    *out = nullptr;
    if (!path || !d || !d->device || !d->display_width || !d->display_height ||
        !d->max_render_width || !d->max_render_height ||
        d->max_render_width > d->display_width || d->max_render_height > d->display_height ||
        (d->flags & ~(FFX_FRAMEGENERATION_ENABLE_DEPTH_INVERTED |
                      FFX_FRAMEGENERATION_ENABLE_DEPTH_INFINITE |
                      FFX_FRAMEGENERATION_ENABLE_MOTION_VECTORS_JITTER_CANCELLATION |
                      FFX_FRAMEGENERATION_ENABLE_DEBUG_CHECKING))) return -1;
    auto c = std::unique_ptr<VfgContext>(new (std::nothrow) VfgContext{});
    if (!c) return -2;
    c->library = LoadLibraryExW(path, nullptr, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR |
                                            LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
    if (!c->library) return -3;
    c->create = reinterpret_cast<PfnFfxCreateContext>(GetProcAddress(c->library, "ffxCreateContext"));
    c->destroy = reinterpret_cast<PfnFfxDestroyContext>(GetProcAddress(c->library, "ffxDestroyContext"));
    c->configure = reinterpret_cast<PfnFfxConfigure>(GetProcAddress(c->library, "ffxConfigure"));
    c->dispatch = reinterpret_cast<PfnFfxDispatch>(GetProcAddress(c->library, "ffxDispatch"));
    if (!c->create || !c->destroy || !c->configure || !c->dispatch) return -4;
    c->desc = *d;
    ffxCreateBackendDX12Desc backend{};
    backend.header.type = FFX_API_CREATE_CONTEXT_DESC_TYPE_BACKEND_DX12;
    backend.device = static_cast<ID3D12Device*>(d->device);
    ffxCreateContextDescFrameGeneration fg{};
    fg.header.type = FFX_API_CREATE_CONTEXT_DESC_TYPE_FRAMEGENERATION;
    fg.header.pNext = &backend.header;
    fg.flags = d->flags;
    fg.displaySize = {d->display_width, d->display_height};
    fg.maxRenderSize = {d->max_render_width, d->max_render_height};
    fg.backBufferFormat = FFX_API_SURFACE_FORMAT_R8G8B8A8_UNORM;
    auto result = c->create(&c->effect, &fg.header, nullptr);
    if (result || !c->effect) return result ? static_cast<int32_t>(result) : -5;
    *out = c.release();
    return 0;
}

static bool dimensions(ID3D12Resource* r, uint32_t w, uint32_t h) {
    if (!r) return false;
    auto d = r->GetDesc();
    return d.Dimension == D3D12_RESOURCE_DIMENSION_TEXTURE2D &&
        d.Width == w && d.Height == h && d.DepthOrArraySize == 1 &&
        d.SampleDesc.Count == 1 && d.MipLevels == 1;
}
static bool readable(ID3D12Resource* r) {
    return !(r->GetDesc().Flags & D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE);
}
static bool valid_camera(const VfgFrame& f, uint32_t flags) {
    const float vals[] = {f.jitter_x, f.jitter_y, f.motion_scale_x, f.motion_scale_y,
        f.delta_ms, f.near_z, f.far_z, f.vertical_fov, f.meters_factor};
    for (float v : vals) if (!std::isfinite(v)) return false;
    if (f.delta_ms <= 0 || f.near_z <= 0 || f.vertical_fov <= 0 ||
        f.vertical_fov >= 3.141593f || f.meters_factor <= 0 ||
        f.motion_scale_x == 0 || f.motion_scale_y == 0) return false;
    if (!(flags & FFX_FRAMEGENERATION_ENABLE_DEPTH_INFINITE) && f.far_z <= f.near_z) return false;
    for (auto p : {f.camera_position, f.camera_up, f.camera_right, f.camera_forward})
        for (int i = 0; i < 3; ++i) if (!std::isfinite(p[i])) return false;
    for (auto p : {f.camera_up, f.camera_right, f.camera_forward}) {
        float n = p[0]*p[0] + p[1]*p[1] + p[2]*p[2];
        if (std::abs(n-1.f) > .02f) return false;
    }
    return true;
}
extern "C" int32_t vfg_record(VfgContext* c, const VfgFrame* f) {
    if (!c || c->failed || !f || !f->list || !f->render_width || !f->render_height ||
        f->render_width > c->desc.max_render_width || f->render_height > c->desc.max_render_height ||
        !valid_camera(*f, c->desc.flags)) return -10;
    auto scene = static_cast<ID3D12Resource*>(f->scene);
    auto depth = static_cast<ID3D12Resource*>(f->depth);
    auto motion = static_cast<ID3D12Resource*>(f->motion);
    auto output = static_cast<ID3D12Resource*>(f->output);
    const auto w = c->desc.display_width, h = c->desc.display_height;
    if (!dimensions(scene,w,h) || !dimensions(output,w,h) ||
        !dimensions(depth,f->render_width,f->render_height) ||
        !dimensions(motion,f->render_width,f->render_height) ||
        !readable(scene) || !readable(depth) || !readable(motion) ||
        scene == output || depth == motion || depth == output || motion == output ||
        scene->GetDesc().Format != DXGI_FORMAT_R8G8B8A8_UNORM ||
        output->GetDesc().Format != DXGI_FORMAT_R8G8B8A8_UNORM ||
        !(output->GetDesc().Flags & D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS)) return -11;
    // Do not infer vector semantics from format. Caller supplies a verified RG float texture.
    const auto mf = motion->GetDesc().Format;
    if (mf != DXGI_FORMAT_R16G16_FLOAT && mf != DXGI_FORMAT_R32G32_FLOAT) return -12;
    // Explicit R32_FLOAT depth view/copy avoids guessing how the title's
    // typeless depth/stencil or inverted depth should be exposed to FSR.
    if (depth->GetDesc().Format != DXGI_FORMAT_R32_FLOAT) return -13;
    bool reset = f->reset || !c->has_history || f->frame_id != c->previous_id + 1;
    ffxConfigureDescFrameGeneration config{};
    config.header.type = FFX_API_CONFIGURE_DESC_TYPE_FRAMEGENERATION;
    config.frameGenerationEnabled = true;
    config.allowAsyncWorkloads = false;
    config.flags = FFX_FRAMEGENERATION_FLAG_NO_SWAPCHAIN_CONTEXT_NOTIFY;
    config.frameID = f->frame_id;
    auto result = c->configure(&c->effect, &config.header);
    if (result) { c->failed = true; return static_cast<int32_t>(result); }
    ffxDispatchDescFrameGenerationPrepareCameraInfo camera{};
    camera.header.type = FFX_API_DISPATCH_DESC_TYPE_FRAMEGENERATION_PREPARE_CAMERAINFO;
    std::copy_n(f->camera_position,3,camera.cameraPosition);
    std::copy_n(f->camera_up,3,camera.cameraUp);
    std::copy_n(f->camera_right,3,camera.cameraRight);
    std::copy_n(f->camera_forward,3,camera.cameraForward);
    ffxDispatchDescFrameGenerationPrepare prep{};
    prep.header.type = FFX_API_DISPATCH_DESC_TYPE_FRAMEGENERATION_PREPARE;
    prep.header.pNext = &camera.header;
    prep.frameID = f->frame_id;
    prep.commandList = f->list;
    prep.renderSize = {f->render_width,f->render_height};
    prep.jitterOffset = {f->jitter_x,f->jitter_y};
    prep.motionVectorScale = {f->motion_scale_x,f->motion_scale_y};
    prep.frameTimeDelta = f->delta_ms;
    prep.cameraNear = f->near_z;
    prep.cameraFar = f->far_z;
    prep.cameraFovAngleVertical = f->vertical_fov;
    prep.viewSpaceToMetersFactor = f->meters_factor;
    prep.depth = ffxApiGetResourceDX12(depth,FFX_API_RESOURCE_STATE_COMPUTE_READ);
    prep.motionVectors = ffxApiGetResourceDX12(motion,FFX_API_RESOURCE_STATE_COMPUTE_READ);
    result = c->dispatch(&c->effect,&prep.header);
    if (result) { c->failed = true; return static_cast<int32_t>(result); }
    ffxDispatchDescFrameGeneration gen{};
    gen.header.type = FFX_API_DISPATCH_DESC_TYPE_FRAMEGENERATION;
    gen.commandList = f->list;
    gen.presentColor = ffxApiGetResourceDX12(scene,FFX_API_RESOURCE_STATE_COMPUTE_READ);
    gen.outputs[0] = ffxApiGetResourceDX12(output,FFX_API_RESOURCE_STATE_UNORDERED_ACCESS);
    gen.numGeneratedFrames = 1;
    gen.reset = reset;
    gen.backbufferTransferFunction = FFX_API_BACKBUFFER_TRANSFER_FUNCTION_SRGB;
    gen.frameID = f->frame_id;
    result = c->dispatch(&c->effect,&gen.header);
    if (result) { c->failed = true; return static_cast<int32_t>(result); }
    c->has_history = true;
    c->previous_id = f->frame_id;
    return 0;
}
extern "C" int32_t vfg_destroy(VfgContext* c) {
    if (!c) return 0;
    const auto result = c->destroy(&c->effect,nullptr);
    if (result) return static_cast<int32_t>(result);
    delete c;
    return 0;
}
