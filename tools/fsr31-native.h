// Native FSR 3.1.4 D3D12 dispatch interface. No title data or AMD binary bundled.
#pragma once
#include <stdint.h>
#ifdef _WIN32
#define VFG_EXPORT __declspec(dllexport)
#else
#define VFG_EXPORT
#endif
#ifdef __cplusplus
extern "C" {
#endif
typedef struct VfgContext VfgContext;
typedef struct VfgCreate {
    void* device; // ID3D12Device*
    uint32_t display_width, display_height, max_render_width, max_render_height;
    uint32_t flags; // FfxApiCreateContextFramegenerationFlags (SDR, synchronous only)
} VfgCreate;
typedef struct VfgFrame {
    void* list; // open ID3D12GraphicsCommandList*, DIRECT
    void* scene; // HUD-free display-sized R8G8B8A8_UNORM, NON_PIXEL_SHADER_RESOURCE
    void* depth; // verified render-sized depth SRV, NON_PIXEL_SHADER_RESOURCE
    void* motion; // verified render-sized motion vector SRV, NON_PIXEL_SHADER_RESOURCE
    void* output; // distinct display-sized R8G8B8A8_UNORM, UAV state and flag
    uint64_t frame_id;
    uint32_t render_width, render_height, reset;
    float jitter_x, jitter_y, motion_scale_x, motion_scale_y, delta_ms;
    float near_z, far_z, vertical_fov, meters_factor;
    float camera_position[3], camera_up[3], camera_right[3], camera_forward[3];
} VfgFrame;
// All calls on a single owner thread. Caller owns the device, list, texture lifetimes,
// resource barriers, submission/fences and presentation. No CPU readback.
// SDK library path must be absolute. Returns 0 on success; negative local error,
// positive FidelityFX status. After any failed record, discard its list and context.
// First/reset frame seeds history: never present its generated output.
VFG_EXPORT int32_t vfg_create(const wchar_t* sdk_path, const VfgCreate* desc, VfgContext** out);
VFG_EXPORT int32_t vfg_record(VfgContext* ctx, const VfgFrame* frame);
// Call only after all submitted SDK work has completed.
VFG_EXPORT int32_t vfg_destroy(VfgContext* ctx);
#ifdef __cplusplus
}
#endif
