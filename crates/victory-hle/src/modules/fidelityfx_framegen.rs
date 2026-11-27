//! Observe the title's FidelityFX upscale calls without replacing its upscaler.
//! Xbox ABI layouts and import interposition adapted from readmet3xt-lab/foxhound
//! crates/victory-hle/src/modules/fidelityfx.rs (GPL-3.0), with its credit to
//! DexNom/VIctory. All original calls and return codes are preserved.
#![allow(non_snake_case)]
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use victory_gpu::fsr_framegen::{self, FrameMeta};
use victory_loader::{ImportResolver, Provenance, Resolution};
use victory_pe::ImportSymbol;

#[repr(C)]
#[derive(Clone, Copy)]
struct Header { kind: u64, next: *mut Header }
#[repr(C)]
#[derive(Clone, Copy)]
struct CreateUpscale { header: Header, flags: u32, max_render: [u32; 2], max_output: [u32; 2], message: usize }
#[repr(C)]
#[derive(Clone, Copy)]
struct Resource { resource: *mut c_void, description: [u32; 8], state: u32 }
#[repr(C)]
#[derive(Clone, Copy)]
struct DispatchUpscale {
    header: Header, list: *mut c_void, resources: [Resource; 7],
    jitter: [f32; 2], motion_scale: [f32; 2], render_size: [u32; 2], upscale_size: [u32; 2],
    enable_sharpening: u8, sharpness: f32, frame_time_delta: f32, pre_exposure: f32,
    reset: u8, camera_near: f32, camera_far: f32, camera_fov_angle_vertical: f32,
    view_space_to_meters_factor: f32, flags: u32,
}
const _: () = assert!(size_of::<CreateUpscale>() == 0x30);
const _: () = assert!(size_of::<Resource>() == 0x30);
const _: () = assert!(size_of::<DispatchUpscale>() == 0x1b0);
type Create = unsafe extern "C" fn(*mut *mut c_void, *mut Header, *const c_void) -> u32;
type Destroy = unsafe extern "C" fn(*mut *mut c_void, *const c_void) -> u32;
type Dispatch = unsafe extern "C" fn(*mut *mut c_void, *mut Header) -> u32;
static ORIGINALS: [AtomicUsize; 3] = [const { AtomicUsize::new(0) }; 3];
#[derive(Clone)]
struct Created { flags: u32, render: [u32; 2], output: [u32; 2], active: Arc<AtomicBool> }
static CONTEXTS: Mutex<Option<HashMap<usize, Created>>> = Mutex::new(None);
fn contexts<T>(f: impl FnOnce(&mut HashMap<usize, Created>) -> T) -> T {
    f(CONTEXTS.lock().unwrap_or_else(|e| e.into_inner()).get_or_insert_with(HashMap::new))
}

pub struct Interpose<'a> { pub inner: &'a dyn ImportResolver }
impl ImportResolver for Interpose<'_> {
    fn resolve(&self, dll: &str, symbol: &ImportSymbol) -> Resolution {
        let found = self.inner.resolve(dll, symbol);
        if !fsr_framegen::requested() || !dll.eq_ignore_ascii_case("amd_fidelityfx_loader_xbox.dll") { return found; }
        let (Resolution::Found { address, .. }, ImportSymbol::Name { name, .. }) = (&found, symbol) else { return found; };
        let (i, ours) = match name.as_str() {
            "ffxCreateContext" => (0, create as *const () as usize),
            "ffxDestroyContext" => (1, destroy as *const () as usize),
            "ffxDispatch" => (2, dispatch as *const () as usize),
            _ => return found,
        };
        if *address == ours { return found; }
        let old = ORIGINALS[i].compare_exchange(0, *address, Ordering::AcqRel, Ordering::Acquire);
        if old.is_err_and(|v| v != *address) { return found; }
        tracing::info!(name, "FSR FG: FidelityFX import intercepted");
        Resolution::Found { address: ours, provenance: Provenance::Hle }
    }
}

extern "C" fn create(context: *mut *mut c_void, desc: *mut Header, memory: *const c_void) -> u32 {
    let address = ORIGINALS[0].load(Ordering::Acquire);
    if address == 0 { return 6; }
    let original: Create = unsafe { std::mem::transmute(address) };
    let asked = if !desc.is_null() && unsafe { (*desc).kind } == 0x10000 {
        Some(unsafe { desc.cast::<CreateUpscale>().read() })
    } else { None };
    let code = unsafe { original(context, desc, memory) };
    if code == 0 && !context.is_null() {
        if let Some(asked) = asked {
            let handle = unsafe { *context } as usize;
            if handle != 0 {
                let previous = contexts(|c| c.insert(handle, Created { flags: asked.flags, render: asked.max_render, output: asked.max_output, active: Arc::new(AtomicBool::new(true)) }));
                if let Some(old) = previous { old.active.store(false, Ordering::Release); }
            }
        }
    }
    code
}

extern "C" fn destroy(context: *mut *mut c_void, memory: *const c_void) -> u32 {
    let address = ORIGINALS[1].load(Ordering::Acquire);
    if address == 0 { return 6; }
    let original: Destroy = unsafe { std::mem::transmute(address) };
    if !context.is_null() {
        let handle = unsafe { *context } as usize;
        if let Some(created) = contexts(|c| c.remove(&handle)) { created.active.store(false, Ordering::Release); }
    }
    unsafe { original(context, memory) }
}

extern "C" fn dispatch(context: *mut *mut c_void, desc: *mut Header) -> u32 {
    let address = ORIGINALS[2].load(Ordering::Acquire);
    if address == 0 { return 6; }
    let original: Dispatch = unsafe { std::mem::transmute(address) };
    let capture = if !context.is_null() && !desc.is_null() && unsafe { (*desc).kind } == 0x10001 {
        let handle = unsafe { *context } as usize;
        contexts(|c| c.get(&handle).cloned()).map(|created| (created, unsafe { desc.cast::<DispatchUpscale>().read() }))
    } else { None };
    let code = unsafe { original(context, desc) };
    if code == 0 {
        if let Some((created, d)) = capture {
            let meta = FrameMeta { flags: created.flags, render: d.render_size, output: if d.upscale_size == [0, 0] { created.output } else { d.upscale_size },
                max_render: created.render, jitter: d.jitter, motion_scale: d.motion_scale, frame_ms: d.frame_time_delta,
                near: d.camera_near, far: d.camera_far, fov: d.camera_fov_angle_vertical, meters: d.view_space_to_meters_factor, reset: d.reset != 0 };
            fsr_framegen::capture(d.list.cast_const().cast(), d.resources[1].resource.cast_const().cast(), d.resources[2].resource.cast_const().cast(), meta, created.active);
        }
    }
    code
}
