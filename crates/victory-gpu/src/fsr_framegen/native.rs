//! Native AMD FidelityFX SDK 1.1.4 DX12 frame generation, independent of
//! Streamline. FFX ABI definitions follow the MIT-licensed headers
//! ffx_api.h, ffx_api_types.h, ffx_framegeneration.h, dx12/ffx_api_dx12.h.
//! Copyright (C) 2024 Advanced Micro Devices, Inc. See docs/amd-fsr-framegen.md.
//! The SDK is supplied by the user, never loaded from the current directory.
use super::Captured;
use crate::host::{HostList, legal_state};
use std::cell::UnsafeCell;
use std::collections::VecDeque;
use std::ffi::{CStr, c_void};
use std::mem::offset_of;
use std::os::windows::ffi::OsStrExt;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D12::*;
use windows::Win32::Graphics::Dxgi::*;
use windows::Win32::Graphics::Dxgi::Common::*;
use windows::core::Interface;

type Context = usize;
#[repr(C)]
struct Header { kind: u64, next: *mut Header }
impl Header {
    fn new(kind: u64) -> Self { Self { kind, next: std::ptr::null_mut() } }
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Resource { ptr: *mut c_void, desc: [u32; 8], state: u32 }
impl Resource {
    fn empty() -> Self { Self { ptr: std::ptr::null_mut(), desc: [0; 8], state: 0 } }
}
#[repr(C)]
struct SwapDesc {
    header: Header, out: *mut *mut c_void, hwnd: *mut c_void,
    desc: *mut DXGI_SWAP_CHAIN_DESC1, fullscreen: *mut DXGI_SWAP_CHAIN_FULLSCREEN_DESC,
    factory: *mut c_void, queue: *mut c_void,
}
#[repr(C)]
struct DeviceDesc { header: Header, device: *mut c_void }
#[repr(C)]
struct FrameDesc {
    header: Header, flags: u32, display: [u32; 2], max_render: [u32; 2], format: u32,
}
type Callback = unsafe extern "C" fn(*mut Header, *mut c_void) -> u32;
#[repr(C)]
struct Configure {
    header: Header, swap: *mut c_void,
    present_cb: Option<Callback>, present_user: *mut c_void,
    generate_cb: Option<Callback>, generate_user: *mut c_void,
    enabled: bool, allow_async: bool, hudless: Resource,
    flags: u32, only_generated: bool, rect: [i32; 4], frame_id: u64,
}
#[repr(C)]
struct CameraInfo {
    header: Header, position: [f32; 3], up: [f32; 3], right: [f32; 3], forward: [f32; 3],
}
#[repr(C)]
struct Prepare {
    header: Header, frame_id: u64, flags: u32, list: *mut c_void,
    render: [u32; 2], jitter: [f32; 2], motion_scale: [f32; 2],
    frame_ms: f32, unused_reset: bool, near: f32, far: f32, fov: f32, meters: f32,
    depth: Resource, motion: Resource,
}
const _: () = {
    assert!(size_of::<Header>() == 16);
    assert!(size_of::<Resource>() == 48);
    assert!(size_of::<SwapDesc>() == 64);
    assert!(size_of::<DeviceDesc>() == 24);
    assert!(size_of::<FrameDesc>() == 40);
    assert!(size_of::<Configure>() == 144);
    assert!(size_of::<CameraInfo>() == 64);
    assert!(size_of::<Prepare>() == 184);
    assert!(offset_of!(Configure, hudless) == 64);
    assert!(offset_of!(Configure, frame_id) == 136);
    assert!(offset_of!(Prepare, depth) == 88);
};

type Create = unsafe extern "C" fn(*mut Context, *mut Header, *const c_void) -> u32;
type Destroy = unsafe extern "C" fn(*mut Context, *const c_void) -> u32;
type Call = unsafe extern "C" fn(*mut Context, *const Header) -> u32;
struct Api { create: Create, destroy: Destroy, configure: Call, dispatch: Call, _module: usize }
static API: OnceLock<Option<Api>> = OnceLock::new();
static CALLBACKS: AtomicU64 = AtomicU64::new(0);
static FAILED_CALLBACKS: AtomicU64 = AtomicU64::new(0);

fn api() -> Option<&'static Api> {
    API.get_or_init(|| match load() {
        Ok(a) => Some(a),
        Err(error) => { tracing::warn!(%error, "FSR FG: SDK unavailable; plain DXGI present"); None }
    }).as_ref()
}
fn load() -> Result<Api, String> {
    use windows_sys::Win32::System::LibraryLoader::{
        GetProcAddress, LoadLibraryExW, LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR, LOAD_LIBRARY_SEARCH_SYSTEM32,
    };
    let path = std::env::var_os("VICTORY_FSR_SDK_DLL").map(std::path::PathBuf::from)
        .ok_or("set VICTORY_FSR_SDK_DLL to the absolute path of SDK 1.1.4 amd_fidelityfx_dx12.dll")?;
    if !path.is_absolute() || !path.is_file() { return Err("SDK DLL must be an existing absolute path".into()); }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: NUL-terminated absolute path; dependencies only in SDK folder and System32.
    let module = unsafe { LoadLibraryExW(wide.as_ptr(), std::ptr::null_mut(), LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32) };
    if module.is_null() { return Err(std::io::Error::last_os_error().to_string()); }
    let find = |name: &CStr| unsafe { GetProcAddress(module, name.as_ptr().cast()) }
        .map(|f| f as *const c_void).ok_or_else(|| format!("missing SDK export {}", name.to_string_lossy()));
    macro_rules! get { ($name:literal, $t:ty) => { unsafe { std::mem::transmute::<*const c_void, $t>(find($name)?) } }; }
    let a = Api { create: get!(c"ffxCreateContext", Create), destroy: get!(c"ffxDestroyContext", Destroy),
        configure: get!(c"ffxConfigure", Call), dispatch: get!(c"ffxDispatch", Call), _module: module as usize };
    // Keep SDK DLL loaded for the host process, including the SDK presentation threads.
    tracing::info!(path = %path.display(), "FSR FG: loaded native AMD FidelityFX SDK");
    Ok(a)
}

// DXGI -> FfxApiSurfaceFormat as defined by the SDK 1.1.4 DX12 helper.
// Only the formats relevant to depth, motion and SDR display are accepted.
fn surface(dxgi: i32) -> Option<u32> {
    Some(match dxgi {
        10 => 4, 16 => 6, 19 | 20 | 21 | 40 | 41 => 28,
        33 => 33, 34 => 18, 36 => 19,
        39 => 37, 44 | 45 | 46 => 8, 53 => 34, 54 => 21, 55 | 56 => 23,
        27 => 9, 28 => 10, 29 => 12, 87 => 14, 90 => 13, 91 => 15,
        24 => 17, 26 => 16, 15 => 31,
        _ => return None,
    })
}
fn resource(texture: &ID3D12Resource, state: u32) -> Option<Resource> {
    let d = unsafe { texture.GetDesc() };
    if d.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || d.DepthOrArraySize != 1 ||
       d.SampleDesc.Count != 1 || d.Width > u32::MAX as u64 { return None; }
    let usage = (if d.Flags.0 & D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS.0 != 0 { 2 } else { 0 }) |
        (if matches!(d.Format.0, 40 | 55) { 4 } else if matches!(d.Format.0, 20 | 45) { 4 | 32 } else { 0 });
    Some(Resource { ptr: texture.as_raw(), desc: [2, surface(d.Format.0)?, d.Width as u32,
        d.Height, d.DepthOrArraySize as u32, d.MipLevels as u32, 0, usage], state })
}
fn ffx_state(state: u32) -> Option<u32> {
    Some(match state { 0 => 1, 8 => 2, 0x40 => 4, 0x80 => 8, 0xc0 => 12,
        0x800 => 16, 0x400 => 32, 4 => 256, _ => return None })
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Key { display: [u32; 2], max_render: [u32; 2], flags: u32, format: u32 }
struct Frame { ctx: Box<UnsafeCell<Context>>, key: Key }
impl Frame {
    fn handle(&self) -> *mut Context { self.ctx.get() }
}
/// The callback user pointer is a stable heap allocation for the lifetime
/// of the AMD frame context; no Rust mutex is entered from the SDK callback.
unsafe extern "C" fn generate(desc: *mut Header, user: *mut c_void) -> u32 {
    if desc.is_null() || user.is_null() { return 6; }
    let Some(api) = API.get().and_then(Option::as_ref) else { return 6; };
    let code = unsafe { (api.dispatch)(user.cast(), desc) };
    if code == 0 {
        let n = CALLBACKS.fetch_add(1, Ordering::Relaxed) + 1;
        if n == 1 || n % 120 == 0 { tracing::info!(n, "FSR FG: SDK interpolation dispatch completed"); }
    } else {
        FAILED_CALLBACKS.fetch_add(1, Ordering::Relaxed);
        tracing::warn!(code, "FSR FG: SDK interpolation dispatch failed");
    }
    code
}

/// One host presenter owns this, the swapchain and the frame context.
/// No async interpolation queue is requested for the first RDNA2 test.
pub(crate) struct Backend {
    swap_ctx: Context,
    frame: Option<Frame>,
    enabled: bool,
    faulted: bool,
    next_id: u64,
    prepared: u64,
    hudless: VecDeque<ID3D12Resource>,
}
impl Backend {
    pub(crate) fn status(&self) -> &'static str {
        if self.faulted || FAILED_CALLBACKS.load(Ordering::Relaxed) != 0 { "ERROR" }
        else if self.enabled { "ON" } else { "WAITING" }
    }
    pub(crate) fn callback_count(&self) -> u64 { CALLBACKS.load(Ordering::Relaxed) }

    pub(crate) fn create(factory: &IDXGIFactory2, queue: &ID3D12CommandQueue, hwnd: HWND,
        desc: &DXGI_SWAP_CHAIN_DESC1) -> Option<(IDXGISwapChain3, Self)> {
        let on = |k| std::env::var_os(k).is_some_and(|v| v == "1");
        if !on("VICTORY_FSR_FRAMEGEN") { return None; }
        if on("VICTORY_AFMF") || crate::console::current() != crate::console::Console::SeriesS ||
            !(on("VICTORY_THIN") || on("VICTORY_ASYNC_SUBMIT")) {
            tracing::warn!("FSR FG: requires Series S, ordered submission (THIN or ASYNC_SUBMIT), AFMF off; using plain DXGI");
            return None;
        }
        let api = api()?;
        let mut desc = *desc;
        let mut fullscreen = DXGI_SWAP_CHAIN_FULLSCREEN_DESC { Windowed: true.into(), ..Default::default() };
        let mut raw = std::ptr::null_mut();
        let mut swap_ctx = 0;
        let mut create = SwapDesc { header: Header::new(0x30006), out: &mut raw, hwnd: hwnd.0,
            desc: &mut desc, fullscreen: &mut fullscreen, factory: factory.as_raw(), queue: queue.as_raw() };
        let code = unsafe { (api.create)(&mut swap_ctx, &mut create.header, std::ptr::null()) };
        if code != 0 || raw.is_null() || swap_ctx == 0 {
            tracing::warn!(code, "FSR FG: AMD swapchain creation failed; using plain DXGI");
            if !raw.is_null() { drop(unsafe { IDXGISwapChain4::from_raw(raw) }); }
            if swap_ctx != 0 { unsafe { (api.destroy)(&mut swap_ctx, std::ptr::null()) }; }
            return None;
        }
        // SDK returns a caller-owned COM reference plus its own context reference.
        let chain4 = unsafe { IDXGISwapChain4::from_raw(raw) };
        let chain = match chain4.cast::<IDXGISwapChain3>() {
            Ok(chain) => chain,
            Err(e) => {
                tracing::warn!(%e, "FSR FG: swapchain lacks IDXGISwapChain3");
                drop(chain4);
                unsafe { (api.destroy)(&mut swap_ctx, std::ptr::null()) };
                return None;
            }
        };
        tracing::info!("FSR FG: native AMD interpolation swapchain created");
        Some((chain, Self { swap_ctx, frame: None, enabled: false, faulted: false,
            next_id: 0, prepared: 0, hudless: VecDeque::new() }))
    }

    fn configure(&self, swap: &IDXGISwapChain3, enabled: bool, id: u64, hudless: Resource) -> bool {
        let Some(frame) = &self.frame else { return !enabled; };
        let Some(api) = api() else { return false; };
        let [w,h] = frame.key.display;
        // Use the exact interface type returned by the SDK rather than
        // assuming a queried IDXGISwapChain3 pointer is identical.
        let Ok(swap4) = swap.cast::<IDXGISwapChain4>() else { return false; };
        let cfg = Configure { header: Header::new(0x20002), swap: swap4.as_raw(),
            present_cb: None, present_user: std::ptr::null_mut(),
            generate_cb: if enabled { Some(generate) } else { None },
            generate_user: if enabled { frame.handle().cast() } else { std::ptr::null_mut() },
            enabled, allow_async: false, hudless, flags: 0, only_generated: false,
            rect: [0,0,w as i32,h as i32], frame_id: id };
        let code = unsafe { (api.configure)(frame.handle(), &cfg.header) };
        if code != 0 { tracing::warn!(code, enabled, id, "FSR FG: configure failed"); }
        code == 0
    }
    fn disable(&mut self, swap: &IDXGISwapChain3, id: u64) -> bool {
        if !self.enabled { return true; }
        if !self.configure(swap, false, id, Resource::empty()) {
            self.faulted = true;
            return false;
        }
        self.enabled = false;
        tracing::info!(id, "FSR FG: disabled (no fresh/compatible gameplay inputs)");
        true
    }
    fn wait(&mut self) -> bool {
        let Some(api) = api() else { return false; };
        let header = Header::new(0x30007);
        let code = unsafe { (api.dispatch)(&mut self.swap_ctx, &header) };
        if code != 0 { tracing::warn!(code, "FSR FG: wait for presents failed"); return false; }
        self.hudless.clear();
        true
    }
    pub(crate) fn before_resize(&mut self, swap: &IDXGISwapChain3) -> bool {
        self.disable(swap, self.next_id) && self.wait()
    }

    fn ensure_frame(&mut self, device: &ID3D12Device, key: Key) -> bool {
        if let Some(frame) = &self.frame {
            if frame.key != key {
                static WARN: std::sync::Once = std::sync::Once::new();
                WARN.call_once(|| tracing::warn!(previous=?frame.key, requested=?key,
                    "FSR FG: frame context sizing/flags changed; interpolation stays off for incompatible frames"));
                return false;
            }
            return true;
        }
        let Some(api) = api() else { return false; };
        let ctx = Box::new(UnsafeCell::new(0));
        let mut backend = DeviceDesc { header: Header::new(2), device: device.as_raw() };
        let mut create = FrameDesc { header: Header::new(0x20001), flags: key.flags,
            display: key.display, max_render: key.max_render, format: key.format };
        create.header.next = &mut backend.header;
        let code = unsafe { (api.create)(ctx.get(), &mut create.header, std::ptr::null()) };
        if code != 0 || unsafe { *ctx.get() } == 0 {
            tracing::warn!(code, ?key, "FSR FG: AMD frame context creation failed");
            if unsafe { *ctx.get() } != 0 { unsafe { (api.destroy)(ctx.get(), std::ptr::null()) }; }
            self.faulted = true;
            return false;
        }
        self.frame = Some(Frame { ctx, key });
        tracing::info!(?key, "FSR FG: native AMD frame context created");
        true
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prepare(&mut self, device: &ID3D12Device, swap: &IDXGISwapChain3, copy: &HostList,
        captured: Option<Captured>, scene: &ID3D12Resource, scene_state: u32, composed: bool,
        display_size: [u32; 2], format: DXGI_FORMAT) {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1);
        if self.faulted || FAILED_CALLBACKS.load(Ordering::Relaxed) != 0 {
            self.faulted = true;
            self.disable(swap, id);
            return;
        }
        let Some(input) = captured else {
            static NO_INPUT: std::sync::Once = std::sync::Once::new();
            NO_INPUT.call_once(|| tracing::info!("FSR FG: waiting for a submitted FidelityFX upscale"));
            self.disable(swap,id); return;
        };
        let m = input.meta;
        // The title's shipping FidelityFX path only accepts its Series S
        // resolutions. Start with SDR 1080p; no image patch or fake upscaling.
        if display_size != [1920,1080] || m.output != display_size || m.reset ||
            m.max_render.contains(&0) || m.render.contains(&0) ||
            m.render.iter().zip(m.max_render).any(|(&a,b)| a > b) ||
            !matches!(format.0, 28 | 87) ||
            !m.jitter.iter().chain(m.motion_scale.iter()).all(|v| v.is_finite()) ||
            !m.near.is_finite() || !m.far.is_finite() || !m.fov.is_finite() ||
            m.near.min(m.far) <= 0.0 || m.near == m.far || !(0.0..std::f32::consts::PI).contains(&m.fov) {
            static INVALID: std::sync::Once = std::sync::Once::new();
            if !m.reset {
                INVALID.call_once(|| tracing::warn!(?m, ?display_size, format=format.0,
                    "FSR FG: unsupported display/camera/render configuration"));
            }
            self.disable(swap,id);
            return;
        }
        let (Some(depth), Some(motion)) = (resource(&input.depth.texture,4), resource(&input.motion.texture,4)) else {
            static FORMAT: std::sync::Once = std::sync::Once::new();
            FORMAT.call_once(|| {
                let (d,m) = unsafe { (input.depth.texture.GetDesc(),input.motion.texture.GetDesc()) };
                tracing::warn!(depth_format=d.Format.0, motion_format=m.Format.0, "FSR FG: unsupported native input texture");
            });
            self.disable(swap,id); return;
        };
        let motion_size = if m.flags & 2 != 0 { display_size } else { m.render };
        if depth.desc[2] < m.render[0] || depth.desc[3] < m.render[1] ||
            motion.desc[2] < motion_size[0] || motion.desc[3] < motion_size[1] ||
            legal_state(&input.depth.texture,0x40) != 0x40 || legal_state(&input.motion.texture,0x40) != 0x40 {
            static STATE: std::sync::Once = std::sync::Once::new();
            STATE.call_once(|| tracing::warn!(depth=?depth.desc, motion=?motion.desc, ?motion_size,
                "FSR FG: native depth/motion dimensions or shader-read state invalid"));
            self.disable(swap,id); return;
        }
        let key = Key { display: display_size, max_render: m.max_render, flags: m.flags & (2|4|8|16),
            format: surface(format.0).expect("checked SDR format") };
        if !self.ensure_frame(device,key) { self.disable(swap,id); return; }
        // On Series S the first plane excludes the separate premultiplied
        // interface. Use it as HUD-less colour only with a matching format,
        // dimensions and SDK-representable resource state.
        let hudless = if composed {
            ffx_state(scene_state).and_then(|state| resource(scene,state))
                .filter(|r| r.desc[1] == key.format && [r.desc[2],r.desc[3]] == display_size)
        } else { None };
        if self.hudless.len() >= 8 && !self.wait() {
            self.faulted = true; self.disable(swap,id); return;
        }
        if hudless.is_some() { self.hudless.push_back(scene.clone()); }
        if !self.configure(swap,true,id,hudless.unwrap_or_else(Resource::empty)) {
            // A failed configure may have changed the provider; explicitly disable it.
            self.enabled = true; self.faulted = true; self.disable(swap,id); return;
        }
        if !self.enabled { tracing::info!(id, hudless = hudless.is_some(), "FSR FG: enabled"); }
        self.enabled = true;
        let frame_ctx = self.frame.as_ref().expect("created above").handle();
        let Some(api) = api() else { return; };
        // Use tracked host states; the submission prelude transitions the
        // actual resources, and the list retains the Arcs until GPU completion.
        let before_d = input.depth.state.load(Ordering::Relaxed);
        let before_m = input.motion.state.load(Ordering::Relaxed);
        copy.settle(&input.depth,0x40);
        copy.settle(&input.motion,0x40);
        let mut camera = CameraInfo { header: Header::new(0x2000a), position: [0.0;3],
            up: [0.0,1.0,0.0], right: [1.0,0.0,0.0], forward: [0.0,0.0,1.0] };
        let mut prep = Prepare { header: Header::new(0x20004), frame_id: id, flags: 0, list: copy.list.as_raw(),
            render: m.render, jitter: m.jitter, motion_scale: m.motion_scale,
            frame_ms: if m.frame_ms.is_finite() && m.frame_ms > 0.0 { m.frame_ms.min(1000.0) } else { 16.667 },
            unused_reset: m.reset, near: m.near.min(m.far), far: m.near.max(m.far), fov: m.fov,
            meters: if m.meters.is_finite() && m.meters > 0.0 { m.meters } else { 1.0 }, depth, motion };
        prep.header.next = &mut camera.header;
        let code = unsafe { (api.dispatch)(frame_ctx,&prep.header) };
        copy.settle(&input.depth,before_d);
        copy.settle(&input.motion,before_m);
        if code != 0 {
            tracing::warn!(code,id,"FSR FG: prepare dispatch failed; disabling");
            self.faulted = true; self.disable(swap,id); return;
        }
        self.prepared += 1;
        if self.prepared == 1 || self.prepared % 120 == 0 {
            tracing::info!(id, prepared=self.prepared, generated_dispatches=CALLBACKS.load(Ordering::Relaxed),
                render=?m.render, display_size=?display_size, hudless=hudless.is_some(), "FSR FG: prepared frame");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ffx_surface_values_and_states() {
        assert_eq!(surface(28),Some(10));
        assert_eq!(surface(34),Some(18));
        assert_eq!(surface(40),Some(28));
        assert_eq!(surface(87),Some(14));
        assert_eq!(surface(0),None);
        assert_eq!(ffx_state(0xc0),Some(12));
        assert_eq!(ffx_state(0x10),None);
    }
}
