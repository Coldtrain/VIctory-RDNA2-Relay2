//! Experimental opt-in FSR 3.1 frame generation at the two-plane presenter.
use std::{ffi::c_void, os::windows::ffi::OsStrExt, sync::{Arc, Mutex, OnceLock}, time::Instant};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering::Relaxed};
use windows::core::Interface;
use windows::Win32::Graphics::{Direct3D12::*, Dxgi::Common::*};
use windows_sys::Win32::System::LibraryLoader::{LoadLibraryW, GetProcAddress};
use windows_sys::Win32::Foundation::FreeLibrary;
use crate::{host::Host, resource::HostTexture};

static DISABLED: AtomicBool = AtomicBool::new(false);
fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("VICTORY_FSR_FRAMEGEN").is_some_and(|s| s == "1"))
        && !DISABLED.load(Relaxed)
}
fn num<T: std::str::FromStr>(key: &str, default: T) -> T {
    std::env::var(key).ok().and_then(|s| s.parse().ok()).unwrap_or(default)
}
fn render_size() -> (u32,u32) {
    static SIZE: OnceLock<(u32,u32)> = OnceLock::new();
    *SIZE.get_or_init(|| std::env::var("VICTORY_FSR_RENDER_SIZE").ok().and_then(|s| {
        let (w,h) = s.split_once('x')?;
        Some((w.parse::<u32>().ok()?,h.parse::<u32>().ok()?))
    }).filter(|&(w,h)| w > 0 && h > 0).unwrap_or((576,324)))
}
struct Candidate { copy: Arc<HostTexture>, format: i32, hits: u32, writes: u32 }
static CANDIDATES: Mutex<Vec<Candidate>> = Mutex::new(Vec::new());

/// Called under the submission lock, not by thin recording threads.
pub(crate) fn note_submitted(copy: &Arc<HostTexture>, first: u32, last: u32) {
    if !enabled() { return; }
    let d = unsafe { copy.texture.GetDesc() };
    let (w,h) = render_size();
    if d.Width != u64::from(w) || d.Height != h ||
        d.Dimension != D3D12_RESOURCE_DIMENSION_TEXTURE2D || d.MipLevels != 1 ||
        d.DepthOrArraySize != 1 || d.SampleDesc.Count != 1 ||
        d.Flags.contains(D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE) { return; }
    let depth = matches!(d.Format.0, 19 | 44) &&
        d.Flags.contains(D3D12_RESOURCE_FLAG_ALLOW_DEPTH_STENCIL);
    let motion = matches!(d.Format.0, 16 | 34) &&
        (d.Flags.contains(D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET) ||
         d.Flags.contains(D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS));
    if !depth && !motion { return; }
    let writes = u32::from((first | last) & (if depth { 0x10 } else { 0xc }) != 0);
    let mut c = CANDIDATES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(x) = c.iter_mut().find(|x| Arc::ptr_eq(&x.copy,copy)) {
        x.hits = x.hits.saturating_add(1);
        x.writes = x.writes.saturating_add(writes);
    } else if c.len() < 64 {
        c.push(Candidate { copy: copy.clone(), format: d.Format.0, hits: 1, writes });
    }
}

#[repr(C)]
struct Create {
    device: *mut c_void,
    display_width: u32, display_height: u32, max_render_width: u32, max_render_height: u32,
    flags: u32,
}
#[repr(C)]
struct Frame {
    list: *mut c_void, scene: *mut c_void, depth: *mut c_void, motion: *mut c_void,
    output: *mut c_void, frame_id: u64, render_width: u32, render_height: u32, reset: u32,
    jitter_x: f32, jitter_y: f32, motion_scale_x: f32, motion_scale_y: f32, delta_ms: f32,
    near_z: f32, far_z: f32, vertical_fov: f32, meters_factor: f32,
    camera_position: [f32;3], camera_up: [f32;3], camera_right: [f32;3], camera_forward: [f32;3],
}
type CreateFn = unsafe extern "C" fn(*const u16,*const Create,*mut *mut c_void) -> i32;
type RecordFn = unsafe extern "C" fn(*mut c_void,*const Frame) -> i32;
type DestroyFn = unsafe extern "C" fn(*mut c_void) -> i32;
struct Backend {
    library: usize, ctx: usize, record: RecordFn, destroy: DestroyFn,
    output: ID3D12Resource, depth: ID3D12Resource, converter: crate::compose::Composer,
    size: (u32,u32), previous: u64, time: Instant, selected: (u64,u64),
}
// Context access is serialized, D3D12 objects are free-threaded.
unsafe impl Send for Backend {}
static BACKEND: Mutex<Option<Backend>> = Mutex::new(None);
impl Drop for Backend {
    fn drop(&mut self) {
        unsafe {
            if (self.destroy)(self.ctx as *mut c_void) == 0 {
                let _ = FreeLibrary(self.library as _);
            } // keep code resident if destruction failed
        }
    }
}
fn path(key: &str, fallback: Option<&str>) -> Result<Vec<u16>,String> {
    let raw = std::env::var_os(key).or_else(|| fallback.map(Into::into))
        .ok_or_else(|| format!("{key} is not set"))?;
    let p = std::fs::canonicalize(raw).map_err(|e| format!("{key}: {e}"))?;
    Ok(p.as_os_str().encode_wide().chain(std::iter::once(0)).collect())
}
fn target(host: &Host, size: (u32,u32), format: DXGI_FORMAT,
          flags: D3D12_RESOURCE_FLAGS, state: D3D12_RESOURCE_STATES)
          -> windows::core::Result<ID3D12Resource> {
    let mut resource = None;
    let desc = D3D12_RESOURCE_DESC {
        Dimension: D3D12_RESOURCE_DIMENSION_TEXTURE2D,
        Width: u64::from(size.0), Height: size.1, DepthOrArraySize: 1, MipLevels: 1,
        Format: format, SampleDesc: DXGI_SAMPLE_DESC { Count: 1, Quality: 0 },
        Flags: flags, ..Default::default()
    };
    unsafe {
        host.device.CreateCommittedResource(
            &D3D12_HEAP_PROPERTIES { Type: D3D12_HEAP_TYPE_DEFAULT, ..Default::default() },
            D3D12_HEAP_FLAG_NONE, &desc, state, None, &mut resource)?;
    }
    resource.ok_or_else(windows::core::Error::empty)
}
impl Backend {
    fn new(host: &Host, size: (u32,u32), rs: (u32,u32)) -> Result<Self,String> {
        if rs.0 > size.0 || rs.1 > size.1 { return Err("render exceeds scene dimensions".into()); }
        let bridge = path("VICTORY_FSR_BRIDGE", Some("target/fsr31-native/victory_fsr31_native.dll"))?;
        let sdk = path("VICTORY_FSR_DLL", None)?;
        let output = target(host,size,DXGI_FORMAT_R8G8B8A8_UNORM,
            D3D12_RESOURCE_FLAG_ALLOW_UNORDERED_ACCESS,D3D12_RESOURCE_STATE_UNORDERED_ACCESS).map_err(|e| e.to_string())?;
        let depth = target(host,rs,DXGI_FORMAT_R32_FLOAT,D3D12_RESOURCE_FLAG_ALLOW_RENDER_TARGET,
            D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE).map_err(|e| e.to_string())?;
        let converter = crate::compose::Composer::new_depth(host).map_err(|e| e.to_string())?;
        unsafe {
            let lib = LoadLibraryW(bridge.as_ptr());
            if lib.is_null() { return Err("could not load native FSR bridge".into()); }
            let (Some(create),Some(record),Some(destroy)) = (
                GetProcAddress(lib,c"vfg_create".as_ptr().cast()),
                GetProcAddress(lib,c"vfg_record".as_ptr().cast()),
                GetProcAddress(lib,c"vfg_destroy".as_ptr().cast()),
            ) else {
                let _ = FreeLibrary(lib);
                return Err("native FSR bridge exports missing".into());
            };
            let create: CreateFn = std::mem::transmute(create);
            let record: RecordFn = std::mem::transmute(record);
            let destroy: DestroyFn = std::mem::transmute(destroy);
            let mut ctx = std::ptr::null_mut();
            let d = Create {
                device: host.device.as_raw(), display_width: size.0, display_height: size.1,
                max_render_width: rs.0, max_render_height: rs.1,
                flags: num("VICTORY_FSR_FLAGS",8u32),
            };
            let rc = create(sdk.as_ptr(),&d,&mut ctx);
            if rc != 0 || ctx.is_null() {
                let _ = FreeLibrary(lib);
                return Err(format!("AMD FSR context creation returned {rc}"));
            }
            tracing::info!(?size, ?rs, flags=d.flags, "FSR 3.1 experimental live context created");
            Ok(Self { library: lib as usize,ctx:ctx as usize,record,destroy,output,depth,converter,
                size,previous:0,time:Instant::now(),selected:(0,0) })
        }
    }

    fn run(&mut self,host:&Host,scene:&ID3D12Resource,state:u32,
           depth:&Candidate,motion:&Candidate,frame:u64,rs:(u32,u32)) -> Result<Option<ID3D12Resource>,String> {
        let list = host.new_list().map_err(|e| e.to_string())?;
        let cmds = &list.list;
        let old_depth = D3D12_RESOURCE_STATES(crate::host::legal_state(&depth.copy.texture,depth.copy.state.load(Relaxed)) as i32);
        let old_motion = D3D12_RESOURCE_STATES(crate::host::legal_state(&motion.copy.texture,motion.copy.state.load(Relaxed)) as i32);
        let old_scene = D3D12_RESOURCE_STATES(crate::host::legal_state(scene,state) as i32);
        let read = D3D12_RESOURCE_STATE_NON_PIXEL_SHADER_RESOURCE;
        let pixel = D3D12_RESOURCE_STATE_PIXEL_SHADER_RESOURCE;
        let target = D3D12_RESOURCE_STATE_RENDER_TARGET;
        let barrier = |r:&ID3D12Resource, from:D3D12_RESOURCE_STATES, to:D3D12_RESOURCE_STATES| {
            if from != to { unsafe { cmds.ResourceBarrier(&[crate::host::transition(r,from,to)]) }; }
        };
        let depth_view = match depth.format {
            19 => DXGI_FORMAT_R32_FLOAT_X8X24_TYPELESS,
            44 => DXGI_FORMAT_R24_UNORM_X8_TYPELESS,
            _ => return Err("unsupported hardware depth format".into()),
        };
        barrier(&depth.copy.texture,old_depth,pixel);
        barrier(&self.depth,read,target);
        if let Err(e) = self.converter.record(host,0,cmds,
            [(&depth.copy.texture,depth_view),(&depth.copy.texture,depth_view)],&self.depth,rs) {
            let _ = unsafe { cmds.Close() };
            return Err(e.to_string());
        }
        barrier(&depth.copy.texture,pixel,old_depth);
        barrier(&self.depth,target,read);
        barrier(&motion.copy.texture,old_motion,read);
        barrier(scene,old_scene,read);
        let now = Instant::now();
        let delta_ms = (now.duration_since(self.time).as_secs_f32()*1000.0).clamp(1.0,250.0);
        let reset = self.previous == 0 || self.previous.checked_add(1) != Some(frame) || delta_ms > 150.0;
        // Provisional conventions, NOT camera data recovered from the game.
        let f = Frame {
            list: cmds.as_raw(), scene: scene.as_raw(), depth: self.depth.as_raw(),
            motion: motion.copy.texture.as_raw(), output:self.output.as_raw(),frame_id:frame,
            render_width:rs.0,render_height:rs.1,reset:u32::from(reset),
            jitter_x:num("VICTORY_FSR_JITTER_X",0.0),jitter_y:num("VICTORY_FSR_JITTER_Y",0.0),
            motion_scale_x:num("VICTORY_FSR_MV_X",rs.0 as f32),
            motion_scale_y:num("VICTORY_FSR_MV_Y",rs.1 as f32),
            delta_ms,near_z:num("VICTORY_FSR_NEAR",0.1),far_z:num("VICTORY_FSR_FAR",10000.0),
            vertical_fov:num("VICTORY_FSR_FOV",1.0471976),meters_factor:1.0,
            camera_position:[0.,0.,0.],camera_up:[0.,1.,0.],
            camera_right:[1.,0.,0.],camera_forward:[0.,0.,1.],
        };
        let rc = unsafe { (self.record)(self.ctx as *mut c_void,&f) };
        if rc != 0 {
            let _ = unsafe { cmds.Close() }; // discard partial AMD commands
            return Err(format!("AMD FSR dispatch returned {rc}"));
        }
        barrier(scene,read,old_scene);
        barrier(&motion.copy.texture,read,old_motion);
        list.close();
        // Synchronous first implementation; never drain the async lane from itself.
        host.execute_sync_internal(&list);
        unsafe { host.device.GetDeviceRemovedReason() }.map_err(|e| e.to_string())?;
        let selected = (depth.copy.serial,motion.copy.serial);
        if self.selected != selected {
            tracing::info!(frame, depth_serial=selected.0,motion_serial=selected.1,
                depth_format=depth.format,motion_format=motion.format,
                "FSR experimental candidate pair (unverified semantics)");
        }
        self.selected = selected;
        self.previous = frame;
        self.time = now;
        Ok((!reset).then(||self.output.clone()))
    }
}

pub(crate) fn generate(host:&Host,scene:&ID3D12Resource,state:u32,has_overlay:bool,frame:u64)
    -> Option<ID3D12Resource> {
    if !enabled() { return None; }
    let candidates = std::mem::take(&mut *CANDIDATES.lock().unwrap_or_else(|e| e.into_inner()));
    let desc = unsafe { scene.GetDesc() };
    if !has_overlay || desc.Format != DXGI_FORMAT_R8G8B8A8_UNORM ||
        desc.DepthOrArraySize != 1 || desc.MipLevels != 1 || desc.SampleDesc.Count != 1 ||
        desc.Flags.contains(D3D12_RESOURCE_FLAG_DENY_SHADER_RESOURCE) { return None; }
    let rs = render_size();
    let size = (desc.Width as u32,desc.Height);
    let df = num("VICTORY_FSR_DEPTH_FORMAT",19i32);
    let mf = num("VICTORY_FSR_MOTION_FORMAT",34i32);
    let choose = |format| candidates.iter().filter(|x| x.format == format)
        .max_by_key(|x| (x.writes,x.hits,x.copy.serial));
    let (Some(depth),Some(motion)) = (choose(df),choose(mf)) else {
        if frame % 300 == 0 { tracing::info!(frame, ?rs, df, mf, candidates=candidates.len(),
            "FSR waiting for matching submitted resources"); }
        return None;
    };
    if depth.copy.texture.as_raw() == scene.as_raw() || motion.copy.texture.as_raw() == scene.as_raw() {
        return None;
    }
    // Settle older GPU work before reusing SDK and conversion resources.
    crate::host::settle_for_present();
    let mut backend = BACKEND.lock().unwrap_or_else(|e| e.into_inner());
    if backend.as_ref().is_some_and(|b|b.size != size) { *backend = None; }
    let result = (|| {
        if backend.is_none() { *backend = Some(Backend::new(host,size,rs)?); }
        backend.as_mut().unwrap().run(host,scene,state,depth,motion,frame,rs)
    })();
    match result {
        Ok(image) => image,
        Err(e) => {
            DISABLED.store(true,Relaxed);
            tracing::warn!(error=%e,"FSR framegen disabled; continuing ordinary presentation");
            *backend = None;
            CANDIDATES.lock().unwrap_or_else(|e| e.into_inner()).clear();
            None
        }
    }
}
pub(crate) fn presented() {
    static GENERATED: AtomicU64 = AtomicU64::new(0);
    let n = GENERATED.fetch_add(1,Relaxed)+1;
    if n == 1 || n % 120 == 0 {
        tracing::info!(generated_presents=n,"FSR generated frame presented (experimental)");
    }
}
