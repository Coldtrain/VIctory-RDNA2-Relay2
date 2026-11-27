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
static ORIGINALS: [AtomicUsize; 4] = [const { AtomicUsize::new(0) }; 4];
#[derive(Clone)]
struct Created { flags: u32, render: [u32; 2], output: [u32; 2], active: Arc<AtomicBool>, native: Option<Arc<fsr_framegen::Upscaler>>, clamped: bool }
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
            "ffxQuery" if fsr_framegen::upscaling() => (3, query as *const () as usize),
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
    // Foxhound's shipping context workaround, from DexNom: the original
    // Xbox FSR4 tables are fixed at 1280x720 -> 1920x1080. Keep that context
    // for title bookkeeping only; the native Windows SDK uses asked sizes.
    let replace = fsr_framegen::upscaling() && asked.is_some();
    if replace && !fsr_framegen::sdk_ready() {
        tracing::error!("FSR SR: SDK missing; cannot replace Xbox upscaler");
        return 6;
    }
    let clamped = replace && asked.is_some_and(|a| a.max_render != [1280,720] || a.max_output != [1920,1080]);
    let code = if clamped {
        let mut fitted = asked.expect("checked");
        fitted.max_render = [1280,720];
        fitted.max_output = [1920,1080];
        let code = unsafe { original(context, &mut fitted.header, memory) };
        tracing::info!(code, asked_render=?asked.map(|a|a.max_render), asked_output=?asked.map(|a|a.max_output),
            "FSR SR: Xbox context created at shipping size for bookkeeping");
        code
    } else { unsafe { original(context, desc, memory) } };
    if code != 0 && asked.is_some() {
        tracing::error!(code, max_render=?asked.map(|a|a.max_render), max_output=?asked.map(|a|a.max_output),
            "FidelityFX: Xbox context creation failed");
    }
    if code == 0 && !context.is_null() {
        if let Some(asked) = asked {
            let handle = unsafe { *context } as usize;
            if handle != 0 {
                let native = if replace {
                    let Some(up) = fsr_framegen::make_upscaler(asked.flags, asked.max_render, asked.max_output) else {
                        tracing::error!("FSR SR: cannot create native replacement; refusing unsafe Xbox dispatch");
                        let destroy_addr = ORIGINALS[1].load(Ordering::Acquire);
                        if destroy_addr != 0 {
                            let destroy: Destroy = unsafe { std::mem::transmute(destroy_addr) };
                            unsafe { destroy(context,memory) };
                        }
                        return 6;
                    };
                    Some(up)
                } else { None };
                let previous = contexts(|c| c.insert(handle, Created { flags: asked.flags, render: asked.max_render, output: asked.max_output, active: Arc::new(AtomicBool::new(true)), native, clamped }));
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

extern "C" fn query(context: *mut *mut c_void, desc: *mut Header) -> u32 {
    if fsr_framegen::upscaling() {
        if let Some(code) = unsafe { fsr_framegen::upscale_query(desc.cast()) } {
            return code;
        }
    }
    let address = ORIGINALS[3].load(Ordering::Acquire);
    if address == 0 { return 6; }
    let original: Dispatch = unsafe { std::mem::transmute(address) };
    unsafe { original(context,desc) }
}

extern "C" fn dispatch(context: *mut *mut c_void, desc: *mut Header) -> u32 {
    let address = ORIGINALS[2].load(Ordering::Acquire);
    if address == 0 { return 6; }
    let original: Dispatch = unsafe { std::mem::transmute(address) };
    let capture = if !context.is_null() && !desc.is_null() && unsafe { (*desc).kind } == 0x10001 {
        let handle = unsafe { *context } as usize;
        contexts(|c| c.get(&handle).cloned()).map(|created| (created, unsafe { desc.cast::<DispatchUpscale>().read() }))
    } else { None };
    if let Some((created,d)) = &capture {
        if let Some(native) = &created.native {
            let meta = FrameMeta { flags:created.flags, render:d.render_size,
                output:if d.upscale_size == [0,0] {created.output} else {d.upscale_size},
                max_render:created.render, jitter:d.jitter, motion_scale:d.motion_scale,
                frame_ms:d.frame_time_delta, near:d.camera_near, far:d.camera_far,
                fov:d.camera_fov_angle_vertical, meters:d.view_space_to_meters_factor, reset:d.reset != 0 };
            let work = fsr_framegen::UpscaleWork { context:native.clone(), resources:d.resources.map(|r|r.resource as usize),
                meta, sharpening:d.enable_sharpening != 0, sharpness:d.sharpness,
                pre_exposure:d.pre_exposure, dispatch_flags:d.flags, active:created.active.clone() };
            if fsr_framegen::upscale(d.list.cast_const().cast(),work) { return 0; }
            // Never call the fixed-size Xbox provider at a mismatched size,
            // or mix its temporal history with that of the native provider.
            tracing::error!(clamped=created.clamped, ?meta, "FSR SR: dispatch could not be recorded; no Xbox fallback");
            return 6;
        }
    }
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
