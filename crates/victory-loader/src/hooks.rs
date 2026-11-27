//! Opt-in compatibility hooks for known title builds.
//!
//! Render-scale subset ported from trustno1-coder/VIctory-NVIDIA (hooks.rs blob 0e67bb21192437d306af8b06397f9a8181a2c07b).
//! Upstream attribution: copied from vitutu1/victory-perf crates/victory-loader/src/hooks.rs, which ported it from
//! ZigZagBoi1/ScarlettPort (`runtime/victory`, GPL-3.0-or-later).
//! Only per-build offsets and short byte signatures live here (asset policy:
//! nothing of the title itself). Every hook checks the build's hash and the
//! exact bytes it replaces, and does nothing otherwise.

use windows_sys::Win32::System::Diagnostics::Debug::FlushInstructionCache;
use windows_sys::Win32::System::Memory::{PAGE_EXECUTE_READWRITE, VirtualProtect};
use windows_sys::Win32::System::Threading::GetCurrentProcess;

/// First 16 hex digits of the BLAKE3 of the Series X 1.0.2.1 executable.
const BUILD_1_0_2_1: &str = "b99f876b74d4a77f";

/// One code patch: `rva` must hold exactly `expect` before `new` is written.
struct Patch<'a> {
    what: &'static str,
    rva: usize,
    expect: &'a [u8],
    new: &'a [u8],
}

/// Environment variable that pins the title's internal render scale
/// (`0.2..=1.0` of the output size). Testing lever, off by default.
pub(crate) const RENDER_SCALE_ENV: &str = "VICTORY_RENDER_SCALE";
/// Environment variable that makes the title use its own temporal upscaler
/// on every console profile (Series S otherwise picks FSR).
pub(crate) const OWN_UPSCALER_ENV: &str = "VICTORY_OWN_UPSCALER";

/// A patch whose bytes are computed at load time.
struct OwnedPatch {
    what: &'static str,
    rva: usize,
    expect: Vec<u8>,
    new: Vec<u8>,
}

/// The upscaler choice: `mov eax, 6` (own upscaler) then `cmovne eax, ecx`
/// (FSR when the profile asks for it). NOPing the cmovne keeps 6.
const OWN_UPSCALER: Patch<'static> = Patch {
    what: "upscaler: always the title's own",
    rva: 0x7e9426,
    expect: &[0x0F, 0x45, 0xC1],
    new: &[0x0F, 0x1F, 0x00],
};

/// Read-only float constants of this build usable as the FSR scale:
/// value, RVA.
const FSR_SCALES: [(f32, usize); 4] = [
    (0.5, 0x5fc6edc),
    (0.4, 0x5fc6f90),
    (1.0 / 3.0, 0x615a240),
    (0.75, 0x6159b04),
];

/// The render-scale patches for `scale`, with the image at `base`:
/// - the own upscaler's minimum scale (an immediate, 2/3 by default) set to
///   `scale` and its maximum made equal to the minimum (`vmaxss` → `vmovss`),
///   so the scale is fixed;
/// - FSR's scale (a load of 2/3 on Series S) pointed at a constant equal to
///   `scale`, when this build has one, and its maximum size made the scaled
///   size (vtable slot 11 → slot 10's function).
fn render_scale_patches(base: usize, scale: f32, fsr: bool) -> Vec<OwnedPatch> {
    let mut patches = vec![
        OwnedPatch {
            what: "render scale: own upscaler minimum",
            rva: 0x28f1e7e,
            expect: vec![0xAB, 0xAA, 0x2A, 0x3F],
            new: scale.to_le_bytes().to_vec(),
        },
        OwnedPatch {
            what: "render scale: own upscaler maximum = minimum",
            rva: 0x28f1e87,
            expect: vec![0xC5, 0xFA, 0x5F, 0x86, 0x90, 0, 0, 0],
            new: vec![0xC5, 0xFA, 0x10, 0x86, 0x90, 0, 0, 0],
        },
    ];
    if !fsr {
        return patches;
    }
    match FSR_SCALES.iter().find(|(v, _)| (v - scale).abs() < 1e-3) {
        Some(&(_, constant)) => {
            // vmovss xmm0, [rip + disp32]; the next instruction is at +8.
            const LOAD: usize = 0x28ef6b2;
            let disp = (constant as i64 - (LOAD as i64 + 8)) as i32;
            let mut new = vec![0xC5, 0xFA, 0x10, 0x05];
            new.extend_from_slice(&disp.to_le_bytes());
            patches.push(OwnedPatch {
                what: "render scale: FSR scale",
                rva: LOAD,
                expect: vec![0xC5, 0xFA, 0x10, 0x05, 0xA2, 0xE2, 0x86, 0x03],
                new,
            });
            patches.push(OwnedPatch {
                what: "render scale: FSR maximum = scaled size",
                rva: 0x6300ba8,
                expect: ((base + 0x28ef6f0) as u64).to_le_bytes().to_vec(),
                new: ((base + 0x28ef680) as u64).to_le_bytes().to_vec(),
            });
        }
        None => tracing::warn!(
            scale,
            "no FSR constant for this scale (0.5, 0.4, 1/3, 0.75); FSR keeps its own (use {OWN_UPSCALER_ENV}=1)"
        ),
    }
    patches
}

// Opt-in port of Foxhound's prepare_upscaler (from DexNom), GPL-3.0-or-later.
// Select renderer 7, set its quality mode and sizing branch, and match the
// temporal scene scale. Xbox FidelityFX acceptance of non-stock sizes is
// experimental: upstream replaces its actual upscale with DLSS.
const STOCK_SIZING_ENV: &str = "VICTORY_FSR_STOCK_SIZING";
const STOCK_SIZING_BUILD: &str = "b99f876b74d4a77f03af5f878223b27bd647b5c35a79d5decd9f716504239e93";
const STOCK_CHECKS: [(usize, usize, &str); 4] = [
    (0x7e9421, 8, "f0633abf37df17124108b1e37be64645bd4b2412b0524c2f89334f22bd948464"),
    (0x28ef69a, 14, "2ae2ac102d12ca003cb218413ef018e4dba68fc2003654a30c1743e13a820ffd"),
    (0x28ef6a8, 10, "2579c24601d173b086a8136edf54ffd82720e6bf186f9139274bb09a45ddadf5"),
    (0x28ef6b2, 8, "e218882c95d2e16163a409d55b3451519414eb87c4d6220785dc09852d4e14f4"),
];

fn stock_mode(scale: f32) -> Option<(u8, f32)> {
    [(0, 1.0), (1, 2.0/3.0), (2, 0.58), (3, 0.5), (4, 1.0/3.0)]
        .into_iter().find(|&(_, v)| (v - scale).abs() < 0.001)
}

// Native FSR overrides the mode-4 sizing queries as well as the title's
// two scale constants. Do not change the shipping provider's preset path.
fn native_low_mode(scale: f32) -> Option<(u8, f32)> {
    (0.2..0.5).contains(&scale).then_some((4, scale))
}

fn stock_writes(mode: u8, scale: f32) -> Vec<(usize, Vec<u8>)> {
    let mut writes = vec![
        (0x7e9421, vec![0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90]),
        (0x28ef69a, vec![0xb8, mode, 0, 0, 0, 0x41, 0x89, 0x86, 0x90, 0, 0, 0, 0xeb, 0x0a]),
        (0x28f1e7e, scale.to_le_bytes().to_vec()),
    ];
    if mode != 1 {
        // Same unreachable native load literal and RIP-relative load as
        // upstream; the shared 2/3 constant is not overwritten.
        writes.push((0x28ef6ac, scale.to_le_bytes().to_vec()));
        let disp = 0x28ef6ac_i32 - (0x28ef6b2_i32 + 8);
        writes.push((0x28ef6b6, disp.to_le_bytes().to_vec()));
    }
    writes
}

/// Check ALL original bytes before writing, acquire all page protections
/// before writing, and restore in reverse order (some share a page).
/// SAFETY: exact original title image mapped, title code not yet running.
unsafe fn stock_sizing(base: usize, hash: &blake3::Hash, scale: f32) -> usize {
    let native = std::env::var_os("VICTORY_FSR_UPSCALER").is_some_and(|v| v == "1");
    let Some((mode, scale)) = (if native { native_low_mode(scale).or_else(|| stock_mode(scale)) } else { stock_mode(scale) }) else {
        tracing::warn!(scale, native, "unsupported FidelityFX scale; native AMD supports custom 0.20–0.50");
        return 0;
    };
    if hash.to_hex().as_str() != STOCK_SIZING_BUILD { return 0; }
    for (rva, len, expected) in STOCK_CHECKS {
        let bytes = unsafe { std::slice::from_raw_parts((base + rva) as *const u8, len) };
        if blake3::hash(bytes).to_hex().as_str() != expected {
            tracing::warn!(rva, "stock sizing refused: unexpected instruction hash");
            return 0;
        }
    }
    for rva in [0x615d95c, 0x28f1e7e] {
        let bytes = unsafe { std::slice::from_raw_parts((base + rva) as *const u8, 4) };
        if bytes != (2.0f32/3.0).to_le_bytes() {
            tracing::warn!(rva, "stock sizing refused: unexpected temporal/FSR scale");
            return 0;
        }
    }
    let writes = stock_writes(mode, scale);
    let mut protected: Vec<(*mut std::ffi::c_void, usize, u32)> = Vec::with_capacity(writes.len());
    for (rva, bytes) in &writes {
        let at = (base + *rva) as *mut u8;
        let mut old = 0;
        if unsafe { VirtualProtect(at as _, bytes.len(), PAGE_EXECUTE_READWRITE, &mut old) } == 0 {
            for (at, len, old) in protected.into_iter().rev() {
                let mut ignored = 0;
                unsafe { VirtualProtect(at, len, old, &mut ignored); }
            }
            tracing::warn!("stock sizing refused: cannot protect all patch ranges");
            return 0;
        }
        protected.push((at.cast(), bytes.len(), old));
    }
    for (rva, bytes) in &writes {
        let at = (base + *rva) as *mut u8;
        unsafe {
            std::ptr::copy_nonoverlapping(bytes.as_ptr(), at, bytes.len());
            FlushInstructionCache(GetCurrentProcess(), at as _, bytes.len());
        }
    }
    for (at, len, old) in protected.into_iter().rev() {
        let mut ignored = 0;
        if unsafe { VirtualProtect(at, len, old, &mut ignored) } == 0 {
            tracing::warn!("stock sizing: failed to restore original page protection");
        }
    }
    tracing::info!(mode, scale, "stock sizing: renderer 7, quality mode and temporal scale synchronized");
    if native && scale < 0.5 {
        tracing::info!(mode, scale, "native AMD custom sizing: mode 4 with synchronized scale queries");
    } else if mode >= 2 && !native {
        tracing::warn!(mode, scale, "stock Xbox FidelityFX custom size is experimental; Foxhound uses DLSS at these sizes");
    }
    writes.len()
}

/// The scale `VICTORY_RENDER_SCALE` asks for, if valid.
fn parse_scale(text: &str) -> Option<f32> {
    let v: f32 = text.trim().parse().ok()?;
    (0.2..=1.0).contains(&v).then_some(v)
}

/// Applies the resolution levers asked for in the environment
/// ([`RENDER_SCALE_ENV`], [`OWN_UPSCALER_ENV`]). Returns how many applied.
///
/// # Safety
/// The mapped image must have original hash `hash`; no title code may be running.
pub(crate) unsafe fn resolution_levers(base: usize, hash: &blake3::Hash) -> usize {
    let scale = std::env::var(RENDER_SCALE_ENV).ok().filter(|v| !v.trim().is_empty() && v.trim() != "0");
    let own = std::env::var_os(OWN_UPSCALER_ENV).is_some_and(|v| v != "0");
    if scale.is_none() && !own {
        return 0;
    }
    // Every image the loader maps comes here (the title's DLLs too); only
    // the 1.0.2.1 executable has the patched places.
    if !hash.to_hex().starts_with(BUILD_1_0_2_1) {
        tracing::debug!("resolution levers: not the 1.0.2.1 executable; nothing patched");
        return 0;
    }
    if std::env::var_os(STOCK_SIZING_ENV).is_some_and(|v| v == "1") {
        if own {
            tracing::warn!("stock sizing requires VICTORY_OWN_UPSCALER=0");
            return 0;
        }
        if let Some(scale) = scale.as_deref().and_then(parse_scale) {
            return unsafe { stock_sizing(base, hash, scale) };
        }
        tracing::warn!("stock sizing requires an explicit supported render scale");
        return 0;
    }
    let mut applied = 0;
    if own {
        // SAFETY: forwarded from this function's contract.
        applied += usize::from(unsafe { apply(base, &OWN_UPSCALER) });
    }
    if let Some(text) = scale {
        match parse_scale(&text) {
            Some(scale) => {
                // With the own upscaler forced, FSR is left as it is.
                for p in render_scale_patches(base, scale, !own) {
                    let p = Patch {
                        what: p.what,
                        rva: p.rva,
                        expect: &p.expect,
                        new: &p.new,
                    };
                    // SAFETY: forwarded from this function's contract.
                    applied += usize::from(unsafe { apply(base, &p) });
                }
            }
            None => tracing::warn!(text, "{RENDER_SCALE_ENV} wants a number in 0.2..=1.0"),
        }
    }
    applied
}

/// # Safety
/// The patch must address a valid range in the mapped image of the checked build.
unsafe fn apply(base: usize, p: &Patch<'_>) -> bool {
    let at = (base + p.rva) as *mut u8;
    // SAFETY: the offset is inside the mapped .text of this exact build.
    let now = unsafe { std::slice::from_raw_parts(at, p.expect.len()) };
    if now != p.expect || p.new.len() != p.expect.len() {
        tracing::warn!(what = p.what, "hook skipped: the bytes are not the expected ones");
        return false;
    }
    let mut old = 0;
    // SAFETY: changing the protection of those bytes only, restored below.
    if unsafe { VirtualProtect(at as _, p.new.len(), PAGE_EXECUTE_READWRITE, &mut old) } == 0 {
        return false;
    }
    // SAFETY: writable now; the bytes are the ones checked above.
    unsafe {
        std::ptr::copy_nonoverlapping(p.new.as_ptr(), at, p.new.len());
        VirtualProtect(at as _, p.new.len(), old, &mut old);
        FlushInstructionCache(GetCurrentProcess(), at as _, p.new.len());
    }
    tracing::info!(what = p.what, "hook applied");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn synchronized_stock_sizing_matches_upstream() {
        assert_eq!(stock_mode(0.5), Some((3, 0.5)));
        assert_eq!(stock_mode(1.0), Some((0, 1.0)));
        assert_eq!(stock_mode(0.30), None);
        assert_eq!(native_low_mode(0.30), Some((4, 0.30)));
        assert_eq!(native_low_mode(0.20), Some((4, 0.20)));
        assert_eq!(native_low_mode(0.50), None);
        assert_eq!(native_low_mode(0.19), None);
        assert_eq!(native_low_mode(f32::NAN), None);
        let low = stock_writes(4, 0.30);
        assert_eq!(low[1].1[1], 4);
        assert_eq!(low[2].1, 0.30f32.to_le_bytes());
        assert_eq!(low[3].1, 0.30f32.to_le_bytes());
        let p = stock_writes(3, 0.5);
        assert_eq!(p[0].1, [0xb8, 7, 0, 0, 0, 0x90, 0x90, 0x90]);
        assert_eq!(p[1].1[1], 3);
        assert_eq!(p[1].1[12..], [0xeb, 0x0a]);
        assert_eq!(p[2].1, 0.5f32.to_le_bytes());
        assert_eq!(p[3].1, 0.5f32.to_le_bytes());
        assert_eq!(p[4].1, (-14i32).to_le_bytes());
        assert_eq!(stock_writes(1, 2.0/3.0).len(), 3);
    }

    #[test]
    fn scales_outside_the_floor_are_refused() {
        assert_eq!(parse_scale("0.5"), Some(0.5));
        assert_eq!(parse_scale(" 1 "), Some(1.0));
        assert_eq!(parse_scale("0.1"), None);
        assert_eq!(parse_scale("1.5"), None);
        assert_eq!(parse_scale("half"), None);
    }

    #[test]
    fn half_scale_patches_both_upscalers() {
        let base = 0x7ff0_0000_0000;
        let p = render_scale_patches(base, 0.5, true);
        assert_eq!(p.len(), 4);
        assert_eq!(p[0].new, 0.5f32.to_le_bytes());
        // The FSR load now reads 0.5 at 0x5fc6edc: disp from the next instruction.
        assert_eq!(p[2].new, [0xC5, 0xFA, 0x10, 0x05, 0x22, 0x78, 0x6D, 0x03]);
        assert_eq!(p[3].new, ((base + 0x28ef680) as u64).to_le_bytes());
        assert!(p.iter().all(|p| p.new.len() == p.expect.len()));
    }

    #[test]
    fn odd_scales_patch_only_the_own_upscaler() {
        assert_eq!(render_scale_patches(0, 0.6, true).len(), 2);
        assert_eq!(render_scale_patches(0, 0.5, false).len(), 2);
    }
}

