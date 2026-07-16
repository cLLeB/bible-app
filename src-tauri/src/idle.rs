//! When the app may teach itself — and, more importantly, when it may not.
//!
//! Learning is hours of hard decoding. On the machine running the service, that is the
//! last thing anyone wants: the laptop at the back of the church is often the same one
//! projecting, and it is frequently a modest one. So the rule is that learning happens
//! only when the machine is plainly not needed, and stops the instant it is.
//!
//! The gate is a plain function of plainly-observable facts, so what the app will and
//! won't do to a church's laptop can be read here and tested, rather than inferred from
//! where a thread happens to be spawned.

use crate::events::ProjectionState;

/// Why learning is not going to happen right now — each phrased as the operator would
/// be told it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Blocked {
    /// A service is in progress. Nothing matters more than that.
    Listening,
    /// Something is on the screen for the congregation to read.
    Projecting,
    /// Heavy decoding on battery would empty it. Mains only.
    OnBattery,
    /// A pass is already under way.
    AlreadyLearning,
    /// No approved service to learn from — nothing has been recorded and reviewed.
    NothingApproved,
    /// Learned recently; leave the machine alone for a while.
    TooSoon,
}

impl Blocked {
    /// What the operator sees. Never blame them for it — these are all fine states.
    pub fn message(&self) -> &'static str {
        match self {
            Blocked::Listening => "Not while a service is being listened to.",
            Blocked::Projecting => "Not while something is on the screen.",
            Blocked::OnBattery => "Plug the laptop in first — learning is heavy work.",
            Blocked::AlreadyLearning => "Already learning.",
            Blocked::NothingApproved => {
                "Nothing to learn from yet. Record a service and approve it first."
            }
            Blocked::TooSoon => "Learned recently — it will look again later.",
        }
    }
}

/// Everything the gate needs to know about this moment.
#[derive(Clone, Debug)]
pub struct Now {
    pub listening: bool,
    pub projecting: bool,
    pub on_mains: bool,
    pub learning: bool,
    /// Approved services available for the speaker in question.
    pub approved: usize,
    /// Milliseconds since the last pass for this speaker; `None` if never.
    pub since_last_ms: Option<u64>,
}

/// Long enough that a church which has just been offered a proposal is not badgered
/// again the same afternoon, short enough that a week of services is never left
/// unlearned.
pub const COOLDOWN_MS: u64 = 12 * 60 * 60 * 1000;

/// May the app learn right now? `None` means yes. The order is deliberate: the reasons
/// that concern the service come before the ones that are merely housekeeping, so the
/// operator is told the most important one.
pub fn blocked(now: &Now) -> Option<Blocked> {
    if now.listening {
        return Some(Blocked::Listening);
    }
    if now.projecting {
        return Some(Blocked::Projecting);
    }
    if now.learning {
        return Some(Blocked::AlreadyLearning);
    }
    if now.approved == 0 {
        return Some(Blocked::NothingApproved);
    }
    if !now.on_mains {
        return Some(Blocked::OnBattery);
    }
    if now.since_last_ms.map(|ms| ms < COOLDOWN_MS).unwrap_or(false) {
        return Some(Blocked::TooSoon);
    }
    None
}

/// Is anything on the screen the congregation could be reading? A blanked, blacked-out
/// or logo screen is the projector idling between parts of the service — the machine is
/// not busy in any way that matters.
pub fn is_projecting(state: &ProjectionState) -> bool {
    !matches!(
        state,
        ProjectionState::Blank | ProjectionState::Blackout | ProjectionState::Logo
    )
}

/// Is the machine on mains power? Learning flattens a battery, so a laptop running on
/// its own is left alone. A desktop, or anything we cannot ask, counts as mains: the
/// question is only ever asked to protect a battery that exists.
#[cfg(windows)]
pub fn on_mains() -> bool {
    #[repr(C)]
    struct SystemPowerStatus {
        ac_line_status: u8,
        battery_flag: u8,
        battery_life_percent: u8,
        system_status_flag: u8,
        battery_life_time: u32,
        battery_full_life_time: u32,
    }
    unsafe extern "system" {
        fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
    }
    let mut status = SystemPowerStatus {
        ac_line_status: 255,
        battery_flag: 255,
        battery_life_percent: 255,
        system_status_flag: 0,
        battery_life_time: 0,
        battery_full_life_time: 0,
    };
    // 0 = on battery, 1 = on mains, 255 = unknown. Only a definite "on battery" stops us.
    if unsafe { GetSystemPowerStatus(&mut status) } == 0 {
        return true; // the call failed; do not invent a battery
    }
    // 128 = no system battery at all (a desktop).
    status.ac_line_status != 0 || status.battery_flag == 128
}

#[cfg(not(windows))]
pub fn on_mains() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn idle() -> Now {
        Now {
            listening: false,
            projecting: false,
            on_mains: true,
            learning: false,
            approved: 2,
            since_last_ms: None,
        }
    }

    #[test]
    fn an_idle_plugged_in_machine_with_approved_services_may_learn() {
        assert_eq!(blocked(&idle()), None);
        // Having learned long ago is no obstacle.
        assert_eq!(blocked(&Now { since_last_ms: Some(COOLDOWN_MS + 1), ..idle() }), None);
    }

    #[test]
    fn nothing_learns_while_the_church_is_using_the_machine() {
        assert_eq!(blocked(&Now { listening: true, ..idle() }), Some(Blocked::Listening));
        assert_eq!(blocked(&Now { projecting: true, ..idle() }), Some(Blocked::Projecting));
        // The service outranks every other reason it might be told.
        let mid_service = Now { listening: true, projecting: true, on_mains: false, ..idle() };
        assert_eq!(blocked(&mid_service), Some(Blocked::Listening));
    }

    #[test]
    fn a_laptop_on_its_own_battery_is_left_alone() {
        assert_eq!(blocked(&Now { on_mains: false, ..idle() }), Some(Blocked::OnBattery));
    }

    #[test]
    fn there_is_no_learning_without_an_approved_service_however_idle_the_machine() {
        assert_eq!(blocked(&Now { approved: 0, ..idle() }), Some(Blocked::NothingApproved));
    }

    #[test]
    fn a_church_is_not_badgered_twice_in_an_afternoon() {
        assert_eq!(blocked(&Now { since_last_ms: Some(60_000), ..idle() }), Some(Blocked::TooSoon));
        assert_eq!(blocked(&Now { since_last_ms: Some(COOLDOWN_MS - 1), ..idle() }), Some(Blocked::TooSoon));
        assert_eq!(blocked(&Now { since_last_ms: Some(COOLDOWN_MS), ..idle() }), None);
    }

    #[test]
    fn only_what_the_congregation_can_read_counts_as_projecting() {
        assert!(!is_projecting(&ProjectionState::Blank));
        assert!(!is_projecting(&ProjectionState::Blackout));
        assert!(!is_projecting(&ProjectionState::Logo));
        assert!(is_projecting(&ProjectionState::Verse {
            text: "For God so loved the world".into(),
            caption: "John 3:16".into(),
        }));
        assert!(is_projecting(&ProjectionState::Message { text: "Welcome".into() }));
    }
}
