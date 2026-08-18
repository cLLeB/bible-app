//! Which processor whisper runs on: this machine's CPU, or its graphics card.
//!
//! Until now there was no choice being made. The shipped whisper build carries ten
//! CPU backends (`ggml-cpu-alderlake.dll` and friends, one of which ggml picks at
//! runtime for the exact chip) and no GPU code whatsoever. So "the app uses the
//! CPU" was never a decision it took — a graphics card was not in the box to
//! choose. This module puts one in the box and decides between them.
//!
//! Two GPU backends, because between them they cover essentially every laptop a
//! church is likely to own:
//!
//!   * CUDA — NVIDIA only, and by a wide margin the fastest thing available. It is
//!     also the expensive one to ship: the CUDA runtime is several hundred
//!     megabytes.
//!   * Vulkan — NVIDIA, AMD and Intel alike, including the integrated graphics in
//!     most ordinary laptops. A couple of megabytes, riding on the display driver
//!     that is already installed.
//!
//! Preferring the GPU is the default, and it is *checked* rather than assumed:
//! `measure` times real transcription on every backend this machine can run, and the
//! winner is what gets used.
//!
//! That check was built expecting integrated graphics to be a close call. It is not.
//! Measured on a 15W i5-1334U with Intel Iris Xe: eleven seconds of real speech,
//! `small`, the decode settings a service actually uses, five runs of each
//! interleaved so drift cancels rather than accumulates (milliseconds of compute
//! per utterance, model loading excluded):
//!
//!                        min    median    max
//!     Vulkan (Iris Xe)  1075      1452   1755
//!     CPU (8 threads)   5667      8487   9398
//!
//! About five times faster, from the graphics chip that comes free with the
//! processor, and the transcript is identical character for character. The clearest
//! way to put it: the GPU's *worst* run still beat the CPU's *best* run threefold.
//!
//! Single readings are worth little here — an earlier pass on a busy machine read
//! 10201 ms for the CPU, which flattered the GPU. Hence the repeats.
//!
//! The check stays, because this will not be five times on every machine and could
//! still go the other way on some. But the expectation it was guarding against —
//! that integrated graphics might lose outright — was simply wrong.

use std::path::{Path, PathBuf};

/// Where whisper runs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Backend {
    Cpu,
    Vulkan,
    Cuda,
}

impl Backend {
    /// Fastest first. This is the order `Preference::Auto` falls back on when
    /// nothing has been measured yet, so it is a claim about typical hardware
    /// rather than about this machine.
    pub const RANKED: [Backend; 3] = [Backend::Cuda, Backend::Vulkan, Backend::Cpu];

    /// The subdirectory of `bin/` holding this backend's whisper build.
    pub fn key(self) -> &'static str {
        match self {
            Backend::Cpu => "cpu",
            Backend::Vulkan => "vulkan",
            Backend::Cuda => "cuda",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Backend::Cpu => "Processor (CPU)",
            Backend::Vulkan => "Graphics card (Vulkan)",
            Backend::Cuda => "NVIDIA graphics card (CUDA)",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "cpu" => Some(Backend::Cpu),
            "vulkan" => Some(Backend::Vulkan),
            "cuda" => Some(Backend::Cuda),
            _ => None,
        }
    }

    /// The driver library this backend cannot run without. A card can be fitted and
    /// its driver missing, so the presence of the library is the thing to test, not
    /// the presence of hardware.
    fn driver_dll(self) -> Option<&'static str> {
        match self {
            Backend::Cpu => None,
            // The Vulkan loader, installed by every current Windows display driver.
            Backend::Vulkan => Some("vulkan-1.dll"),
            // The NVIDIA driver's own CUDA library. Note this is deliberately not
            // the CUDA *toolkit* — no toolkit is needed to run, only the driver.
            Backend::Cuda => Some("nvcuda.dll"),
        }
    }
}

/// Can this machine's drivers run `b`? Asking the OS to load the library is the
/// only honest test available: registry entries and device names both lie about
/// hybrid laptops that have two graphics chips in them.
#[cfg(windows)]
fn driver_present(dll: &str) -> bool {
    use windows_sys::Win32::Foundation::FreeLibrary;
    use windows_sys::Win32::System::LibraryLoader::LoadLibraryA;
    let name = match std::ffi::CString::new(dll) {
        Ok(n) => n,
        Err(_) => return false,
    };
    // SAFETY: `name` is a valid NUL-terminated C string for the duration of the
    // call, and the returned handle is either null or ours to free.
    unsafe {
        let h = LoadLibraryA(name.as_ptr() as *const u8);
        if h.is_null() {
            return false;
        }
        FreeLibrary(h);
        true
    }
}

#[cfg(not(windows))]
fn driver_present(_dll: &str) -> bool {
    false
}

fn drivers_allow(b: Backend) -> bool {
    match b.driver_dll() {
        None => true,
        Some(dll) => driver_present(dll),
    }
}

/// A backend counts as installed when its whisper build is actually on disk. Two
/// layouts are accepted: `bin/<backend>/` as flavors are built now, and a bare
/// `bin/` holding a CPU build, which is how every installer shipped so far is laid
/// out and which must keep working.
pub fn dir_for(bin_root: &Path, b: Backend) -> Option<PathBuf> {
    let nested = bin_root.join(b.key());
    if nested.join(exe_name()).exists() {
        return Some(nested);
    }
    if b == Backend::Cpu && bin_root.join(exe_name()).exists() {
        return Some(bin_root.to_path_buf());
    }
    None
}

fn exe_name() -> &'static str {
    if cfg!(windows) {
        "whisper-cli.exe"
    } else {
        "whisper-cli"
    }
}

/// Backends that are both shipped in this build and runnable on this machine,
/// ranked fastest first.
pub fn available(bin_root: &Path) -> Vec<Backend> {
    Backend::RANKED
        .iter()
        .copied()
        .filter(|&b| dir_for(bin_root, b).is_some() && drivers_allow(b))
        .collect()
}

// ---- What the operator has asked for ---------------------------------------

pub const SETTING_PREFERENCE: &str = "accel_preference";
pub const SETTING_MEASURED: &str = "accel_measured";
pub const SETTING_THREADS: &str = "accel_threads";

/// Auto is the default and means "the fastest thing that works here". The forced
/// options exist because a graphics driver that crashes under load is a real thing,
/// and when it happens during a service the operator needs a way back to the CPU
/// that does not involve a rebuild.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preference {
    Auto,
    Force(Backend),
}

impl Preference {
    pub fn parse(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            "auto" | "" => Preference::Auto,
            other => Backend::parse(other).map(Preference::Force).unwrap_or(Preference::Auto),
        }
    }

    pub fn key(self) -> String {
        match self {
            Preference::Auto => "auto".into(),
            Preference::Force(b) => b.key().into(),
        }
    }
}

/// The backend to run on, given what is installed, what the drivers allow, what has
/// been measured here, and what the operator asked for.
///
/// Written as a plain function over its inputs so the rule can be tested without a
/// graphics card, a database or a whisper build present.
pub fn choose(
    available: &[Backend],
    measured_best: Option<Backend>,
    preference: Preference,
) -> Backend {
    // A forced choice is honoured whenever it can be. It cannot be when the machine
    // has no such driver or the build did not ship that backend, and silently
    // failing to transcribe would be a far worse answer than quietly using the CPU.
    if let Preference::Force(b) = preference {
        if available.contains(&b) {
            return b;
        }
    }
    // Measurement beats the ranking: it is about this machine rather than about
    // laptops in general, which is the whole reason for taking it.
    if let Some(b) = measured_best {
        if available.contains(&b) {
            return b;
        }
    }
    available.first().copied().unwrap_or(Backend::Cpu)
}

/// The backend in force. Worked out once at startup and again whenever the operator
/// changes the setting, rather than re-derived on every utterance — the same shape
/// the learned speech threshold and thread count already use.
static CHOSEN: std::sync::Mutex<Option<Backend>> = std::sync::Mutex::new(None);

pub fn chosen() -> Option<Backend> {
    CHOSEN.lock().ok().and_then(|g| *g)
}

/// Decide which backend to run on from what this build ships, what this machine's
/// drivers allow, what was measured here, and what the operator asked for; then
/// remember it. Also restores the measured thread count, since the two were
/// measured together and only make sense together.
pub fn refresh(db: &crate::db::Db, bin_root: &Path) -> Backend {
    let have = available(bin_root);
    let preference =
        db.get_setting(SETTING_PREFERENCE).map(|s| Preference::parse(&s)).unwrap_or(Preference::Auto);
    let measured = db.get_setting(SETTING_MEASURED).and_then(|s| Backend::parse(&s));
    let b = choose(&have, measured, preference);
    if let Ok(mut g) = CHOSEN.lock() {
        *g = Some(b);
    }
    if let Some(n) = db.get_setting(SETTING_THREADS).and_then(|s| s.trim().parse::<usize>().ok()) {
        crate::stt::set_tuned_threads(n);
    }
    b
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graphics_card_is_preferred_when_nothing_has_been_measured() {
        // The operator's ask: if the machine has a GPU, use it, without being told to.
        let have = vec![Backend::Cuda, Backend::Vulkan, Backend::Cpu];
        assert_eq!(choose(&have, None, Preference::Auto), Backend::Cuda);
        let intel = vec![Backend::Vulkan, Backend::Cpu];
        assert_eq!(choose(&intel, None, Preference::Auto), Backend::Vulkan);
    }

    #[test]
    fn measuring_this_machine_overrules_the_general_ranking() {
        // The case this exists for: a low-power integrated chip where Vulkan really
        // does lose to the CPU. Assuming the GPU wins would make the app slower on
        // exactly the machines that can least afford it.
        let have = vec![Backend::Vulkan, Backend::Cpu];
        assert_eq!(choose(&have, Some(Backend::Cpu), Preference::Auto), Backend::Cpu);
    }

    #[test]
    fn the_operator_can_overrule_both() {
        let have = vec![Backend::Cuda, Backend::Vulkan, Backend::Cpu];
        assert_eq!(
            choose(&have, Some(Backend::Cuda), Preference::Force(Backend::Cpu)),
            Backend::Cpu
        );
    }

    #[test]
    fn asking_for_a_backend_this_machine_does_not_have_still_transcribes() {
        // A settings file carried to another machine, or a driver uninstalled. Not
        // transcribing at all is the one unacceptable outcome.
        let cpu_only = vec![Backend::Cpu];
        assert_eq!(choose(&cpu_only, None, Preference::Force(Backend::Cuda)), Backend::Cpu);
        assert_eq!(choose(&cpu_only, Some(Backend::Vulkan), Preference::Auto), Backend::Cpu);
        assert_eq!(choose(&[], None, Preference::Auto), Backend::Cpu);
    }

    #[test]
    fn preference_round_trips_and_bad_values_fall_back_to_auto() {
        for p in [Preference::Auto, Preference::Force(Backend::Cuda), Preference::Force(Backend::Cpu)] {
            assert_eq!(Preference::parse(&p.key()), p);
        }
        assert_eq!(Preference::parse("nonsense"), Preference::Auto);
        assert_eq!(Preference::parse(""), Preference::Auto);
    }

    #[test]
    fn a_legacy_flat_bin_folder_is_still_a_cpu_build() {
        // Every installer shipped so far lays bin/ out flat. They must keep working.
        let tmp = std::env::temp_dir().join(format!("accel_flat_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&tmp);
        let _ = std::fs::write(tmp.join(exe_name()), b"");
        assert_eq!(dir_for(&tmp, Backend::Cpu), Some(tmp.clone()));
        assert_eq!(dir_for(&tmp, Backend::Vulkan), None);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
