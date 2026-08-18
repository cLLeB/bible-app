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
    Metal,
}

impl Backend {
    /// Every backend that exists, whatever platform this is. Use `RANKED` to choose;
    /// this is for iterating over settings and reporting.
    pub const ALL: [Backend; 4] = [Backend::Cuda, Backend::Metal, Backend::Vulkan, Backend::Cpu];

    /// Fastest first, for the platform this build targets. The order is a claim about
    /// typical hardware, not about this machine — `measure` overrules it.
    ///
    /// Windows and Linux share a list: CUDA where there is an NVIDIA card, Vulkan for
    /// everything else with a GPU (Intel, AMD, and NVIDIA without CUDA installed),
    /// processor last. macOS has no place for either — Metal is part of the OS, needs
    /// nothing installed, and covers every Mac the app can run on.
    #[cfg(target_os = "macos")]
    pub const RANKED: [Backend; 2] = [Backend::Metal, Backend::Cpu];
    #[cfg(not(target_os = "macos"))]
    pub const RANKED: [Backend; 3] = [Backend::Cuda, Backend::Vulkan, Backend::Cpu];

    /// The subdirectory of `bin/` holding this backend's whisper build.
    pub fn key(self) -> &'static str {
        match self {
            Backend::Cpu => "cpu",
            Backend::Vulkan => "vulkan",
            Backend::Cuda => "cuda",
            Backend::Metal => "metal",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Backend::Cpu => "Processor (CPU)",
            Backend::Vulkan => "Graphics card (Vulkan)",
            Backend::Cuda => "NVIDIA graphics card (CUDA)",
            Backend::Metal => "Apple graphics (Metal)",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "cpu" => Some(Backend::Cpu),
            "vulkan" => Some(Backend::Vulkan),
            "cuda" => Some(Backend::Cuda),
            "metal" => Some(Backend::Metal),
            _ => None,
        }
    }

    /// The driver library this backend cannot run without, named for this platform.
    ///
    /// A card can be fitted and its driver missing, or a build can ship a backend to a
    /// machine that has no hardware for it, so asking the OS to load the library is the
    /// test — not looking for a device name. `None` means nothing to check: the CPU
    /// always works, and Metal is part of macOS rather than a driver anyone installs.
    fn driver_library(self) -> Option<&'static str> {
        match self {
            Backend::Cpu => None,
            Backend::Metal => None,
            Backend::Vulkan => {
                #[cfg(target_os = "windows")]
                {
                    Some("vulkan-1.dll")
                }
                #[cfg(target_os = "linux")]
                {
                    Some("libvulkan.so.1")
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux")))]
                {
                    None
                }
            }
            Backend::Cuda => {
                #[cfg(target_os = "windows")]
                {
                    Some("nvcuda.dll")
                }
                #[cfg(target_os = "linux")]
                {
                    Some("libcuda.so.1")
                }
                #[cfg(not(any(target_os = "windows", target_os = "linux")))]
                {
                    None
                }
            }
        }
    }

    /// Backends that cannot exist on this platform at all, whatever is on disk. Metal
    /// is macOS only; CUDA and Vulkan have no place on a Mac, where Metal is both
    /// always present and the fastest thing available.
    fn possible_here(self) -> bool {
        match self {
            Backend::Cpu => true,
            Backend::Metal => cfg!(target_os = "macos"),
            Backend::Vulkan | Backend::Cuda => !cfg!(target_os = "macos"),
        }
    }
}

/// Can this machine's drivers run `b`? Asking the OS to load the library is the
/// only honest test available: registry entries and device names both lie about
/// hybrid laptops that have two graphics chips in them.
#[cfg(target_os = "windows")]
fn driver_present(lib: &str) -> bool {
    use windows_sys::Win32::Foundation::FreeLibrary;
    use windows_sys::Win32::System::LibraryLoader::LoadLibraryA;
    let Ok(name) = std::ffi::CString::new(lib) else { return false };
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

/// The same question on Linux, answered by looking for the shared object on the
/// loader's path. `dlopen` would be stricter, but it means linking libdl and running
/// a driver's constructors just to ask whether it exists; on the distributions this
/// app targets, a driver that is installed is a file in one of these directories.
#[cfg(target_os = "linux")]
fn driver_present(lib: &str) -> bool {
    const DIRS: [&str; 6] = [
        "/usr/lib/x86_64-linux-gnu",
        "/usr/lib64",
        "/usr/lib",
        "/lib/x86_64-linux-gnu",
        "/usr/local/lib",
        "/usr/lib/aarch64-linux-gnu",
    ];
    DIRS.iter().any(|d| std::path::Path::new(d).join(lib).exists())
}

#[cfg(not(any(target_os = "windows", target_os = "linux")))]
fn driver_present(_lib: &str) -> bool {
    false
}

fn drivers_allow(b: Backend) -> bool {
    if !b.possible_here() {
        return false;
    }
    match b.driver_library() {
        None => true,
        Some(lib) => driver_present(lib),
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

/// Backends that are shipped in this build, possible on this platform, and whose
/// drivers are actually present. Ranked fastest first.
///
/// "Present drivers" is necessary but not sufficient: a driver can load and the GPU
/// still fail to run whisper's shaders. That is what `verified` is for.
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
/// The fastest backend proven to actually transcribe on this machine.
pub const SETTING_VERIFIED: &str = "accel_verified";
/// The hardware fingerprint that verdict was reached under.
pub const SETTING_VERIFIED_FOR: &str = "accel_verified_for";

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

/// The backend to run on: what is installed, what the drivers allow, what has been
/// proven to work here, what measured fastest, and what the operator asked for.
///
/// A plain function over its inputs so the rule can be tested without a graphics
/// card, a database or a whisper build anywhere near it.
///
/// The order matters, and the reason `verified` sits above the ranking is the whole
/// point of this module: a driver that loads is not a GPU that works. Shipping a
/// Vulkan build to a laptop with an ancient driver, trusting the ranking, and
/// discovering during a service that nothing transcribes would be far worse than
/// having shipped no GPU support at all.
pub fn choose(
    available: &[Backend],
    verified: Option<Backend>,
    measured_best: Option<Backend>,
    preference: Preference,
) -> Backend {
    // The operator's explicit choice wins, when it can. It cannot when the machine has
    // no such driver or the build did not ship that backend, and silently failing to
    // transcribe is a far worse answer than quietly using the processor.
    if let Preference::Force(b) = preference {
        if available.contains(&b) {
            return b;
        }
    }
    // Measured beats proven-to-work, which beats the general ranking. A measurement
    // implies the backend ran, so it is proof as well as a time.
    for candidate in [measured_best, verified] {
        if let Some(b) = candidate {
            if available.contains(&b) {
                return b;
            }
        }
    }
    // Nothing proven yet. Take the processor if it is there: it always works, and the
    // background check will move us onto the graphics card within seconds of launch.
    // Being briefly slower is recoverable; being briefly broken is not.
    if available.contains(&Backend::Cpu) {
        return Backend::Cpu;
    }
    available.first().copied().unwrap_or(Backend::Cpu)
}

/// What the graphics situation looked like when a backend was last proven to work.
///
/// Stored beside the verdict so a driver update, a new machine, or an external GPU
/// being plugged in causes a re-check instead of trusting a stale answer. Built from
/// the driver libraries themselves — their size and modification time — because that
/// is what changes when a driver is updated, and it needs no new dependency.
pub fn hardware_fingerprint() -> String {
    let mut parts: Vec<String> = Vec::new();
    for b in Backend::ALL {
        let Some(lib) = b.driver_library() else { continue };
        let present = driver_present(lib);
        parts.push(format!("{}={}", b.key(), if present { "y" } else { "n" }));
    }
    parts.push(format!("v={}", env!("CARGO_PKG_VERSION")));
    parts.join(",")
}

/// The backend in force. Worked out once at startup and again whenever the operator
/// changes the setting, rather than re-derived on every utterance — the same shape
/// the learned speech threshold and thread count already use.
static CHOSEN: std::sync::Mutex<Option<Backend>> = std::sync::Mutex::new(None);

pub fn chosen() -> Option<Backend> {
    CHOSEN.lock().ok().and_then(|g| *g)
}

/// Settle which backend to use from everything known, and remember it.
///
/// Cheap and synchronous: it only reads settings. Proving a backend actually works
/// costs a transcription, so that happens in `verify_in_background`.
pub fn refresh(db: &crate::db::Db, bin_root: &Path) -> Backend {
    let have = available(bin_root);
    let preference =
        db.get_setting(SETTING_PREFERENCE).map(|s| Preference::parse(&s)).unwrap_or(Preference::Auto);
    let measured = db.get_setting(SETTING_MEASURED).and_then(|s| Backend::parse(&s));
    // A verdict reached on different hardware, or a different build, is not evidence
    // about this one.
    let verified = match db.get_setting(SETTING_VERIFIED_FOR) {
        Some(f) if f == hardware_fingerprint() => {
            db.get_setting(SETTING_VERIFIED).and_then(|s| Backend::parse(&s))
        }
        _ => None,
    };
    let b = choose(&have, verified, measured, preference);
    if let Ok(mut g) = CHOSEN.lock() {
        *g = Some(b);
    }
    if let Some(n) = db.get_setting(SETTING_THREADS).and_then(|s| s.trim().parse::<usize>().ok()) {
        crate::stt::set_tuned_threads(n);
    }
    b
}

/// Is the stored verdict still about this machine?
pub fn verdict_is_current(db: &crate::db::Db) -> bool {
    db.get_setting(SETTING_VERIFIED_FOR).map(|f| f == hardware_fingerprint()).unwrap_or(false)
}

/// Prove which backend works here, on a background thread, and switch to it.
///
/// This is what makes the graphics card automatic without making it a gamble. Each
/// candidate is asked to transcribe a short clip, fastest-ranked first; the first
/// that succeeds is remembered and used from then on. A backend whose driver loads
/// but whose GPU cannot run whisper's shaders fails here, quietly, at launch —
/// rather than in front of a congregation.
///
/// Runs once per machine. The verdict is kept until the hardware fingerprint changes,
/// which a driver update or a new graphics card will do.
pub fn verify_in_background(
    app: tauri::AppHandle,
    bin_root: PathBuf,
    model: PathBuf,
) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        use tauri::Manager;
        let candidates = available(&bin_root);
        // The processor is the floor and needs no proving; anything above it does.
        let worth_proving: Vec<Backend> =
            candidates.iter().copied().filter(|&b| b != Backend::Cpu).collect();

        let winner = worth_proving
            .into_iter()
            .find(|&b| match dir_for(&bin_root, b) {
                Some(dir) => {
                    let ok = crate::accel_probe::smoke_test(&dir, &model, b);
                    if !ok {
                        eprintln!(
                            "accel: {} is installed and its driver loads, but it could not                              transcribe; trying the next one",
                            b.label()
                        );
                    }
                    ok
                }
                None => false,
            })
            .unwrap_or(Backend::Cpu);

        let state = app.state::<crate::commands::AppState>();
        if let Ok(db) = state.db.lock() {
            let _ = db.set_setting(SETTING_VERIFIED, winner.key());
            let _ = db.set_setting(SETTING_VERIFIED_FOR, &hardware_fingerprint());
            refresh(&db, &bin_root);
        }
        let _ = tauri::Emitter::emit(
            &app,
            "accel-verified",
            serde_json::json!({ "backend": winner.key(), "label": winner.label() }),
        );
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Everything on this platform, as if all of it were installed and driverful.
    fn all_available() -> Vec<Backend> {
        Backend::RANKED.to_vec()
    }

    #[test]
    fn a_proven_graphics_card_is_used_without_being_asked_for() {
        // The operator's ask: if the machine has a GPU that works, use it, silently.
        let have = all_available();
        let gpu = have[0];
        assert_eq!(choose(&have, Some(gpu), None, Preference::Auto), gpu);
    }

    #[test]
    fn nothing_proven_yet_means_the_processor_not_a_hopeful_guess() {
        // The safety property. A driver that loads is not a GPU that works, so until
        // something has actually transcribed we take the one that always can. The
        // background check moves us up within seconds of launch. Briefly slower is
        // recoverable; briefly broken, in front of a congregation, is not.
        let have = all_available();
        assert_eq!(choose(&have, None, None, Preference::Auto), Backend::Cpu);
    }

    #[test]
    fn measuring_this_machine_overrules_proof_and_ranking_alike() {
        // Proof says "it runs"; measurement says "it runs fastest". The second is
        // strictly more information, and it is about this machine rather than laptops
        // in general.
        let have = all_available();
        let gpu = have[0];
        assert_eq!(choose(&have, Some(gpu), Some(Backend::Cpu), Preference::Auto), Backend::Cpu);
    }

    #[test]
    fn the_operator_can_overrule_everything() {
        // Exists because a display driver that misbehaves under load is a real thing,
        // and mid-service is no time to need a reinstall.
        let have = all_available();
        let gpu = have[0];
        assert_eq!(choose(&have, Some(gpu), Some(gpu), Preference::Force(Backend::Cpu)), Backend::Cpu);
    }

    #[test]
    fn asking_for_something_this_machine_lacks_still_transcribes() {
        // A settings row carried to another machine, or a driver uninstalled. Not
        // transcribing at all is the one unacceptable outcome.
        let cpu_only = vec![Backend::Cpu];
        assert_eq!(choose(&cpu_only, None, None, Preference::Force(Backend::Cuda)), Backend::Cpu);
        assert_eq!(choose(&cpu_only, Some(Backend::Vulkan), None, Preference::Auto), Backend::Cpu);
        assert_eq!(choose(&[], None, None, Preference::Auto), Backend::Cpu);
    }

    #[test]
    fn a_verdict_from_other_hardware_is_not_evidence_about_this_one() {
        // Not the rule itself but the thing it depends on: the fingerprint has to
        // actually move when the graphics situation does, or a stale verdict is
        // trusted forever.
        let f = hardware_fingerprint();
        assert!(f.contains("v="), "the build version belongs in the fingerprint: {f}");
        assert_eq!(f, hardware_fingerprint(), "must be stable between calls");
    }

    #[test]
    fn preference_round_trips_and_nonsense_falls_back_to_auto() {
        for p in [Preference::Auto, Preference::Force(Backend::Cuda), Preference::Force(Backend::Cpu)] {
            assert_eq!(Preference::parse(&p.key()), p);
        }
        assert_eq!(Preference::parse("nonsense"), Preference::Auto);
        assert_eq!(Preference::parse(""), Preference::Auto);
    }

    #[test]
    fn backends_impossible_on_this_platform_are_never_offered() {
        // Metal only exists on a Mac; CUDA and Vulkan have no place on one, where
        // Metal is both always present and the fastest thing going.
        assert_eq!(Backend::Metal.possible_here(), cfg!(target_os = "macos"));
        assert_eq!(Backend::Cuda.possible_here(), !cfg!(target_os = "macos"));
        assert!(Backend::Cpu.possible_here(), "the processor is always possible");
        assert!(!drivers_allow(if cfg!(target_os = "macos") { Backend::Cuda } else { Backend::Metal }));
        // Whatever the platform, the ranking ends at the processor.
        assert_eq!(Backend::RANKED.last().copied(), Some(Backend::Cpu));
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
