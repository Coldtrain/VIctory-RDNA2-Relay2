// Standalone FidelityFX SDK 1.1.4 D3D12 frame-generation context smoke test.
// Compile against the official AMD headers; no game files or bundled DLL.
#include <windows.h>
#include <wrl/client.h>
#include <d3d12.h>
#include <dxgi1_6.h>
#include <ffx_api/ffx_api.h>
#include <ffx_api/ffx_framegeneration.h>
#include <ffx_api/dx12/ffx_api_dx12.h>
#include <cstdio>
#include <cwchar>
using Microsoft::WRL::ComPtr;
int wmain(int argc, wchar_t** argv) {
    if (argc != 2) {
        std::fwprintf(stderr, L"Usage: fsr31-probe.exe <absolute path to SDK 1.1.4 amd_fidelityfx_dx12.dll>\n");
        return 2;
    }
    HMODULE dll = LoadLibraryExW(argv[1], nullptr,
        LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS);
    if (!dll) {
        std::fprintf(stderr, "LoadLibraryExW failed: %lu\n", GetLastError());
        return 3;
    }
    auto create = reinterpret_cast<PfnFfxCreateContext>(GetProcAddress(dll, "ffxCreateContext"));
    auto destroy = reinterpret_cast<PfnFfxDestroyContext>(GetProcAddress(dll, "ffxDestroyContext"));
    if (!create || !destroy || !GetProcAddress(dll, "ffxConfigure") ||
        !GetProcAddress(dll, "ffxDispatch")) {
        std::fprintf(stderr, "Missing FidelityFX API exports\n");
        return 4;
    }
    ComPtr<IDXGIFactory6> factory;
    if (FAILED(CreateDXGIFactory2(0, IID_PPV_ARGS(&factory)))) return 5;
    ComPtr<ID3D12Device> device;
    for (UINT i = 0;; ++i) {
        ComPtr<IDXGIAdapter1> adapter;
        if (factory->EnumAdapters1(i, &adapter) != S_OK) break;
        DXGI_ADAPTER_DESC1 desc{};
        if (FAILED(adapter->GetDesc1(&desc)) || desc.VendorId != 0x1002 ||
            (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE)) continue;
        if (SUCCEEDED(D3D12CreateDevice(adapter.Get(), D3D_FEATURE_LEVEL_12_0,
            IID_PPV_ARGS(&device)))) {
            std::wprintf(L"AMD adapter: %ls\n", desc.Description);
            break;
        }
    }
    if (!device) {
        std::fprintf(stderr, "No AMD D3D12 adapter found\n");
        return 6;
    }
    ffxCreateBackendDX12Desc backend{};
    backend.header.type = FFX_API_CREATE_CONTEXT_DESC_TYPE_BACKEND_DX12;
    backend.device = device.Get();
    ffxCreateContextDescFrameGeneration fg{};
    fg.header.type = FFX_API_CREATE_CONTEXT_DESC_TYPE_FRAMEGENERATION;
    fg.header.pNext = &backend.header;
    fg.displaySize = {1920, 1080};
    fg.maxRenderSize = {1920, 1080};
    fg.backBufferFormat = FFX_API_SURFACE_FORMAT_R8G8B8A8_UNORM;
    ffxContext context = nullptr;
    const auto result = create(&context, &fg.header, nullptr);
    std::printf("ffxCreateContext(frame generation): %u\n", unsigned(result));
    if (result != 0 || !context) return 7;
    const auto closed = destroy(&context, nullptr);
    std::printf("ffxDestroyContext: %u\n", unsigned(closed));
    if (closed != 0) return 8;
    FreeLibrary(dll);
    std::puts("SDK context smoke test passed; no frames generated.");
    return 0;
}
