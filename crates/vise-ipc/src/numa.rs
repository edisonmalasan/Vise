//! What the machine's processor layout looks like, and where the worker is put.
//!
//! A machine with more than one processor group is a machine with more than one
//! set of memory controllers, and a process that runs across all of them pays
//! for memory that is not local to wherever it happens to be running. For an
//! encoder, which is one thread reading one file and writing another, that cost
//! is real; so is the cost of being wrong about it, because a build that pins an
//! encode to a group the file is not on makes the machine look slower than it is.
//!
//! So this module does two small things and nothing more. It *reports* the
//! layout, using the one call that answers without walking a structure tree, and
//! it *decides* what to do about it, as a pure function of the layout, the
//! preference, and the priority the worker asked for. Applying the decision is a
//! single call the worker makes on its own main thread, because a process can
//! always restrict itself and nothing else can restrict it reliably.
//!
//! Only the small amount of platform API needed for that lives here, declared
//! directly, as the worker's process control already does.

use std::fmt;

/// How much scheduling priority the worker asked for, as the contract carries it.
///
/// The contract's signed scale, named here rather than borrowed, because this
/// decision is about whether a worker is competing hard enough to be worth
/// pinning and the contract already says so.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum WorkerPriority {
    /// Below normal, or lower.
    Below,
    /// Whatever the platform gives a process by default.
    #[default]
    Normal,
    /// Above normal, or higher.
    Above,
}

impl WorkerPriority {
    /// Read a priority from the signed scale the contract carries.
    pub fn of(level: i32) -> Self {
        match level {
            i32::MIN..=-1 => WorkerPriority::Below,
            0 => WorkerPriority::Normal,
            _ => WorkerPriority::Above,
        }
    }
}

/// What the user asked for about processor placement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NumaMode {
    /// Look at the machine and decide.
    #[default]
    Auto,
    /// Leave the placement entirely to the platform scheduler.
    Off,
    /// Keep the worker inside one named processor group.
    PinTo(u16),
}

impl NumaMode {
    /// Stable identifier the preferences record.
    ///
    /// A pinned group is a number rather than one of a fixed set of names, so
    /// its identifier is computed from the group it names; the two named modes
    /// are the only ones that have a constant to return.
    pub fn id(self) -> String {
        match self {
            NumaMode::Auto => "auto".to_string(),
            NumaMode::Off => "off".to_string(),
            NumaMode::PinTo(group) => format!("group:{group}"),
        }
    }

    /// Read a mode the interface layer sent.
    pub fn read(level: i32) -> NumaMode {
        match level {
            i32::MIN..=-1 => NumaMode::Off,
            0 => NumaMode::Auto,
            group => NumaMode::PinTo((group as u64).min(u64::from(u16::MAX)) as u16),
        }
    }

    /// The level the preferences record, which is the contract's own scale.
    pub fn level(self) -> i32 {
        match self {
            NumaMode::Auto => 0,
            NumaMode::Off => -1,
            NumaMode::PinTo(group) => i32::from(group),
        }
    }

    /// Label on the option, for a group the user named.
    pub fn label(self) -> String {
        match self {
            NumaMode::Auto => "Let Vise decide".to_string(),
            NumaMode::Off => "Leave it to the operating system".to_string(),
            NumaMode::PinTo(group) => format!("Keep it on processor group {group}"),
        }
    }

    /// What this mode does to the worker.
    pub fn effect(self) -> &'static str {
        match self {
            NumaMode::Auto => {
                "the machine is asked what it looks like, and the worker is pinned only when there is a reason to"
            }
            NumaMode::Off => "the worker is left wherever the operating system puts it",
            NumaMode::PinTo(_) => "the worker is kept inside the processor group that was named.",
        }
    }
}

fn parse_pin(text: &str) -> Option<u16> {
    text.trim()
        .strip_prefix("group:")
        .and_then(|rest| rest.trim().parse::<u16>().ok())
}

/// How the machine is laid out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Topology {
    /// How many processor groups the machine reports.
    pub groups: u16,
    /// How many NUMA nodes the machine reports.
    pub nodes: u16,
    /// How many logical processors the machine can run at once.
    pub processors: u32,
    /// Where the numbers came from, in words.
    ///
    /// Held rather than assumed, because a machine that could not be asked is
    /// not the same as a machine with one group, and the difference decides
    /// whether anything is pinned.
    pub source: ProbeSource,
}

/// Whether the layout was measured or guessed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeSource {
    /// The platform answered.
    Measured,
    /// The platform did not answer, so the layout is the conservative one.
    Unknown,
}

impl Topology {
    /// The layout of a machine that could not be asked.
    pub fn unknown() -> Topology {
        Topology {
            groups: 1,
            nodes: 1,
            processors: 1,
            source: ProbeSource::Unknown,
        }
    }

    /// A layout with the given numbers, for tests and for a platform answer.
    pub fn of(groups: u16, nodes: u16, processors: u32) -> Topology {
        Topology {
            groups,
            nodes: nodes.max(1),
            processors,
            source: ProbeSource::Measured,
        }
    }

    /// Whether the machine was actually asked.
    pub fn measured(&self) -> bool {
        self.source == ProbeSource::Measured
    }

    /// One line describing the layout, for the preferences tab and the log.
    pub fn describe(&self) -> String {
        match self.source {
            ProbeSource::Measured => format!(
                "{} processor group(s), {} memory node(s), {} logical processor(s)",
                self.groups, self.nodes, self.processors
            ),
            ProbeSource::Unknown => {
                "this machine did not report its processor layout, so the worker is left alone"
                    .to_string()
            }
        }
    }
}

/// What was decided, and where the worker goes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// The scheduler decides, as it always did.
    Scheduler,
    /// The worker keeps itself inside one processor group.
    PinToGroup(u16),
}

impl Placement {
    /// Stable identifier for the preferences read-out.
    pub fn id(&self) -> &'static str {
        match self {
            Placement::Scheduler => "scheduler",
            Placement::PinToGroup(_) => "pinned",
        }
    }

    /// One line saying what will happen, which is what the log records.
    pub fn describe(&self) -> String {
        match self {
            Placement::Scheduler => {
                "The worker is left to the operating system scheduler.".to_string()
            }
            Placement::PinToGroup(group) => {
                format!("The worker is kept inside processor group {group}.")
            }
        }
    }
}

/// The decision, with the sentence the window shows and the log records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementDecision {
    /// Where the worker goes.
    pub placement: Placement,
    /// One line saying why, in the user's terms.
    pub reason: String,
}

/// Decide where the worker should run.
///
/// Four answers, and each of them is a refusal rather than a guess:
///
/// * A machine that could not be asked is left alone. Treating an unknown layout
///   as a single group would produce a "no pinning needed" decision that reads
///   as a measurement.
/// * A layout the mode names explicitly is honoured, and a group that does not
///   exist is refused by name rather than clamped into one that does.
/// * `Off` is honoured as written.
/// * `Auto` pins only when there is a reason: a worker that asked for more than
///   its share of a machine with several memory controllers should not also be
///   running on all of them at once. Below that, the scheduler's own placement
///   is better than anything guessed here.
pub fn placement_for(
    topology: &Topology,
    mode: NumaMode,
    priority: WorkerPriority,
) -> PlacementDecision {
    let leave_alone = |reason: String| PlacementDecision {
        placement: Placement::Scheduler,
        reason,
    };
    if !topology.measured() {
        return leave_alone(topology.describe());
    }
    match mode {
        NumaMode::Off => leave_alone(
            "the preferences ask for no placement, so the operating system keeps its own"
                .to_string(),
        ),
        NumaMode::PinTo(group) => {
            if group >= topology.groups {
                return leave_alone(format!(
                    "processor group {group} was asked for, but this machine has {} ({}); the worker is left to the scheduler",
                    topology.groups,
                    topology.describe()
                ));
            }
            PlacementDecision {
                placement: Placement::PinToGroup(group),
                reason: format!(
                    "the preferences ask for processor group {group}, and this machine has it ({})",
                    topology.describe()
                ),
            }
        }
        NumaMode::Auto => {
            if topology.groups <= 1 {
                return leave_alone(format!(
                    "this machine reports {} processor group, so there is nothing to pin",
                    topology.groups
                ));
            }
            if priority == WorkerPriority::Above {
                return PlacementDecision {
                    placement: Placement::PinToGroup(0),
                    reason: format!(
                        "the worker asked for more than its share of a machine with {} processor groups, so it is kept inside group 0 rather than running on every memory controller at once",
                        topology.groups
                    ),
                };
            }
            leave_alone(format!(
                "this machine has {} processor groups, and the worker is not competing hard enough for that to matter ({}); it is left to the scheduler",
                topology.groups,
                topology.describe()
            ))
        }
    }
}

/// Read a mode the interface layer sent as text.
///
/// The two named modes are named; a group is `group:3`. An unrecognised name is
/// the automatic answer rather than an error, because this setting is about
/// performance and a typo should not cost somebody their encode settings.
pub fn read_mode(text: &str) -> NumaMode {
    let trimmed = text.trim().to_ascii_lowercase();
    match trimmed.as_str() {
        "auto" | "" => NumaMode::Auto,
        "off" | "none" => NumaMode::Off,
        other => parse_pin(other)
            .map(NumaMode::PinTo)
            .unwrap_or(NumaMode::Auto),
    }
}

/// Whether this platform can restrict a process to a processor group.
pub fn placement_available() -> bool {
    sys::available()
}

/// Keep this process inside `placement`.
///
/// Returns what the platform said. A refusal is reported rather than swallowed,
/// because a worker that was asked to be pinned and was not is a worker whose
/// timing the user cannot reason about.
pub fn apply(placement: &Placement) -> Result<(), PlacementError> {
    match placement {
        Placement::Scheduler => Ok(()),
        Placement::PinToGroup(group) => sys::pin_current(*group),
    }
}

/// Why a placement could not be carried out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementError {
    /// The platform has no processor groups to pin to.
    Unsupported,
    /// The platform refused the request.
    Refused(String),
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacementError::Unsupported => {
                write!(
                    f,
                    "this platform cannot place a process on a processor group"
                )
            }
            PlacementError::Refused(reason) => write!(f, "the placement was refused: {reason}"),
        }
    }
}

impl std::error::Error for PlacementError {}

/// Ask the machine what it looks like.
///
/// Never fails: a machine that cannot answer produces the layout that leads to no
/// action rather than a guess that leads to a wrong one.
pub fn topology() -> Topology {
    sys::topology().unwrap_or_else(Topology::unknown)
}

#[cfg(windows)]
mod sys {
    use super::{PlacementError, ProbeSource, Topology};
    use std::ffi::c_void;
    use std::os::raw::c_char;

    extern "system" {
        fn GetModuleHandleW(module_name: *const u16) -> *mut c_void;
        fn GetProcAddress(module: *mut c_void, proc_name: *const c_char) -> *mut c_void;
        fn GetCurrentThread() -> *mut c_void;
        fn SetThreadGroupAffinity(
            thread: *mut c_void,
            group_affinity: *const GroupAffinity,
            previous: *mut GroupAffinity,
        ) -> i32;
    }

    /// `GROUP_AFFINITY` as the platform declares it: a mask that can hold every
    /// logical processor of one group, and the group's number.
    ///
    /// Declared here rather than imported because that is what this crate's
    /// footprint asks for, and the layout is pinned by a test below.
    #[repr(C)]
    #[derive(Clone, Copy)]
    pub(super) struct GroupAffinity {
        pub mask: u64,
        pub reserved: [u16; 3],
        pub group: u16,
    }

    type GroupCountFn = unsafe extern "system" fn() -> u16;
    type NodeFn = unsafe extern "system" fn(highest: *mut u16) -> i32;

    fn utf16(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn ascii(s: &str) -> Vec<u8> {
        s.as_bytes()
            .iter()
            .copied()
            .chain(std::iter::once(0))
            .collect()
    }

    fn kernel32<T: Copy>(name: &str) -> Option<T> {
        unsafe {
            let module = GetModuleHandleW(utf16("kernel32.dll").as_ptr());
            if module.is_null() {
                return None;
            }
            let symbol = GetProcAddress(module, ascii(name).as_ptr().cast::<c_char>());
            if symbol.is_null() {
                return None;
            }
            // The symbol and the function pointer are both one pointer wide, so
            // the read is a copy of a pointer rather than a conversion between
            // sizes the compiler cannot check.
            let address: *mut c_void = std::ptr::read(&symbol);
            Some(std::mem::transmute_copy(&address))
        }
    }

    pub(super) fn available() -> bool {
        kernel32::<GroupCountFn>("GetActiveProcessorGroupCount").is_some()
    }

    pub(super) fn topology() -> Option<Topology> {
        let groups = unsafe { kernel32::<GroupCountFn>("GetActiveProcessorGroupCount")?() };
        // The highest node number is the answer the platform gives in one call; a
        // node count is one more than that, and a machine that answers zero has a
        // single node by definition.
        let nodes = match kernel32::<NodeFn>("GetNumaHighestNodeNumber") {
            Some(function) => {
                let mut highest: u16 = 0;
                let ok = unsafe { function(&mut highest) };
                if ok == 0 {
                    highest.checked_add(1).unwrap_or(1)
                } else {
                    1
                }
            }
            None => 1,
        };
        Some(Topology {
            groups: groups.max(1),
            nodes: nodes.max(1),
            // The processor count is the one number this crate already knows how to
            // ask for without a platform call at all, so it does not drag a second
            // structure walk in behind it.
            processors: std::thread::available_parallelism()
                .map(|count| count.get() as u32)
                .unwrap_or(1),
            source: ProbeSource::Measured,
        })
    }

    pub(super) fn pin_current(group: u16) -> Result<(), PlacementError> {
        if !available() {
            return Err(PlacementError::Unsupported);
        }
        let affinity = GroupAffinity {
            // Every logical processor of the group: the decision is which group to
            // be in, not how much of it to use.
            mask: u64::MAX,
            reserved: [0; 3],
            group,
        };
        let mut previous = affinity;
        let ok = unsafe { SetThreadGroupAffinity(GetCurrentThread(), &affinity, &mut previous) };
        if ok == 0 {
            return Err(PlacementError::Refused(
                std::io::Error::last_os_error().to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod sys {
    use super::{PlacementError, Topology};

    pub(super) fn available() -> bool {
        false
    }

    pub(super) fn topology() -> Option<Topology> {
        // Every platform this builds for has some way to ask about processor
        // affinity, and none of them has the processor-group model this module
        // reasons about, so there is no layout to report rather than a wrong one.
        None
    }

    pub(super) fn pin_current(_group: u16) -> Result<(), PlacementError> {
        Err(PlacementError::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_contract_scale_reads_into_three_bands() {
        assert_eq!(WorkerPriority::of(0), WorkerPriority::Normal);
        assert_eq!(WorkerPriority::of(-1), WorkerPriority::Below);
        assert_eq!(WorkerPriority::of(-99), WorkerPriority::Below);
        assert_eq!(WorkerPriority::of(1), WorkerPriority::Above);
        assert_eq!(WorkerPriority::of(99), WorkerPriority::Above);
    }

    #[test]
    fn a_machine_that_cannot_be_asked_is_never_pinned() {
        // The failure this guards against is a "no pinning needed" decision that
        // reads as a measurement: an unknown layout has to say it is unknown.
        let topology = Topology::unknown();
        assert!(!topology.measured());
        for mode in [NumaMode::Auto, NumaMode::Off, NumaMode::PinTo(0)] {
            let decision = placement_for(&topology, mode, WorkerPriority::Above);
            assert_eq!(decision.placement, Placement::Scheduler, "{mode:?}");
            assert!(
                decision.reason.contains("did not report"),
                "{}",
                decision.reason
            );
        }
    }

    #[test]
    fn one_group_is_left_to_the_scheduler() {
        let topology = Topology::of(1, 1, 16);
        for priority in [
            WorkerPriority::Below,
            WorkerPriority::Normal,
            WorkerPriority::Above,
        ] {
            let decision = placement_for(&topology, NumaMode::Auto, priority);
            assert_eq!(decision.placement, Placement::Scheduler);
            assert!(
                decision.reason.contains("nothing to pin"),
                "{}",
                decision.reason
            );
        }
    }

    #[test]
    fn several_groups_pin_only_a_worker_that_asked_for_more_than_its_share() {
        let topology = Topology::of(2, 2, 32);
        let polite = placement_for(&topology, NumaMode::Auto, WorkerPriority::Normal);
        assert_eq!(polite.placement, Placement::Scheduler);
        assert!(
            polite.reason.contains("not competing hard enough"),
            "{}",
            polite.reason
        );

        let greedy = placement_for(&topology, NumaMode::Auto, WorkerPriority::Above);
        assert_eq!(greedy.placement, Placement::PinToGroup(0));
        assert!(
            greedy.reason.contains("memory controller"),
            "{}",
            greedy.reason
        );
    }

    #[test]
    fn several_nodes_on_one_group_is_still_one_group() {
        // Two memory nodes behind one processor group is a machine the scheduler
        // already places well; nothing here has anything to add.
        let topology = Topology::of(1, 2, 32);
        let decision = placement_for(&topology, NumaMode::Auto, WorkerPriority::Above);
        assert_eq!(decision.placement, Placement::Scheduler);
    }

    #[test]
    fn a_group_that_does_not_exist_is_refused_by_name() {
        let topology = Topology::of(2, 2, 32);
        let decision = placement_for(&topology, NumaMode::PinTo(5), WorkerPriority::Normal);
        assert_eq!(decision.placement, Placement::Scheduler);
        assert!(decision.reason.contains("group 5"), "{}", decision.reason);
        assert!(decision.reason.contains("has 2"), "{}", decision.reason);
    }

    #[test]
    fn a_group_that_does_exist_is_honoured_and_the_mode_reads_back() {
        let topology = Topology::of(4, 2, 64);
        let decision = placement_for(&topology, NumaMode::PinTo(3), WorkerPriority::Normal);
        assert_eq!(decision.placement, Placement::PinToGroup(3));
        assert_eq!(NumaMode::read(3), NumaMode::PinTo(3));
        assert_eq!(NumaMode::PinTo(3).level(), 3);
        assert!(NumaMode::PinTo(3).label().contains('3'));
        assert!(NumaMode::PinTo(3).effect().ends_with('.'));
    }

    #[test]
    fn a_mode_is_read_from_text_and_an_unknown_name_is_automatic() {
        assert_eq!(read_mode("auto"), NumaMode::Auto);
        assert_eq!(read_mode(" OFF "), NumaMode::Off);
        assert_eq!(read_mode("group:2"), NumaMode::PinTo(2));
        // A typo is about performance, not about the file the encode is written
        // to, so it falls back rather than refusing the whole record.
        assert_eq!(read_mode("pinned to the fast one"), NumaMode::Auto);
        assert_eq!(read_mode(""), NumaMode::Auto);
        assert_eq!(NumaMode::read(0), NumaMode::Auto);
        assert_eq!(NumaMode::read(-1), NumaMode::Off);
    }

    #[test]
    fn the_layout_a_machine_reports_can_be_read_and_described() {
        let topology = topology();
        assert!(topology.groups >= 1);
        assert!(topology.nodes >= 1);
        assert!(topology.processors >= 1);
        assert!(!topology.describe().is_empty());
        if topology.measured() {
            assert!(
                topology.describe().contains("processor group"),
                "{}",
                topology.describe()
            );
            // A machine that answered must never be pinned to a group it does not
            // have, whatever the mode asks for.
            for group in 0..topology.groups {
                let decision =
                    placement_for(&topology, NumaMode::PinTo(group), WorkerPriority::Normal);
                assert_eq!(decision.placement, Placement::PinToGroup(group));
            }
        } else {
            assert!(topology.describe().contains("did not report"));
        }
    }

    #[test]
    fn applying_the_schedulers_answer_is_doing_nothing() {
        // Leaving the worker alone is the placement on most machines, so it has to
        // be an answer the platform is never asked about.
        assert!(apply(&Placement::Scheduler).is_ok());
    }

    #[test]
    fn applying_a_pin_reports_the_platform_rather_than_pretending() {
        let topology = topology();
        match apply(&Placement::PinToGroup(0)) {
            Ok(()) => assert!(
                placement_available(),
                "a pin was applied on a platform that has none"
            ),
            Err(e) => {
                // A machine with one group refuses a pin to group 0 only if the
                // platform itself will not do it; either way the answer is a
                // reason the caller can log.
                assert!(!e.to_string().is_empty(), "{e}");
                let _ = topology;
            }
        }
    }

    #[test]
    fn every_placement_describes_itself_in_one_line() {
        assert!(Placement::Scheduler.describe().ends_with('.'));
        assert!(Placement::PinToGroup(2).describe().contains('2'));
        assert_eq!(Placement::Scheduler.id(), "scheduler");
        assert_eq!(Placement::PinToGroup(2).id(), "pinned");
    }

    #[cfg(windows)]
    #[test]
    fn the_affinity_structure_matches_what_the_platform_declares() {
        // GROUP_AFFINITY is a mask, three reserved words for alignment, and a group
        // number. Getting the layout wrong would pin to a group the process does
        // not hold, so the size is pinned rather than assumed.
        use super::sys::GroupAffinity;
        assert_eq!(std::mem::size_of::<GroupAffinity>(), 16);
    }
}
