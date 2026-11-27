//! AMD frame generation input capture. Adapted from the FidelityFX dispatch
//! capture and submission ordering in readmet3xt-lab/foxhound (GPL-3.0).
//! Captures only; the host AMD SDK backend is separate.
use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use crate::object::Object;
use crate::resource::HostTexture;

#[derive(Clone, Copy, Debug)]
pub struct FrameMeta {
    pub flags: u32,
    pub render: [u32; 2],
    pub output: [u32; 2],
    pub max_render: [u32; 2],
    pub jitter: [f32; 2],
    pub motion_scale: [f32; 2],
    pub frame_ms: f32,
    pub near: f32,
    pub far: f32,
    pub fov: f32,
    pub meters: f32,
    pub reset: bool,
}

pub(crate) struct Captured {
    pub(crate) meta: FrameMeta,
    pub(crate) depth: Arc<HostTexture>,
    pub(crate) motion: Arc<HostTexture>,
    pub(crate) active: Arc<AtomicBool>,
}

pub fn requested() -> bool {
    static ASKED: OnceLock<bool> = OnceLock::new();
    *ASKED.get_or_init(|| ["VICTORY_FSR_FRAMEGEN", "VICTORY_FSR_CAPTURE"].iter()
        .any(|k| std::env::var_os(k).is_some_and(|v| v == "1")))
}

pub fn capture(list: *const Object, depth: *const Object, motion: *const Object, meta: FrameMeta, active: Arc<AtomicBool>) {
    if !requested() || list.is_null() || depth.is_null() || motion.is_null() || meta.render.contains(&0) || meta.output.contains(&0) {
        return;
    }
    if !crate::resource::is_resource(depth) || !crate::resource::is_resource(motion) {
        return;
    }
    let (Some(depth), Some(motion)) = (crate::resource::host_copy(depth), crate::resource::host_copy(motion)) else { return; };
    crate::list::record_fsr(list, Captured { meta, depth, motion, active });
}

static LATEST: Mutex<Option<Captured>> = Mutex::new(None);
static COUNT: AtomicU64 = AtomicU64::new(0);

pub(crate) fn submitted(input: Captured) {
    if !input.active.load(Ordering::Acquire) { return; }
    let n = COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    if n == 1 || n % 120 == 0 {
        // SAFETY: both textures are live, owned by the capture.
        let (d, m) = unsafe { (input.depth.texture.GetDesc(), input.motion.texture.GetDesc()) };
        tracing::info!(n, meta = ?input.meta, depth_format = d.Format.0, depth_size = ?(d.Width, d.Height),
            motion_format = m.Format.0, motion_size = ?(m.Width, m.Height), "FSR FG: submitted FidelityFX inputs");
    }
    *LATEST.lock().unwrap_or_else(|e| e.into_inner()) = Some(input);
}

pub(crate) fn take_latest() -> Option<Captured> {
    LATEST.lock().unwrap_or_else(|e| e.into_inner()).take()
        .filter(|input| input.active.load(Ordering::Acquire))
}

mod native;
pub(crate) use native::Backend;
