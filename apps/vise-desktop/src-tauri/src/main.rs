#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The Vise desktop shell.
//!
//! The window never runs the transcode pipeline itself. Scanning and encoding
//! happen in the background worker process, reached over its loopback,
//! token-authenticated JSON API, so an encoder crash costs a job rather than
//! the application. Process isolation can be switched off in the portable
//! settings file; only then does the shell fall back to calling the core
//! library in-process.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::menu::IsMenuItem;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};
use vise_core::{Job, JobId, JobSpec, JobState, Progress, Session};
use vise_ipc::contract::{CommandResult, EncodeCommand, InitCommand, ScanCommand, WorkState};
use vise_ipc::portable::PortableSettings;
use vise_ipc::timespan::TimeSpan;
use vise_ipc::validation::ValidationReport;
use vise_ipc::{WorkerClient, WorkerConfig, WorkerSupervisor};

mod appearance;
mod locale;
mod preferences;
mod scanflow;
mod sources;
mod system;
mod tabs;
mod viewmodel;
mod windows;

use appearance::{AppearanceState, AppearanceViewModel, SystemTheme};
use locale::{LanguageSwitch, LanguageViewModel, LANGUAGE_EVENT};
use preferences::{PreferencesSettings, PreferencesState, PreferencesViewModel};
use scanflow::{scan_in_process, ScanFlow};
use sources::{RecentSources, SourceDropReport, SourceViewModel};
use system::tray::{TrayAction, TrayItemKind, TrayModel};
use system::{Integration, SystemFacts};
use tabs::{
    AudioDefaultsInput, AudioTrackInput, ChapterImportInput, ChaptersInput, DimensionsInput,
    FilterInput, MarkerInput, MetadataInput, SubtitleTrackInput, SubtitlesInput, SummaryInput,
    TabSetViewModel, TabSettings, TitleFacts, VideoInput,
};
use viewmodel::{
    RangeSelection, ScanPhase, ScanProgressViewModel, SelectionViewModel, TrackSelectionInput,
};
use windows::diagnostics::{DiagnosticsState, DiagnosticsViewModel, ReportViewModel};
use windows::dialogs::{self, AboutViewModel, AlertAction, AlertState, ErrorState, ErrorViewModel};
use windows::log::ActivityLog;
use windows::presets::{PresetAction, PresetInput, PresetState};
use windows::preview::{CropOverlay, PreviewState, PreviewViewModel, StillImage, StillPosition};
use windows::queue::{QueueSelectionInput, QueueState};
use windows::SurfaceState;

/// Label the configuration gives the application window.
///
/// The appearance of the window is read from and written to this one window, so
/// its label is named here rather than repeated at every use.
const MAIN_WINDOW: &str = "main";

/// Event the page listens to for a theme the operating system changed.
const APPEARANCE_EVENT: &str = "appearance-changed";
/// Event carrying a question the window asked for itself.
///
/// The countdown alert is normally opened by the page pressing a control; the
/// when-done preferences open it without one, because the queue finishing is not
/// something the user does. The answer still travels the same way round.
const ALERT_EVENT: &str = "alert-asked";
/// Event carrying a surface the notification-area icon asked the page to open.
///
/// A tray item is pressed by somebody who is not looking at the page, so the window
/// decides what is open and tells the page, rather than the page being asked to
/// open it.
const TRAY_EVENT: &str = "tray-action";

/// How the shell reaches the transcode pipeline.
enum Backend {
    /// A background worker process.
    Worker(Box<WorkerSupervisor>),
    /// In-process execution, only when portable settings disable isolation.
    InProcess,
}

impl Backend {
    /// A client for the running worker, if isolation is enabled.
    fn client(&mut self) -> Option<WorkerClient> {
        match self {
            Backend::Worker(supervisor) => {
                // A worker that died underneath the window is replaced here, so
                // a failed job never becomes an application failure.
                match supervisor.ensure_running() {
                    Ok(connection) => Some(connection.client()),
                    Err(e) => {
                        eprintln!("worker restart failed: {e}");
                        None
                    }
                }
            }
            Backend::InProcess => None,
        }
    }

    /// One-line description used in the activity log.
    fn describe(&self) -> String {
        match self {
            Backend::Worker(supervisor) => match supervisor.connection() {
                Some(connection) => format!(
                    "worker process {} on 127.0.0.1:{} ({} restart(s))",
                    connection.pid,
                    connection.port,
                    supervisor.restarts()
                ),
                None => "worker process is not running".to_string(),
            },
            Backend::InProcess => {
                "running in process: process isolation is switched off".to_string()
            }
        }
    }
}

#[derive(Default)]
struct DesktopState {
    session: Session,
    /// The activity history, from this window and from the worker.
    activity: ActivityLog,
    backend: Option<Backend>,
    /// The source surface: source, scan, titles, and the current selection.
    flow: ScanFlow,
    /// The configuration tab set.
    tabs: TabSettings,
    /// Sources the user opened recently.
    recents: RecentSources,
    /// Where the recent source list is persisted.
    settings_path: PathBuf,
    /// Monotonic stamp that orders the recent source list.
    recents_stamp: u64,
    /// Which secondary windows and dialogs are open.
    surfaces: SurfaceState,
    /// The queue's own order and priorities.
    queue: QueueState,
    /// What the queue selection sheet is holding.
    selection: QueueSelectionInput,
    /// The still frames the preview window is looking at.
    preview: PreviewState,
    /// Everything that went wrong.
    errors: ErrorState,
    /// The action waiting for its countdown.
    alerts: AlertState,
    /// The action the user has confirmed but not yet acted on.
    confirmed: Option<AlertAction>,
    /// The preset library.
    presets: PresetState,
    /// Theme and layout direction: the preference, the observation, and the
    /// answer both of them add up to.
    appearance: AppearanceState,
    /// The preferences surface: the choices, what they were read from, and the
    /// record a reset can be taken back from.
    preferences: PreferencesState,
    /// Whether the When-done preferences have already been acted on for the drain
    /// in progress.
    ///
    /// Set when a question is put and cleared the moment the queue has something
    /// in it again, so two jobs finishing together ask once rather than twice, and
    /// a queue that drains a second time asks again.
    when_done_armed: AtomicBool,
    /// Everything that integrates the application with the desktop: the tray, the
    /// notifications, the hold on the machine, the update check, and where the
    /// queue is written.
    integration: Integration,
    /// Why the queue has stopped, when something in the way stopped it.
    ///
    /// Held rather than derived, because the reason is a sentence the window
    /// composed — there was not enough room on the disk — and a tray line that
    /// recomputed it from a number would read like a database column.
    queue_pause: Option<String>,
    /// Where the preferences and the queue are written.
    storage: vise_ipc::portable::StorageRoots,
    /// The diagnostics surface: per-encode logs, the hardware report, the
    /// quality sweep, and the reports this window has produced.
    diagnostics: DiagnosticsState,
    /// Whether the window is on screen and in front.
    ///
    /// An observation rather than a preference, and it is the difference between a
    /// notification about a finished queue and a line in the activity log: somebody
    /// watching the window has already seen it finish.
    window_visible: AtomicBool,
}

type SharedState = Arc<Mutex<DesktopState>>;

fn push_log(state: &SharedState, message: impl Into<String>) {
    if let Ok(mut guard) = state.lock() {
        guard.activity.record(message);
    }
}

/// Record a problem and hand back its number.
///
/// Everything that fails for a reason the user can act on goes through here, so
/// the error view is a record of what happened rather than a place the page
/// composes strings on its own.
fn record_problem(
    state: &SharedState,
    source: &str,
    context: impl Into<String>,
    detail: impl Into<String>,
) {
    if let Ok(mut guard) = state.lock() {
        guard.errors.record(source, &context.into(), detail);
    }
}

/// Record a warning.
fn record_warning(
    state: &SharedState,
    source: &str,
    context: impl Into<String>,
    detail: impl Into<String>,
) {
    if let Ok(mut guard) = state.lock() {
        guard.errors.push_warning(source, &context.into(), detail);
    }
}

/// Bring the queue's order in line with the session.
fn sync_queue(guard: &mut DesktopState) {
    let jobs = guard.session.jobs().to_vec();
    guard.queue.sync(&jobs);
}

/// The queue, as the page renders it, for a state that is already locked.
fn queue_view(guard: &DesktopState) -> windows::queue::QueueViewModel {
    windows::queue::queue_view(&guard.queue, guard.session.jobs())
}

/// Whether the user has confirmed `action` and has not acted on it yet.
///
/// The countdown alert is only worth having if it is load-bearing: an action
/// that destroys work asks first, and the answer has to come from the alert
/// rather than from the same control that asked for it.
fn is_confirmed(state: &SharedState, action: AlertAction) -> bool {
    state
        .lock()
        .ok()
        .map(|guard| guard.confirmed == Some(action))
        .unwrap_or(false)
}

/// Refuse `action` until it has been confirmed, with the reason to show.
fn require_confirmation(state: &SharedState, action: AlertAction) -> Result<(), String> {
    if is_confirmed(state, action) {
        return Ok(());
    }
    Err(format!(
        "'{}' cannot be undone, so it asks for confirmation first.",
        action.verb()
    ))
}

/// Spend a confirmation, so one answer cannot drive two actions.
fn take_confirmation(state: &SharedState, action: AlertAction) {
    if let Ok(mut guard) = state.lock() {
        if guard.confirmed == Some(action) {
            guard.confirmed = None;
        }
    }
}

/* ------------------------------------------------------- desktop integration */

/// The facts the system module decides on, read out of the window.
///
/// Everything here is read rather than remembered: the queue's own methods say
/// what is waiting, what is running, and what has finished, so the tray and the
/// notifications cannot describe a queue that the queue window is not showing.
fn system_facts(guard: &DesktopState) -> SystemFacts {
    let jobs = guard.session.jobs();
    let finished: Vec<&Job> = jobs
        .iter()
        .filter(|job| windows::queue::is_finished(job.status.state))
        .collect();
    // The last failure is carried rather than recomputed, because the notification
    // about it is about a change that happened once: reading "the most recent
    // failed job" on every poll would announce the same failure for as long as it
    // stayed in the queue.
    let last_failure = finished
        .iter()
        .rev()
        .find(|job| job.status.state == JobState::Failed)
        .map(|job| {
            (
                job.spec.output_path.clone(),
                job.status.message.clone().unwrap_or_default(),
            )
        });
    SystemFacts {
        queued: guard.queue.pending(jobs).len(),
        running: guard.queue.active(jobs).len(),
        finished: finished.len(),
        completed: finished
            .iter()
            .filter(|job| job.status.state == JobState::Completed)
            .count(),
        failed: finished
            .iter()
            .filter(|job| job.status.state == JobState::Failed)
            .count(),
        last_failure,
        window_visible: guard.window_visible.load(Ordering::SeqCst),
        in_process: !matches!(guard.backend.as_ref(), Some(Backend::Worker(_))),
        portable: guard.storage.portable,
        update_available: guard.integration.updates().available,
        queue_paused: guard.queue_pause.clone(),
    }
}

/// What the preferences and the window say about notifications.
fn notify_policy(guard: &DesktopState) -> system::notify::NotifyPolicy {
    let advanced = &guard.preferences.settings().advanced;
    system::notify::NotifyPolicy {
        enabled: advanced.notifications,
        scope: system::notify::NotifyScope::of(advanced.notification_scope.as_str()),
    }
}

/// Carry one effect out.
///
/// The decisions are made by [`Integration`] as plain values and acted on here, so
/// that what the integration decided can be checked in a test without a window and
/// without an operating system.
fn carry_out(state: &SharedState, effect: system::Effect) {
    match effect {
        system::Effect::Log(line) => push_log(state, line),
        system::Effect::Notify(notification) => {
            if let Ok(mut guard) = state.lock() {
                guard.integration.show(notification);
            }
        }
        system::Effect::HoldPower(scope) => {
            let request = vise_ipc::PowerRequest::Hold(vise_ipc::PowerScope::of(&scope));
            if let Err(e) = vise_ipc::power::request(request) {
                push_log(state, format!("power management: {e}"));
            }
        }
        system::Effect::ReleasePower => {
            if let Err(e) = vise_ipc::power::request(vise_ipc::PowerRequest::Release) {
                push_log(state, format!("power management: {e}"));
            }
        }
        system::Effect::Tray(model) => publish_tray(&model),
        system::Effect::CheckForUpdates => {
            let shared: SharedState = state.clone();
            thread::spawn(move || check_for_releases(&shared));
        }
    }
}

/// Decide everything the desktop integration needs, and carry it out.
///
/// Called wherever the queue or the window has changed, rather than on a timer: a
/// tray tooltip that lags behind the queue is worse than one that is a moment out
/// of date, and there is no cost in asking when the answer is the same — the
/// integration only produces effects for what actually changed.
fn refresh_system(state: &SharedState) {
    let effects = {
        let Ok(mut guard) = state.lock() else {
            return;
        };
        let facts = system_facts(&guard);
        let advanced = guard.preferences.settings().advanced.clone();
        let policy = system::power::PowerPolicy {
            prevent_sleep: advanced.prevent_sleep,
            scope: vise_ipc::PowerScope::of(&advanced.power_scope),
        };
        let notify = notify_policy(&guard);
        guard.integration.refresh(&facts, policy, notify)
    };
    for effect in effects {
        carry_out(state, effect);
    }
}

/// The application handle, once there is a window.
///
/// A `OnceLock` rather than an argument on every function, because the tray, the
/// notification area, and the update check all happen away from the command that
/// started them and all need the window that owns them.
static HANDLE: OnceLock<AppHandle> = OnceLock::new();

/// The application handle, when the window exists.
fn app_handle() -> Option<AppHandle> {
    HANDLE.get().cloned()
}

/// The tray icon, once the platform has given the window one.
static TRAY: OnceLock<Mutex<Option<tauri::tray::TrayIcon<tauri::Wry>>>> = OnceLock::new();

/// The tray icon, when there is one.
fn tray_icon() -> Option<tauri::tray::TrayIcon<tauri::Wry>> {
    TRAY.get()
        .and_then(|slot| slot.lock().ok().and_then(|guard| guard.clone()))
}

/// Publish the tray menu and its tooltip.
///
/// The menu is rebuilt rather than edited: the model is small, and rebuilding is
/// the one way a row cannot be left showing what was true when it was created. The
/// set happens on its own thread because the platform wants the menu thread and the
/// page thread to be different.
fn publish_tray(model: &TrayModel) {
    let (Some(app), Some(tray)) = (app_handle(), tray_icon()) else {
        return;
    };
    let mut items: Vec<tauri::menu::MenuItemKind<tauri::Wry>> = Vec::new();
    for item in &model.items {
        // A separator is not a row, and a row that cannot be pressed is published
        // as a disabled one rather than left out, so the menu does not change shape
        // from one poll to the next.
        if item.kind == TrayItemKind::Separator {
            continue;
        }
        let built = match item.kind {
            TrayItemKind::Tick => tauri::menu::CheckMenuItem::with_id(
                &app,
                item.id.clone(),
                item.label.clone(),
                item.available,
                item.ticked,
                None::<&str>,
            )
            .map(|menu_item| menu_item.kind()),
            _ => tauri::menu::MenuItem::with_id(
                &app,
                item.id.clone(),
                item.label.clone(),
                item.available,
                None::<&str>,
            )
            .map(|menu_item| menu_item.kind()),
        };
        if let Ok(kind) = built {
            items.push(kind);
        }
    }
    let borrowed: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = items
        .iter()
        .map(|item| item as &dyn tauri::menu::IsMenuItem<tauri::Wry>)
        .collect();
    let tooltip = model.tooltip.clone();
    match tauri::menu::MenuBuilder::new(&app).items(&borrowed).build() {
        Ok(menu) => {
            thread::spawn(move || {
                let _ = tray.set_menu(Some(menu));
                let _ = tray.set_tooltip(Some(tooltip));
            });
        }
        Err(e) => eprintln!("the tray menu could not be built: {e}"),
    }
}

/// Write the queue where the storage settings say it goes.
///
/// Written after anything that changes it rather than on a timer, because a queue
/// file that is a moment out of date is a queue that comes back without a job the
/// user just queued.
fn persist_queue(state: &SharedState) {
    let result = {
        let Ok(mut guard) = state.lock() else {
            return;
        };
        sync_queue(&mut guard);
        let path = guard.integration.queue_path().to_path_buf();
        system::persistence::save(&path, guard.session.jobs(), &guard.queue)
    };
    if let Err(problem) = result {
        push_log(state, problem);
    }
}

/// Ask whether there is room for `spec`, and pause the queue when there is not.
///
/// Asked per job rather than once for the queue: a queue of forty jobs can run out
/// of room halfway through, and the job that would have run out is the one that has
/// to be refused rather than discovered half-written.
fn gate_disk_space(state: &SharedState, id: JobId, spec: &JobSpec) -> Result<(), String> {
    let reading = match system::diskspace::read_free_space(&spec.output_path) {
        Ok(reading) => reading,
        Err(e) => {
            push_log(state, format!("the free space could not be read: {e}"));
            system::diskspace::SpaceReading::unknown()
        }
    };
    let duration = state
        .lock()
        .ok()
        .map(|guard| title_facts(&guard).duration_seconds);
    let existing = std::fs::metadata(&spec.output_path)
        .ok()
        .map(|meta| meta.len());
    match system::diskspace::gate(spec, reading, duration, existing) {
        system::diskspace::Gate::Proceed { free } => {
            let was_paused = {
                let Ok(mut guard) = state.lock() else {
                    return Ok(());
                };
                let was_paused = guard.queue_pause.take();
                if let Some(free) = free {
                    guard.activity.record(format!(
                        "{} free before job {id}",
                        system::diskspace::format_bytes(free)
                    ));
                }
                was_paused
            };
            if was_paused.is_some() {
                push_log(
                    state,
                    "there is room again, so the queue is no longer paused",
                );
            }
            Ok(())
        }
        system::diskspace::Gate::Pause { reason, .. } => {
            if let Ok(mut guard) = state.lock() {
                guard.queue_pause = Some(reason.clone());
            }
            Err(format!(
                "Job {id} cannot start: {reason}. The queue has paused; make room and start it again."
            ))
        }
    }
}

/// Run the pre-encode action, and say whether the encode may go ahead.
fn run_pre_action(state: &SharedState, id: JobId, spec: &JobSpec) -> Result<(), String> {
    let (command, timeout, on_failure, title) = {
        let Ok(guard) = state.lock() else {
            return Ok(());
        };
        let advanced = &guard.preferences.settings().advanced;
        (
            advanced.pre_action.clone(),
            system::actions::Timeout::seconds(advanced.action_timeout_seconds),
            system::actions::OnFailure::read(advanced.action_on_failure),
            title_facts(&guard).name,
        )
    };
    let variables = system::actions::Variables::of(spec, &title, id);
    match system::actions::run_pre(&command, &variables, timeout, on_failure) {
        Err(problem) => Err(format!(
            "the pre-encode action could not be read: {problem}"
        )),
        Ok(system::actions::Verdict::Refuse { reason }) => Err(reason),
        Ok(system::actions::Verdict::Proceed { log }) => {
            for line in log {
                push_log(state, line);
            }
            Ok(())
        }
    }
}

/// Run the post-encode action.
///
/// Every outcome is a line in the activity log: the file has already been written,
/// so there is nothing left for this to spoil, and a copy program that fails must
/// not be able to fail an encode that is already done.
fn run_post_action(state: &SharedState, id: JobId, spec: &JobSpec) {
    let (command, timeout, title) = {
        let Ok(guard) = state.lock() else {
            return;
        };
        let advanced = &guard.preferences.settings().advanced;
        (
            advanced.post_action.clone(),
            system::actions::Timeout::seconds(advanced.action_timeout_seconds),
            title_facts(&guard).name,
        )
    };
    let variables = system::actions::Variables::of(spec, &title, id);
    match system::actions::run_post(&command, &variables, timeout) {
        Ok(lines) => {
            for line in lines {
                push_log(state, line);
            }
        }
        Err(problem) => push_log(
            state,
            format!("the post-encode action could not be read: {problem}"),
        ),
    }
}

/// Build the notification-area icon, if this platform has one.
///
/// Built once and then only re-published: the menu is replaced as the queue
/// changes, so rebuilding the icon each time would leave a trail of icons behind
/// it. A platform that has no notification area is not an error — the window runs,
/// the tray says so in the log, and the queue behaves the same.
fn build_tray(app: &mut tauri::App) {
    let handle = app.handle().clone();
    let app = &handle;
    let menu = match tauri::menu::MenuBuilder::new(app).build() {
        Ok(menu) => menu,
        Err(e) => {
            eprintln!("the notification area is unavailable: {e}");
            return;
        }
    };
    let icon = match app.default_window_icon().cloned() {
        Some(icon) => icon,
        None => {
            eprintln!("no application icon is bundled, so there is no notification-area icon");
            return;
        }
    };
    let pressed = {
        let pressed = app.state::<SharedState>().inner().clone();
        let shared = pressed.clone();
        move |app: &tauri::AppHandle, event: tauri::menu::MenuEvent| {
            tray_action(app, event.id().as_ref());
            let _ = &shared;
        }
    };
    match tauri::tray::TrayIconBuilder::with_id("vise-tray")
        .icon(icon)
        .tooltip("Vise Transcoder")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(pressed)
        .build(app)
    {
        Ok(tray) => {
            let _ = TRAY.set(Mutex::new(Some(tray)));
        }
        Err(e) => eprintln!("the notification-area icon could not be built: {e}"),
    }
}

/// The directory this executable is in, which is where a portable install keeps
/// its state.
fn executable_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|executable| executable.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Look for a newer release, and record what was found.
///
/// Nothing is downloaded and nothing is installed by this build: the availability,
/// the version, and the feed it came from are recorded so that the tray and the
/// Updates preferences can show them, and the decision is the user's.
fn check_for_releases(state: &SharedState) {
    let (feed, channel, installed, policy) = {
        let Ok(guard) = state.lock() else {
            return;
        };
        let updates = &guard.preferences.settings().updates;
        (
            updates.feed_url.trim().to_string(),
            updates.channel.id().to_string(),
            vise_core::VISE_VERSION.to_string(),
            notify_policy(&guard),
        )
    };
    let outcome = system::updates::check(&feed, &channel, &installed);
    let version = outcome.version().unwrap_or_default();
    let available = outcome.available();
    let notification = if available {
        system::notify::decide(&system::notify::Event::UpdateAvailable { version }, policy)
    } else {
        None
    };
    let mut summary = None;
    if let Ok(mut guard) = state.lock() {
        guard
            .integration
            .observe_update(outcome, preferences::updates::today());
        // The answer is copied into the preferences as well as kept by the
        // integration, because the Updates tab is built by the preferences and
        // nothing else: one owner for what the tab shows.
        let answer = guard.integration.updates().clone();
        guard.preferences.observe_update_check(answer);
        summary = Some(guard.integration.updates().summary.clone());
    }
    if let Some(summary) = summary {
        push_log(state, format!("release check: {summary}"));
    }
    if let Some(notification) = notification {
        carry_out(state, system::Effect::Notify(notification));
    }
    refresh_system(state);
}

/// Leave every waiting job where it is, and say why.
fn pause_queue(state: &SharedState) {
    if let Ok(mut guard) = state.lock() {
        sync_queue(&mut guard);
        let jobs = guard.session.jobs().to_vec();
        let waiting = guard.queue.pending(&jobs);
        for id in &waiting {
            let _ = guard.session.update_status(
                *id,
                vise_core::JobStatus {
                    state: JobState::Paused,
                    progress: vise_core::Progress::new(0.0),
                    message: Some("paused from the notification area".into()),
                },
            );
        }
        guard.queue_pause = Some("the queue was paused from the notification area".to_string());
        guard
            .activity
            .record(format!("paused {waiting:?} from the notification area"));
    }
    persist_queue(state);
    refresh_system(state);
}

/// Start the queue again after a pause.
///
/// The pause reason goes with it: the thing that was in the way may still be in the
/// way, and the next job asks the question again rather than assuming the answer
/// has changed.
fn resume_queue(state: &SharedState) {
    if let Ok(mut guard) = state.lock() {
        sync_queue(&mut guard);
        let jobs = guard.session.jobs().to_vec();
        // Everything that is waiting *or* paused, in the queue's own order: a job
        // the pause put to sleep is not in the pending list, and a resume that only
        // looked at the pending list would leave exactly those jobs behind.
        let waiting: Vec<JobId> = guard
            .queue
            .run_order(&jobs)
            .into_iter()
            .filter(|job| matches!(job.status.state, JobState::Queued | JobState::Paused))
            .filter(|job| job.status.state != JobState::Running)
            .map(|job| job.id)
            .collect();
        for id in &waiting {
            let _ = guard.session.update_status(
                *id,
                vise_core::JobStatus {
                    state: JobState::Queued,
                    progress: vise_core::Progress::new(0.0),
                    message: Some("queued again".into()),
                },
            );
        }
        guard.queue_pause = None;
        guard.activity.record("resumed the queue");
    }
    persist_queue(state);
    refresh_system(state);
}

/// Ask the worker to stop whatever it is encoding.
fn cancel_running(state: &SharedState) {
    let client = match worker_client(state) {
        Ok(Some(client)) => client,
        _ => {
            push_log(state, "cancelling an encode requires the worker process");
            return;
        }
    };
    let running = {
        let Ok(mut guard) = state.lock() else {
            return;
        };
        sync_queue(&mut guard);
        guard.queue.active(guard.session.jobs())
    };
    for id in running {
        match client.stop_encode() {
            Ok(result) if result.was_successful => set_job_state(
                state,
                id,
                JobState::Cancelled,
                current_progress(state, id),
                Some("cancelled".into()),
            ),
            Ok(result) => record_problem(
                state,
                "worker",
                format!("cancelling job {id}"),
                result.error.clone(),
            ),
            Err(e) => record_problem(
                state,
                "worker",
                format!("cancelling job {id}"),
                e.to_string(),
            ),
        }
    }
    take_confirmation(state, AlertAction::CancelQueue);
    refresh_system(state);
}

/// Carry out one press of a tray item.
///
/// Every press is answered by the same code the toolbar button uses, so a tray item
/// and a window control cannot mean two different things. The window is told about
/// the surfaces it is asked to open as an event, because a tray item is pressed by
/// somebody who is not looking at the page.
fn tray_action(app: &AppHandle, id: &str) {
    let Some(action) = TrayAction::parse(id) else {
        return;
    };
    let state: SharedState = Arc::clone(app.state::<SharedState>().inner());
    match action {
        TrayAction::ShowWindow => {
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                let _ = window.show();
                let _ = window.unminimize();
                let _ = window.set_focus();
            }
        }
        TrayAction::OpenQueue => {
            open_surface(app, &state, windows::queue::SURFACE);
        }
        TrayAction::PauseQueue => pause_queue(&state),
        TrayAction::ResumeQueue => resume_queue(&state),
        TrayAction::CancelCurrent => {
            if require_confirmation(&state, AlertAction::CancelQueue).is_ok() {
                cancel_running(&state);
            }
        }
        TrayAction::PreventSleep => {
            let wanted = {
                let Ok(guard) = state.lock() else {
                    return;
                };
                !guard.preferences.settings().advanced.prevent_sleep
            };
            if let Ok(mut guard) = state.lock() {
                let changed = guard.preferences.set(
                    preferences::advanced::PREVENT_SLEEP,
                    &serde_json::Value::Bool(wanted),
                );
                match changed {
                    Ok(()) => guard
                        .activity
                        .record("the notification area changed the sleep preference"),
                    Err(problem) => guard.activity.record(problem),
                };
            }
            refresh_system(&state);
        }
        TrayAction::CheckForUpdates => {
            let shared = state.clone();
            thread::spawn(move || check_for_releases(&shared));
        }
        TrayAction::About => open_surface(app, &state, windows::dialogs::ABOUT_SURFACE),
        TrayAction::Quit => {
            // The queue is written before the window goes: quitting from the
            // notification area is not a reason to lose a queued job.
            persist_queue(&state);
            app.exit(0);
        }
    }
}

/// Open one of the window's own surfaces and tell the page about it.
///
/// The state is changed here and the page is told, rather than the page being asked
/// to open it: a tray item is pressed by somebody who cannot see the page, and the
/// window is the one that knows what is open.
fn open_surface(app: &AppHandle, state: &SharedState, id: &str) {
    let opened = {
        let Ok(mut guard) = state.lock() else {
            return;
        };
        guard.surfaces.open(id).is_ok()
    };
    if !opened {
        return;
    }
    if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
    }
    let _ = app.emit(
        TRAY_EVENT,
        &TrayRequest {
            surface: id.to_string(),
        },
    );
    refresh_system(state);
}

/// What the tray asked the page to open.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrayRequest {
    /// The surface to open.
    pub surface: String,
}
/// Build the backend the application will use.
///
/// Isolation is on unless a portable settings file switches it off, or the
/// Advanced preferences ask for it to be off, and the worker executable is looked
/// for next to the application binary first so a packaged build uses its own
/// copy.
///
/// The preference is consulted here rather than at the one place the backend is
/// created, because this function is what creates it and a check that lives
/// elsewhere would be a check that can be forgotten.
fn build_backend(isolate: bool) -> Backend {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    let settings = match vise_ipc::portable::load_for(&executable_dir) {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("portable settings ignored: {e}");
            PortableSettings::default()
        }
    };
    if !settings.should_isolate() {
        let reason = settings.isolation_disabled_reason();
        eprintln!("{reason}");
        return Backend::InProcess;
    }
    if !isolate {
        eprintln!("process isolation is switched off in the Advanced preferences");
        return Backend::InProcess;
    }

    let name = if cfg!(windows) {
        "vise-worker.exe"
    } else {
        "vise-worker"
    };
    let candidates = [
        executable_dir.join(name),
        PathBuf::from("target/debug").join(name),
        PathBuf::from("target/release").join(name),
    ];
    let Some(executable) = candidates.into_iter().find(|path| path.is_file()) else {
        eprintln!(
            "the worker executable '{name}' was not found next to the application; \
             process isolation is unavailable"
        );
        return Backend::InProcess;
    };

    let mut supervisor =
        WorkerSupervisor::new(WorkerConfig::new(executable).with_portable_settings(&settings));
    match supervisor.start() {
        Ok(connection) => {
            eprintln!(
                "worker process {} listening on 127.0.0.1:{}",
                connection.pid, connection.port
            );
            Backend::Worker(Box::new(supervisor))
        }
        Err(e) => {
            eprintln!("worker process could not be started: {e}");
            Backend::InProcess
        }
    }
}

/// Whether a worker is available for the next operation.
fn worker_client(state: &SharedState) -> Result<Option<WorkerClient>, String> {
    let mut guard = state
        .lock()
        .map_err(|_| "desktop state is unavailable".to_string())?;
    if guard.backend.is_none() {
        let isolate = guard.preferences.settings().advanced.process_isolation;
        guard.backend = Some(build_backend(isolate));
    }
    Ok(guard.backend.as_mut().and_then(Backend::client))
}

/// Drop the running backend so the next operation builds it again.
///
/// Called after a preference that decides which kind of backend there is. The
/// worker that is running is not stopped here: it is asked to stop when the
/// window closes, and a preference that is changed between two encodes should not
/// cost the queue an encode to take effect on.
fn rebuild_backend_after_preference(state: &SharedState, why: &str) {
    let dropped = {
        let Ok(mut guard) = state.lock() else {
            return;
        };
        guard.backend.take().is_some()
    };
    if dropped {
        push_log(
            state,
            format!("the worker process will be rebuilt on the next operation: {why}"),
        );
    }
}

// ---------------------------------------------------------------------------
// Source surface
// ---------------------------------------------------------------------------

/// Scan options as the interface layer sends them.
///
/// The window owns validation: an unusable option is refused with a reason
/// instead of being forwarded to the worker and failing there.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ScanOptions {
    /// Preview images to extract per title. Zero asks for none, which is what
    /// the window wants until the preview surface is in use.
    preview_count: Option<u32>,
    /// Shortest acceptable title duration, as `h:mm:ss`.
    min_duration: Option<String>,
    /// Longest acceptable title duration, as `h:mm:ss`.
    max_duration: Option<String>,
    /// File names skipped during the scan.
    file_exclusions: Vec<String>,
}

impl ScanOptions {
    /// Build the worker command for `paths`.
    ///
    /// The preferences fill in what the page did not send, rather than being
    /// kept beside it: a still count and a duplicate-title switch are answers to
    /// one question each, and two copies of an answer are two chances to be
    /// wrong.
    fn to_command(
        &self,
        paths: Vec<String>,
        settings: &PreferencesSettings,
        log_folder: &Path,
    ) -> Result<ScanCommand, String> {
        Ok(ScanCommand {
            initialise_command: InitCommand {
                keep_duplicate_titles: settings.general.keep_duplicate_titles,
                ..settings.to_worker_init_in(log_folder)
            },
            paths,
            // The preview window owns how many stills a scan extracts once the
            // user has changed it, and the preference is the answer behind that.
            // The bound is the field's own, so the clamp and the picker cannot
            // disagree about what the largest allowed number is.
            preview_count: self
                .preview_count
                .unwrap_or(settings.general.preview_count)
                .min(preferences::general::MAX_PREVIEW_FRAMES),
            min_duration: parse_duration(self.min_duration.as_deref(), "shortest")?,
            max_duration: parse_duration(self.max_duration.as_deref(), "longest")?,
            file_exclusion_list: self
                .file_exclusions
                .iter()
                .map(|name| name.trim().to_string())
                .filter(|name| !name.is_empty())
                .collect(),
            hw_decode: settings.hardware.decode_preference(),
            ..ScanCommand::default()
        })
    }
}

/// Parse a duration option, treating an empty value as "no limit".
fn parse_duration(text: Option<&str>, label: &str) -> Result<TimeSpan, String> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(TimeSpan::ZERO);
    };
    TimeSpan::parse(text)
        .map_err(|e| format!("The {label} title duration '{text}' is not valid: {e}"))
}

/// Progress of the current scan, as the interface polls it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ScanStatusViewModel {
    phase: ScanPhase,
    phase_name: String,
    running: bool,
    progress: Option<ScanProgressViewModel>,
    title_count: usize,
    problem: String,
}

impl ScanStatusViewModel {
    fn of(flow: &ScanFlow) -> Self {
        let view = flow.view_model();
        Self {
            phase: flow.phase(),
            phase_name: flow.phase().as_str().to_string(),
            running: flow.is_running(),
            progress: view.progress,
            title_count: view.titles.len(),
            problem: view.problem,
        }
    }
}

/// Validate and load a source the user picked.
#[tauri::command]
fn select_source(
    state: State<'_, SharedState>,
    path: String,
) -> Result<SelectionViewModel, String> {
    let source = sources::resolve_source(&path).map_err(|e| e.message().to_string())?;
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.recents_stamp = sources::next_stamp(guard.recents_stamp);
        let stamp = guard.recents_stamp;
        guard.recents.record(&source, stamp);
        guard.flow.set_source(Some(source));
        // A newly loaded source starts in the range the General preference
        // names, so a choice made once is the choice every new title gets.
        let mode = guard
            .preferences
            .settings()
            .general
            .default_range_mode
            .to_range_mode();
        guard.flow.set_default_range_mode(mode);
        let _ = guard.recents.save(&guard.settings_path);
    }
    if scans_on_open(&state) {
        // Started outside the lock: it is a worker round trip, and holding the
        // lock across one would freeze every other control on the window.
        let _ = begin_scan_with(&state, None);
    }
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.flow.view_model())
}

/// Check paths that arrived from a drag-and-drop gesture or a paste.
///
/// Every path is answered with a reason, so a gesture that mixes a folder and
/// an unreadable file explains itself instead of silently doing half the work.
#[tauri::command]
fn inspect_sources(paths: Vec<String>) -> Result<SourceDropReport, String> {
    Ok(sources::inspect_paths(&paths))
}

/// Forget the loaded source.
#[tauri::command]
fn clear_source(state: State<'_, SharedState>) -> Result<SelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.flow.set_source(None);
    Ok(guard.flow.view_model())
}

/// Sources opened recently, newest first.
#[tauri::command]
fn recent_sources(state: State<'_, SharedState>) -> Result<Vec<SourceViewModel>, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let before = guard.recents.len();
    let views = guard.recents.view_models();
    if guard.recents.len() != before {
        let _ = guard.recents.save(&guard.settings_path);
    }
    Ok(views)
}

/// Drop one recent source.
#[tauri::command]
fn forget_recent_source(state: State<'_, SharedState>, path: String) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.recents.forget(&path);
    guard.recents.save(&guard.settings_path)
}

/// Forget every recent source.
#[tauri::command]
fn clear_recent_sources(state: State<'_, SharedState>) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.recents.clear();
    guard.recents.save(&guard.settings_path)
}

/// The whole source surface, as the interface renders it.
#[tauri::command]
fn source_selection(state: State<'_, SharedState>) -> Result<SelectionViewModel, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.flow.view_model())
}

/// Scan a source with the preferences as they stand.
///
/// Split out from the command so the same scan can be started by the General
/// preference "scan when a source is opened" without going back through the
/// command layer, and so the rule about which preferences reach the worker is in
/// one place.
fn begin_scan_with(
    state: &SharedState,
    options: Option<ScanOptions>,
) -> Result<ScanStatusViewModel, String> {
    let worker = worker_client(state)?;
    let mut guard = state
        .lock()
        .map_err(|_| "the window state is unavailable".to_string())?;
    let Some(source) = guard.flow.source().cloned() else {
        return Err("Choose a file or folder to scan.".to_string());
    };
    let mut options = options.unwrap_or_default();
    // The preview window owns how many stills a scan extracts once it has been
    // changed, and the General preference is where that count comes from to
    // begin with.
    if options.preview_count.is_none() {
        options.preview_count = Some(guard.preview.still_count());
    }
    let settings = guard.preferences.settings().clone();
    let log_folder = guard.storage.logs.file.clone();
    let command = options.to_command(vec![source.path.clone()], &settings, &log_folder)?;
    match worker {
        Some(client) => {
            let started = client.start_scan(&command).map_err(|e| e.to_string())?;
            if !started.was_successful {
                guard.flow.fail(started.error);
                return Err(ScanStatusViewModel::of(&guard.flow).problem);
            }
            guard.flow.begin();
        }
        None => {
            // Process isolation is switched off, either by a portable settings
            // file or by the Advanced preferences; the same scan runs here so
            // the window behaves the same either way.
            match scan_in_process(std::slice::from_ref(&source)) {
                Ok(titles) => guard.flow.accept_scan(titles),
                Err(e) => guard.flow.fail(e),
            }
        }
    }
    Ok(ScanStatusViewModel::of(&guard.flow))
}

/// Start scanning the loaded source.
///
/// The command returns as soon as the worker accepted the work, and the
/// interface polls [`scan_status`] from there, so a long scan never blocks the
/// window.
#[tauri::command]
fn begin_scan(
    state: State<'_, SharedState>,
    options: Option<ScanOptions>,
) -> Result<ScanStatusViewModel, String> {
    begin_scan_with(&state, options)
}

/// Whether the loaded source should be scanned the moment it is opened.
///
/// Read from the General preferences rather than held here, so a preference and
/// the decision it drives cannot be two copies of one answer.
fn scans_on_open(state: &SharedState) -> bool {
    state
        .lock()
        .map(|guard| guard.preferences.settings().scans_on_open())
        .unwrap_or(false)
}

/// Poll the running scan once.
#[tauri::command]
fn scan_status(state: State<'_, SharedState>) -> Result<ScanStatusViewModel, String> {
    let worker = worker_client(&state)?;
    // The worker round trip happens before the state is locked, so a slow or
    // hung worker cannot freeze the rest of the window.
    let polled = match &worker {
        Some(client) => {
            let report = client.poll_scan_progress().map_err(|e| e.to_string())?;
            let finished = matches!(report.state, WorkState::ScanDone | WorkState::WorkDone);
            let successful = report
                .work_done
                .as_ref()
                .map(|done| done.was_successful)
                .unwrap_or(true);
            let titles = if finished && successful {
                Some(client.get_titles().map_err(|e| e.to_string())?)
            } else {
                None
            };
            Some((report, titles))
        }
        None => None,
    };
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if let Some((report, titles)) = polled {
        guard.flow.apply_progress(&report);
        if let Some(titles) = titles {
            guard.flow.apply_titles(titles);
        }
    }
    Ok(ScanStatusViewModel::of(&guard.flow))
}

/// Choose a title, which replaces the track and range selection with that
/// title's own.
#[tauri::command]
fn choose_title(state: State<'_, SharedState>, index: u32) -> Result<SelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.flow.select_title(index)?;
    Ok(guard.flow.view_model())
}

/// Range choice as the interface layer sends it.
///
/// The mode arrives as text and is validated here, so an unknown mode is
/// refused with a readable reason rather than a decoding failure. An empty mode
/// keeps the mode already in force, which lets the point pickers work without
/// the page having to track the mode itself.
#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct RangeChoice {
    mode: String,
    start_seconds: Option<f64>,
    end_seconds: Option<f64>,
    angle: Option<u32>,
}

/// Apply a range and angle choice for the chosen title.
#[tauri::command]
fn choose_range(
    state: State<'_, SharedState>,
    choice: RangeChoice,
) -> Result<SelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let current_mode = guard
        .flow
        .view_model()
        .range
        .map(|range| range.mode)
        .unwrap_or_default();
    let mut selection = RangeSelection {
        mode: current_mode,
        start_seconds: choice.start_seconds,
        end_seconds: choice.end_seconds,
        angle: choice.angle,
    };
    if !choice.mode.trim().is_empty() {
        selection = selection.with_mode_name(&choice.mode)?;
    }
    guard.flow.set_range(selection)?;
    Ok(guard.flow.view_model())
}

/// Apply the track choice for the chosen title.
#[tauri::command]
fn choose_tracks(
    state: State<'_, SharedState>,
    input: TrackSelectionInput,
) -> Result<SelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.flow.set_tracks(input.clone())?;
    // The tab set keeps the same choice, so the audio and subtitle tabs start
    // with the tracks the source area has already chosen.
    guard.tabs.choose_tracks(&input.audio, &input.subtitle);
    Ok(guard.flow.view_model())
}

/// The preview window, as the page renders it.
///
/// The view is built from a locked state rather than by calling the command
/// again, so a command that already holds the lock cannot ask for it twice.
fn preview_view(guard: &mut DesktopState) -> PreviewViewModel {
    point_preview_at_title(guard);
    let selection = guard.flow.view_model();
    let source_label = guard
        .flow
        .source()
        .map(|source| source.name.clone())
        .unwrap_or_default();
    let title_label = selection
        .titles
        .iter()
        .find(|title| title.index == selection.selected_title_index)
        .map(|title| title.label.clone())
        .unwrap_or_default();
    guard
        .preview
        .view(guard.flow.source().is_some(), &source_label, &title_label)
}

/// The preview window, as the page renders it.
#[tauri::command]
fn preview_window(state: State<'_, SharedState>) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    Ok(preview_view(&mut guard))
}

/// Keep the preview window pointed at whatever title the source area has
/// chosen, so the stills on screen and the crop rectangle always describe the
/// title the rest of the window is working on.
fn point_preview_at_title(guard: &mut DesktopState) {
    let title = guard.flow.current_title().cloned();
    let wanted = match title {
        Some(title) => (title.duration_seconds, (title.width, title.height)),
        None => (0.0, (0, 0)),
    };
    if guard.preview.describes() != wanted {
        guard.preview.use_title(wanted.0, wanted.1);
    }
}

/// Ask the worker for the still the preview window is showing.
#[tauri::command]
fn fetch_preview_still(state: State<'_, SharedState>) -> Result<PreviewViewModel, String> {
    let (command, number) = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        preview_view(&mut guard);
        let path = guard
            .flow
            .source()
            .map(|source| source.path.clone())
            .ok_or_else(|| "Choose a file or folder to scan first.".to_string())?;
        let title_index = guard
            .flow
            .current_title()
            .map(|title| title.index)
            .ok_or_else(|| "Choose a title before asking for a still.".to_string())?;
        let number = guard.preview.still_number();
        (guard.preview.command(&path, title_index), number)
    };
    let worker = worker_client(&state)?;
    let Some(client) = worker else {
        return Err("preview stills require the worker process".to_string());
    };
    let result = client.get_preview(&command).map_err(|e| e.to_string())?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if !result.was_successful {
        let reason = if result.error.trim().is_empty() {
            "the worker produced no image for that point in the title".to_string()
        } else {
            result.error.clone()
        };
        guard
            .errors
            .record("worker", "extracting a preview still", &reason);
        guard.preview.fail_image(&reason);
    } else if result.image_base64.is_empty() {
        guard
            .preview
            .fail_image("the worker returned no image for that point in the title");
    } else {
        guard.preview.accept_image(StillImage {
            number,
            byte_length: result.image_bytes,
            data_base64: result.image_base64,
        });
    }
    Ok(preview_view(&mut guard))
}

/// Choose how many stills the next scan extracts.
#[tauri::command]
fn set_preview_still_count(
    state: State<'_, SharedState>,
    count: u32,
) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preview.set_still_count(count)?;
    Ok(preview_view(&mut guard))
}

/// Move along the still list.
#[tauri::command]
fn select_preview_still(
    state: State<'_, SharedState>,
    number: Option<u32>,
    delta: Option<i32>,
    position: Option<StillPosition>,
) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    preview_view(&mut guard);
    if let Some(position) = position {
        guard.preview.scrub(position)?;
    } else if let Some(delta) = delta {
        guard.preview.step(delta);
    } else if let Some(number) = number {
        guard.preview.select(number)?;
    } else {
        return Err("Ask for a still by number, by step, or by position.".to_string());
    }
    Ok(preview_view(&mut guard))
}

/// Move the crop rectangle drawn over the frame.
#[tauri::command]
fn set_preview_crop(
    state: State<'_, SharedState>,
    crop: CropOverlay,
) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preview.set_overlay(crop)?;
    Ok(preview_view(&mut guard))
}

/// Put the rectangle drawn over the frame into the dimensions tab.
///
/// The preview window and the dimensions tab are the same rectangle: the tab's
/// crop fields are written from the overlay, so what the user drew and what the
/// encoder is asked for cannot drift apart.
#[tauri::command]
fn apply_preview_crop(state: State<'_, SharedState>) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let Some(rect) = guard.preview.crop_rect()? else {
        guard.tabs.dimensions.crop_mode = "none".to_string();
        guard
            .activity
            .record("preview crop cleared; the whole frame is kept");
        return Ok(guard.tabs.view(&facts));
    };
    guard.tabs.dimensions.crop_mode = "manual".to_string();
    guard.tabs.dimensions.crop_x = tabs::common::NumField::of(rect.0);
    guard.tabs.dimensions.crop_y = tabs::common::NumField::of(rect.1);
    guard.tabs.dimensions.crop_width = tabs::common::NumField::of(rect.2);
    guard.tabs.dimensions.crop_height = tabs::common::NumField::of(rect.3);
    guard.activity.record(format!(
        "preview crop applied: {}, {} · {} by {}",
        rect.0, rect.1, rect.2, rect.3
    ));
    Ok(guard.tabs.view(&facts))
}

/// Forget the crop rectangle and keep the whole frame.
#[tauri::command]
fn reset_preview_crop(state: State<'_, SharedState>) -> Result<PreviewViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preview.reset_overlay();
    Ok(preview_view(&mut guard))
}

// ---------------------------------------------------------------------------
// Configuration tabs
// ---------------------------------------------------------------------------

/// The facts the tabs read for the chosen title.
fn title_facts(guard: &DesktopState) -> TitleFacts {
    TitleFacts::of(guard.flow.current_title())
}

/// Every tab, as the page renders it.
///
/// The tab set is brought in line with the chosen title first, so a title with
/// different tracks never leaves the previous title's per-track rows behind.
#[tauri::command]
fn tab_set(state: State<'_, SharedState>) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    Ok(guard.tabs.view(&facts))
}

/// Apply a change to one tab and hand back every tab.
///
/// Each command does the same three things: read the change, let the window
/// check it, and return the whole set. Returning the whole set is what keeps the
/// page honest — it never has to work out which other tabs the change affected.
macro_rules! tab_command {
    ($name:ident, $input:ty, |$tabs:ident, $change:ident| $body:expr) => {
        #[tauri::command]
        fn $name(state: State<'_, SharedState>, change: $input) -> Result<TabSetViewModel, String> {
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            let facts = title_facts(&guard);
            guard.tabs.sync_with_title(&facts);
            {
                let $tabs = &mut guard.tabs;
                let $change = change;
                $body?;
            }
            Ok(guard.tabs.view(&facts))
        }
    };
}

/// Apply a change to the summary tab.
///
/// The metadata tab owns the passthrough flag, so this switch writes into it
/// rather than keeping a second copy that could drift out of step.
#[tauri::command]
fn set_summary(
    state: State<'_, SharedState>,
    change: SummaryInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let passthrough = change.passthrough_metadata;
    change.apply(&mut guard.tabs.summary)?;
    if passthrough.is_some() {
        let wanted = guard.tabs.summary.passthrough_metadata;
        guard.tabs.metadata.set_passthrough(wanted);
    }
    Ok(guard.tabs.view(&facts))
}

tab_command!(set_dimensions, DimensionsInput, |tabs, change| {
    change.apply(&mut tabs.dimensions)
});

tab_command!(set_filter, FilterInput, |tabs, change| {
    change.apply(&mut tabs.filters)
});

tab_command!(set_video, VideoInput, |tabs, change| {
    change.apply(&mut tabs.video)
});

tab_command!(set_subtitles, SubtitlesInput, |tabs, change| {
    change.apply(&mut tabs.subtitles)
});

tab_command!(set_chapters, ChaptersInput, |tabs, change| {
    change.apply(&mut tabs.chapters)
});

tab_command!(change_marker, MarkerInput, |tabs, change| {
    change.apply(&mut tabs.chapters)
});

// Bringing markers in from a chapter file is a tab change like any other: the
// refusal to accept a file that did not parse travels back as the same error the
// user would see on a bad number, rather than as a silent no-op.
tab_command!(import_chapters, ChapterImportInput, |tabs, change| {
    change.apply(&mut tabs.chapters)
});

/// Apply a change to the metadata tab.
///
/// The tab reads the chosen title's tags, so it needs those facts rather than
/// the generic macro the other tabs use.
#[tauri::command]
fn set_metadata(
    state: State<'_, SharedState>,
    change: MetadataInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    change.apply(&mut guard.tabs.metadata, &facts)?;
    Ok(guard.tabs.view(&facts))
}

/// Apply a change to the audio rules every track starts from.
#[tauri::command]
fn set_audio_defaults(
    state: State<'_, SharedState>,
    change: AudioDefaultsInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let container = guard.tabs.summary.container.clone();
    change.apply(&mut guard.tabs.audio.defaults, &container)?;
    Ok(guard.tabs.view(&facts))
}

/// Apply a change to one audio track.
#[tauri::command]
fn set_audio_track(
    state: State<'_, SharedState>,
    change: AudioTrackInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let container = guard.tabs.summary.container.clone();
    change.apply(&mut guard.tabs.audio, &container)?;
    Ok(guard.tabs.view(&facts))
}

/// Apply a change to one subtitle track.
#[tauri::command]
fn set_subtitle_track(
    state: State<'_, SharedState>,
    change: SubtitleTrackInput,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    change.apply(&mut guard.tabs.subtitles)?;
    Ok(guard.tabs.view(&facts))
}

/// Show one tab.
///
/// The page keeps its own notion of which panel is open, but the window records
/// it so the next launch opens where the user left off.
#[tauri::command]
fn set_active_tab(state: State<'_, SharedState>, tab: String) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if !guard.tabs.is_tab(&tab) {
        return Err(format!("'{}' is not a tab this build offers.", tab.trim()));
    }
    guard.tabs.active = tab.trim().to_ascii_lowercase();
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    Ok(guard.tabs.view(&facts))
}

/// Add an external subtitle file to the output.
///
/// An empty path clears the imported file, which is how the page offers a way out
/// without a second command.
#[tauri::command]
fn import_subtitles(
    state: State<'_, SharedState>,
    path: String,
) -> Result<TabSetViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let trimmed = path.trim();
    if trimmed.is_empty() {
        guard.tabs.subtitles.external_path.clear();
        guard.tabs.subtitles.external_name.clear();
    } else {
        let source = sources::resolve_source(trimmed)
            .map_err(|e| format!("That subtitle file could not be used: {}", e.message()))?;
        if source.kind != sources::SourceKind::File {
            return Err("Choose a subtitle file rather than a folder.".to_string());
        }
        if !subtitle_extension(&source.name) {
            return Err(format!(
                "'{}' does not look like a subtitle file.",
                source.name
            ));
        }
        guard.tabs.subtitles.external_path = source.path.clone();
        guard.tabs.subtitles.external_name = source.name.clone();
    }
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    Ok(guard.tabs.view(&facts))
}

/// Whether a file name looks like a subtitle file this build can read.
fn subtitle_extension(name: &str) -> bool {
    let Some((_, extension)) = name.rsplit_once('.') else {
        return false;
    };
    matches!(
        extension.trim().to_ascii_lowercase().as_str(),
        "srt" | "ass" | "ssa" | "vtt" | "webvtt" | "sub" | "smi" | "txt" | "ttml" | "dfxp" | "mpl2"
    )
}

// ---------------------------------------------------------------------------
// Queue
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Job validation
// ---------------------------------------------------------------------------

/// The facts this window holds about the source of `spec`, when it holds any.
///
/// Facts about a *different* file are worse than no facts, so the answer is
/// only given when the job is against the source that was scanned.
fn source_facts_for(guard: &DesktopState, spec: &JobSpec) -> Option<vise_core::ScanResult> {
    let scanned = guard.flow.source()?.path.trim();
    if scanned.is_empty() || !scanned.eq_ignore_ascii_case(spec.input_path.trim()) {
        return None;
    }
    let facts = title_facts(guard);
    facts.is_chosen().then(|| facts.to_scan_result())
}

/// Judge `spec` against this machine: every problem it has, each one naming the
/// field that carries it.
///
/// The paths are looked up here and handed to the rules, which stay a pure
/// function of the job and those answers. The same call runs in the worker
/// before it commits to an encode, so a job that cannot work is refused here
/// rather than half an hour into somebody else's machine time — and a job that
/// arrives some other way is refused there instead.
fn judge_job(spec: &JobSpec, scan: Option<vise_core::ScanResult>) -> ValidationReport {
    let mut facts = vise_ipc::validation::probe_machine_facts(spec);
    if let Some(scan) = scan {
        facts = facts.and_scan(scan);
    }
    vise_ipc::validation::validate_job(spec, &facts)
}

/// Judge a job and write down what was wrong with it.
fn check_job(state: &SharedState, spec: &JobSpec) -> ValidationReport {
    let report = state
        .lock()
        .ok()
        .map(|guard| judge_job(spec, source_facts_for(&guard, spec)))
        .unwrap_or_default();
    if !report.is_valid() {
        let context = format!("queueing {}", windows::queue::file_name(&spec.output_path));
        for line in report.lines() {
            record_problem(state, "job", context.clone(), line);
        }
    }
    report
}

/// The message a refused job carries.
///
/// Every problem is in it, one per line, because the whole point of reporting
/// them together is that the user can fix them together.
fn refusal(report: &ValidationReport) -> String {
    format!(
        "This job cannot run yet:\n{}\nNothing was queued.",
        report.lines().join("\n")
    )
}

/// Why a waiting job could not be handed to the encoder.
#[derive(Debug)]
enum ClaimStop {
    /// Only this job is refused; the queue carries on with the next one.
    Job { reason: String },
    /// Something in the way stops the queue as well.
    Queue { reason: String },
}

/// Check that a waiting job can run at all, then mark it as running.
///
/// Validation is the first thing that happens, before the job is claimed and
/// before any room is asked for: a job that cannot encode is not a job that ran
/// out of disk, and it fails on its own rather than pausing the queue behind it.
fn prepare_job(state: &SharedState, id: JobId) -> Result<JobSpec, ClaimStop> {
    let spec = state
        .lock()
        .map_err(|_| "desktop state is unavailable".to_string())
        .map_err(|reason| ClaimStop::Queue { reason })?
        .session
        .job(id)
        .map(|job| job.spec.clone())
        .ok_or_else(|| ClaimStop::Queue {
            reason: format!("job {id} not found"),
        })?;

    let report = check_job(state, &spec);
    if !report.is_valid() {
        let reason = report.summary();
        push_log(
            state,
            format!("job {id} was refused before it started: {reason}"),
        );
        return Err(ClaimStop::Job { reason });
    }

    claim_job(state, id).map_err(|reason| ClaimStop::Queue { reason })
}

/// Judge a job without queueing it, for a page that wants to know before it
/// offers the control.
#[tauri::command]
fn validate_job_spec(
    state: State<'_, SharedState>,
    spec: JobSpec,
) -> Result<ValidationReport, String> {
    let report = check_job(state.inner(), &spec);
    Ok(report)
}

#[tauri::command]
fn add_job(state: State<'_, SharedState>, spec: JobSpec) -> Result<JobId, String> {
    let report = check_job(state.inner(), &spec);
    if !report.is_valid() {
        return Err(refusal(&report));
    }
    let id = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        let id = guard.session.add_job(spec);
        guard.queue.push(id);
        guard.activity.record(format!("queued job {id}"));
        id
    };
    // Written and republished here rather than at each command that queues: a queue
    // file that is a moment behind the queue is a queue that comes back without a
    // job the user just added.
    persist_queue(&state);
    refresh_system(&state);
    Ok(id)
}

/// Queue the current source selection.
///
/// The page names the output file and nothing else: the window builds the job
/// from the tab settings it already validated, so a queued job cannot disagree
/// with what the tabs showed, and it cannot be queued against a title the user
/// did not select.
#[tauri::command]
fn queue_selection(state: State<'_, SharedState>, output_path: String) -> Result<JobId, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let selection = guard.flow.view_model();
    if !selection.is_usable() {
        return Err("Scan a source and choose a title before queueing a job.".to_string());
    }
    let output_path = output_path.trim();
    if output_path.is_empty() {
        return Err("Choose where the finished file should be written.".to_string());
    }
    let source = guard
        .flow
        .source()
        .map(|source| source.path.clone())
        .ok_or_else(|| "Choose a file or folder to scan.".to_string())?;
    let title_index = guard
        .flow
        .current_title()
        .map(|title| title.index)
        .ok_or_else(|| "Choose a title before queueing a job.".to_string())?;

    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let tabs = guard.tabs.view(&facts);
    if !tabs.can_queue {
        return Err(format!(
            "Fix these before queueing: {}",
            tabs.problems.join(" ")
        ));
    }
    let hardware =
        preferences::hardware::to_hardware_options(&guard.preferences.settings().hardware);
    let mut spec = guard
        .tabs
        .to_job_spec(&source, output_path, &facts, &hardware)?;
    spec.title_index = Some(selection.selected_title_index);

    let report = judge_job(&spec, source_facts_for(&guard, &spec));
    if !report.is_valid() {
        let context = format!("queueing title {title_index}");
        for line in report.lines() {
            guard.errors.record("job", &context, line);
        }
        drop(guard);
        return Err(refusal(&report));
    }

    let id = guard.session.add_job(spec);
    guard.queue.push(id);
    guard
        .activity
        .record(format!("queued job {id} for title {title_index}"));
    Ok(id)
}

/// The titles the queue selection sheet may offer, with the facts it needs.
fn selection_titles(guard: &DesktopState) -> Vec<(u32, String, String)> {
    guard
        .flow
        .view_model()
        .titles
        .iter()
        .map(|title| (title.index, title.name.clone(), title.facts.clone()))
        .collect()
}

/// The output extension the chosen container implies.
fn output_extension(guard: &DesktopState) -> String {
    guard
        .tabs
        .summary
        .container
        .clone()
        .trim()
        .to_ascii_lowercase()
}

/// The queue selection sheet, as the page renders it.
#[tauri::command]
fn queue_selection_window(
    state: State<'_, SharedState>,
) -> Result<windows::queue::QueueSelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let titles = selection_titles(&guard);
    let extension = output_extension(&guard);
    // An empty sheet starts with the chosen title picked, because that is the
    // one title a user almost always wants.
    if guard.selection.titles.is_empty() {
        let chosen = guard.flow.view_model().selected_title_index;
        if chosen > 0 {
            guard.selection.titles = vec![chosen];
        }
    }
    // An untouched sheet starts in the folder and with the naming pattern the
    // Output preferences hold, so a folder chosen once is the folder every batch
    // starts in. A folder the user has typed into is left alone.
    if guard.selection.output_directory.trim().is_empty() {
        guard.selection.output_directory = guard.preferences.settings().output.folder.clone();
    }
    if guard.selection.name_pattern.trim().is_empty() {
        guard.selection.name_pattern = guard.preferences.settings().output.name_pattern.clone();
    }
    let directory = guard.selection.output_directory.clone();
    let (valid, problem) = check_output_directory(&directory);
    let policy = collision_policy(&guard);
    Ok(windows::queue::selection_view(
        &guard.selection,
        &titles,
        &extension,
        valid,
        &problem,
        policy,
        output_file_exists,
    ))
}

/// Whether something is already written at `path`.
///
/// Asked through a function rather than read inside the planner so the planner
/// can be tested without writing anything, and so the sheet and the queue ask the
/// same question of the same file.
fn output_file_exists(path: &str) -> bool {
    Path::new(path).is_file()
}

/// The one answer the two Output preferences add up to.
///
/// The sheet and the queue both ask here, so the names on the sheet are the names
/// the queue writes even when a collision policy renumbers them.
fn collision_policy(guard: &DesktopState) -> windows::queue::CollisionPolicy {
    preferences::output::policy(&guard.preferences.settings().output)
}

/// Whether a folder can be written to, and why not when it cannot.
fn check_output_directory(directory: &str) -> (bool, String) {
    let directory = directory.trim();
    if directory.is_empty() {
        return (
            false,
            "Choose a folder to write the finished files into.".to_string(),
        );
    }
    let path = PathBuf::from(directory);
    if !path.is_dir() {
        return (false, format!("{directory} is not a folder that exists."));
    }
    (true, String::new())
}

/// Record what the selection sheet is holding.
#[tauri::command]
fn set_queue_selection(
    state: State<'_, SharedState>,
    change: QueueSelectionInput,
) -> Result<windows::queue::QueueSelectionViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let titles = selection_titles(&guard);
    let extension = output_extension(&guard);
    let directory = change.output_directory.trim().to_string();
    let (valid, problem) = check_output_directory(&directory);
    guard.selection.output_directory = directory;
    if !change.name_pattern.trim().is_empty() {
        guard.selection.name_pattern = change.name_pattern.trim().to_string();
    }
    if change.titles.is_empty() {
        guard.selection.titles.clear();
    } else {
        let known: Vec<u32> = titles.iter().map(|(index, _, _)| *index).collect();
        let wanted: Vec<u32> = change
            .titles
            .iter()
            .copied()
            .filter(|index| known.contains(index))
            .collect();
        if wanted.is_empty() {
            return Err("None of those titles is in the scanned source.".to_string());
        }
        guard.selection.titles = wanted;
    }
    for (index, name) in &change.names {
        let trimmed = name.trim();
        if trimmed.is_empty() {
            guard.selection.names.remove(index);
        } else {
            guard
                .selection
                .names
                .insert(index.clone(), trimmed.to_string());
        }
    }
    Ok(windows::queue::selection_view(
        &guard.selection,
        &titles,
        &extension,
        valid,
        &problem,
        collision_policy(&guard),
        output_file_exists,
    ))
}

/// Queue every title the selection sheet has chosen.
///
/// The whole batch is planned and resolved before anything is queued, using the
/// one collision policy the Output preferences settle into, so two titles that
/// would write the same file are numbered, refused, or left out together rather
/// than the second one quietly overwriting the first. The sheet was built by the
/// same call, so the names it showed are the names queued here.
#[tauri::command]
fn queue_titles(state: State<'_, SharedState>) -> Result<Vec<JobId>, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let selection = guard.flow.view_model();
    if !selection.is_usable() {
        return Err("Scan a source and choose a title before queueing.".to_string());
    }
    let source = guard
        .flow
        .source()
        .map(|source| source.path.clone())
        .ok_or_else(|| "Choose a file or folder to scan.".to_string())?;
    let titles = selection_titles(&guard);
    let names: Vec<(u32, String)> = titles
        .iter()
        .map(|(index, name, _)| (*index, name.clone()))
        .collect();
    let extension = output_extension(&guard);
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let tabs = guard.tabs.view(&facts);
    if !tabs.can_queue {
        return Err(format!(
            "Fix these before queueing: {}",
            tabs.problems.join(" ")
        ));
    }
    let policy = collision_policy(&guard);
    let hardware =
        preferences::hardware::to_hardware_options(&guard.preferences.settings().hardware);
    let planned = windows::queue::plan(
        &guard.selection,
        &names,
        &extension,
        policy,
        output_file_exists,
    )?;
    let mut queued = Vec::new();
    let mut refused: Vec<String> = Vec::new();
    for (index, output) in planned {
        let base = guard
            .tabs
            .to_job_spec(&source, &output, &facts, &hardware)?;
        let spec = windows::queue::titled_spec(&base, index, &output);
        // Each title is judged on its own, so one unusable name does not cost
        // the batch the titles that are fine.
        let report = judge_job(&spec, source_facts_for(&guard, &spec));
        if !report.is_valid() {
            refused.push(format!("title {index}: {}", report.summary()));
            continue;
        }
        let id = guard.session.add_job(spec);
        guard.queue.push(id);
        queued.push(id);
    }
    if queued.is_empty() {
        return Err(format!(
            "Nothing from this batch can be queued yet:\n{}",
            refused.join("\n")
        ));
    }
    guard.activity.record(format!(
        "queued {} title(s) from the selection sheet ({})",
        queued.len(),
        windows::queue::collision_summary(policy)
    ));
    for line in refused {
        guard
            .activity
            .record(format!("left out of the batch: {line}"));
    }
    Ok(queued)
}

/// The queue, as the page renders it.
#[tauri::command]
fn queue_window(state: State<'_, SharedState>) -> Result<windows::queue::QueueViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    sync_queue(&mut guard);
    Ok(queue_view(&guard))
}

/// Move a job through the queue's order.
#[tauri::command]
fn move_job(
    state: State<'_, SharedState>,
    id: JobId,
    delta: i32,
) -> Result<windows::queue::QueueViewModel, String> {
    let view = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        sync_queue(&mut guard);
        guard.queue.move_by(id, delta)?;
        guard.activity.record(format!("moved job {id}"));
        windows::queue::queue_view(&guard.queue, guard.session.jobs())
    };
    // The run order is part of what is written down, so a move is a change to the
    // queue on disk as well as to the queue on screen.
    persist_queue(&state);
    refresh_system(&state);
    Ok(view)
}

/// Set what a job is worth in the run order.
#[tauri::command]
fn set_job_priority(
    state: State<'_, SharedState>,
    id: JobId,
    priority: String,
) -> Result<windows::queue::QueueViewModel, String> {
    let view = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        sync_queue(&mut guard);
        let set = guard.queue.set_priority_name(id, &priority)?;
        guard.activity.record(format!(
            "job {id} is now {} priority",
            set.label().to_ascii_lowercase()
        ));
        windows::queue::queue_view(&guard.queue, guard.session.jobs())
    };
    persist_queue(&state);
    refresh_system(&state);
    Ok(view)
}

#[tauri::command]
fn list_jobs(state: State<'_, SharedState>) -> Result<Vec<Job>, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.session.jobs().to_vec())
}

#[tauri::command]
fn remove_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    require_confirmation(&state, AlertAction::RemoveJob)?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.session.remove_job(id).map_err(|e| e.to_string())?;
    guard.queue.forget(id);
    guard.activity.record(format!("removed job {id}"));
    drop(guard);
    take_confirmation(&state, AlertAction::RemoveJob);
    // The queue on disk is the one thing that has to agree with the queue on
    // screen, so every command that changes it writes it and republishes the tray.
    persist_queue(&state);
    refresh_system(&state);
    Ok(())
}

fn set_job_state(
    state: &SharedState,
    id: JobId,
    job_state: JobState,
    progress: Progress,
    message: Option<String>,
) {
    let changed = {
        let Ok(mut guard) = state.lock() else {
            return;
        };
        let was = guard
            .session
            .job(id)
            .map(|job| job.status.state)
            .unwrap_or(job_state);
        let _ = guard.session.update_status(
            id,
            vise_core::JobStatus {
                state: job_state,
                progress,
                message,
            },
        );
        was != job_state
    };
    // A job that has finished is the thing the tray, the notifications, and the
    // saved queue all care about, so it is refreshed here rather than at each of
    // the places a job can end — which are several, and one that was forgotten
    // would be a queue that never says it has finished. Only a change of state is
    // written down: the progress ticks arrive four times a second, and a queue file
    // rewritten four times a second is a queue file nobody can inspect.
    if changed {
        persist_queue(state);
        refresh_system(state);
    }
}

#[tauri::command]
fn run_job(app: AppHandle, state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let spec = match prepare_job(state.inner(), id) {
        Ok(spec) => spec,
        Err(ClaimStop::Job { reason }) => {
            set_job_state(
                state.inner(),
                id,
                JobState::Failed,
                Progress::new(0.0),
                Some(reason.clone()),
            );
            return Err(reason);
        }
        Err(ClaimStop::Queue { reason }) => return Err(reason),
    };
    let shared: SharedState = state.inner().clone();
    let worker = worker_client(&state)?;
    thread::spawn(move || {
        match worker {
            Some(client) => run_through_worker(shared.clone(), client, id, spec),
            None => run_in_process(shared.clone(), id, spec),
        }
        // The queue may have drained with this job, and the When-done preference
        // is acted on the moment the last one finishes rather than the next time
        // something is queued.
        apply_when_done(&app, &shared);
    });
    Ok(())
}

/// Act on the When-done preferences the moment the queue drains.
///
/// Called after every job reaches a finished state. The decision is the pure
/// [`when_done_plan`], so the rule is tested without a window closing itself;
/// only the carrying-out is here.
///
/// A wait is honoured rather than recorded: the window is not closed the instant
/// the encoder stops, which is what makes an answer the user did not mean to give
/// recoverable. The window remembers what it has already decided for this drain,
/// so two jobs finishing together produce one wait and one question rather than
/// two.
fn apply_when_done(app: &AppHandle, state: &SharedState) {
    let plan = {
        let Ok(guard) = state.lock() else {
            return;
        };
        let jobs = guard.session.jobs();
        let pending = jobs
            .iter()
            .filter(|job| !windows::queue::is_finished(job.status.state))
            .count();
        let running = jobs
            .iter()
            .filter(|job| job.status.state == JobState::Running)
            .count();
        let plan = when_done_plan(guard.preferences.settings(), pending, running);
        if plan.is_none() {
            // The queue is not empty, so the next time it drains is a new drain
            // and may ask again.
            guard.when_done_armed.store(false, Ordering::SeqCst);
        }
        plan
    };
    let Some(plan) = plan else {
        return;
    };
    // Two threads can reach a drained queue together when the last two jobs finish
    // at once; asking twice for one drain would be two questions about the same
    // answer.
    if state
        .lock()
        .map(|guard| guard.when_done_armed.swap(true, Ordering::SeqCst))
        .unwrap_or(false)
    {
        return;
    }
    if plan.delay_seconds == 0 {
        carry_out_when_done(app, state, plan);
        return;
    }
    push_log(
        state,
        format!(
            "the queue finished; the when-done preference acts in {} second(s)",
            plan.delay_seconds
        ),
    );
    let app = app.clone();
    let shared = state.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(u64::from(plan.delay_seconds)));
        carry_out_when_done(&app, &shared, plan);
    });
}

/// Put the when-done question to the user.
///
/// Both answers this build can honour end in the same place — the window asks
/// before it closes, through the same countdown every other destructive action
/// uses — so there is one question rather than two that mean the same thing.
///
/// The question is announced as an event rather than waited to be asked for,
/// because the window asked it itself: the page has no control to press and
/// would otherwise sit there looking at a queue that has finished with nothing
/// happening.
///
/// A line is written to the activity log because a window that acts on its own
/// should say so somewhere the user can read it afterwards.
fn carry_out_when_done(app: &AppHandle, state: &SharedState, plan: WhenDonePlan) {
    match plan.action {
        preferences::whendone::WhenDoneAction::CloseWindow
        | preferences::whendone::WhenDoneAction::Ask => {
            push_log(state, "the queue finished; asking before the window closes");
            let Ok(mut guard) = state.lock() else {
                return;
            };
            if guard.surfaces.open(dialogs::ALERT_SURFACE).is_err() {
                return;
            }
            guard.alerts.request(
                AlertAction::CloseWindow,
                "The queue has finished. Close this window?",
                None,
                None,
            );
            let view = guard.alerts.view();
            drop(guard);
            if let Err(e) = app.emit(ALERT_EVENT, &view) {
                eprintln!("the question could not be shown: {e}");
            }
        }
        // The three actions this build cannot do are refused before they are
        // stored, so reaching here with one means a record from a newer build
        // reached the state: it is logged rather than acted on.
        other => push_log(
            state,
            format!(
                "the queue finished; '{}' is not something this build does.",
                other.label()
            ),
        ),
    }
}

/// Mark a queued or paused job as running and hand back its specification.
///
/// The state change happens under the lock and the work happens after it, so two
/// clicks cannot both pick up the same job.
fn claim_job(state: &SharedState, id: JobId) -> Result<JobSpec, String> {
    // The room the file needs is asked for before the job is claimed, because a
    // job that has been marked running and then refused is a queue that looks as
    // though it is doing something.
    {
        let spec = state
            .lock()
            .map_err(|e| e.to_string())?
            .session
            .job(id)
            .map(|job| job.spec.clone())
            .ok_or_else(|| format!("job {id} not found"))?;
        gate_disk_space(state, id, &spec)?;
        run_pre_action(state, id, &spec)?;
    }
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let jobs = guard.session.jobs().to_vec();
    let Some(job) = jobs.iter().find(|job| job.id == id) else {
        return Err(format!("job {id} not found"));
    };
    let spec = job.spec.clone();
    let job_state = job.status.state;
    if !windows::queue::is_runnable(job_state) {
        return Err(format!(
            "job {id} is not runnable from state {:?}",
            job_state
        ));
    }
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Running,
                progress: Progress::new(0.0),
                message: Some("running".into()),
            },
        )
        .map_err(|e| e.to_string())?;
    let fresh = guard.session.jobs().to_vec();
    guard.queue.sync(&fresh);
    guard.activity.record(format!(
        "started job {id} from {}",
        windows::queue::state_label(job_state).to_ascii_lowercase()
    ));
    Ok(spec)
}

/// Run every waiting job, in the queue's run order.
///
/// The encoder takes one job at a time, so the queue runs them one after
/// another rather than starting a thread per job; a job that fails does not
/// stop the ones behind it.
#[tauri::command]
fn start_queue(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<windows::queue::QueueViewModel, String> {
    require_confirmation(&state, AlertAction::StartQueue)?;
    let waiting = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        sync_queue(&mut guard);
        guard
            .queue
            .pending(guard.session.jobs())
            .into_iter()
            .collect::<Vec<_>>()
    };
    if waiting.is_empty() {
        return Err("Nothing in the queue is waiting for the encoder.".to_string());
    }
    let shared: SharedState = state.inner().clone();
    take_confirmation(&shared, AlertAction::StartQueue);
    {
        let mut guard = shared.lock().map_err(|e| e.to_string())?;
        guard
            .activity
            .record(format!("starting the queue with {} job(s)", waiting.len()));
    }
    thread::spawn(move || {
        for id in waiting {
            let spec = match prepare_job(&shared, id) {
                Ok(spec) => spec,
                // A job that cannot encode fails on its own: the queue carries
                // on with the next one rather than costing every job behind it
                // the same refusal.
                Err(ClaimStop::Job { reason }) => {
                    set_job_state(
                        &shared,
                        id,
                        JobState::Failed,
                        Progress::new(0.0),
                        Some(reason),
                    );
                    refresh_system(&shared);
                    continue;
                }
                // Something in the way — no room on the disk, or a pre-encode
                // action that refused — stops the queue rather than costing
                // every job behind this one the same refusal.
                Err(ClaimStop::Queue { reason }) => {
                    record_problem(&shared, "queue", format!("starting job {id}"), &reason);
                    refresh_system(&shared);
                    break;
                }
            };
            // A client per job, because a worker that dies mid-queue is
            // replaced by the supervisor on the next connection.
            let client = worker_client(&shared).ok().flatten();
            match client {
                Some(client) => run_through_worker(shared.clone(), client, id, spec),
                None => run_in_process(shared.clone(), id, spec),
            }
        }
        // The whole queue has run, so the When-done preference applies here as
        // well as after a single job.
        apply_when_done(&app, &shared);
    });
    queue_window(state)
}

/// Forget every job that has finished.
#[tauri::command]
fn clear_finished_jobs(
    state: State<'_, SharedState>,
) -> Result<windows::queue::QueueViewModel, String> {
    require_confirmation(&state, AlertAction::ClearFinished)?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    sync_queue(&mut guard);
    let jobs = guard.session.jobs().to_vec();
    let done = guard.queue.clear_finished(&jobs);
    for id in &done {
        let _ = guard.session.remove_job(*id);
    }
    guard
        .activity
        .record(format!("cleared {} finished job(s)", done.len()));
    let view = queue_view(&guard);
    drop(guard);
    take_confirmation(&state, AlertAction::ClearFinished);
    persist_queue(&state);
    refresh_system(&state);
    Ok(view)
}

/// Empty the queue of everything that is not being encoded.
#[tauri::command]
fn empty_queue(state: State<'_, SharedState>) -> Result<windows::queue::QueueViewModel, String> {
    require_confirmation(&state, AlertAction::ClearQueue)?;
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    sync_queue(&mut guard);
    let jobs = guard.session.jobs().to_vec();
    let running = guard.queue.active(&jobs);
    if !running.is_empty() {
        return Err(format!(
            "{} job(s) are being encoded; cancel them first.",
            running.len()
        ));
    }
    let total = guard.session.jobs().len();
    let ids: Vec<JobId> = guard
        .queue
        .run_order(&jobs)
        .iter()
        .map(|job| job.id)
        .collect();
    for id in ids {
        let _ = guard.session.remove_job(id);
        guard.queue.forget(id);
    }
    guard
        .activity
        .record(format!("emptied the queue of {total} job(s)"));
    let view = queue_view(&guard);
    drop(guard);
    take_confirmation(&state, AlertAction::ClearQueue);
    persist_queue(&state);
    refresh_system(&state);
    Ok(view)
}

/// Ask the worker to stop whatever it is encoding.
#[tauri::command]
fn cancel_all(state: State<'_, SharedState>) -> Result<windows::queue::QueueViewModel, String> {
    let shared: SharedState = state.inner().clone();
    require_confirmation(&shared, AlertAction::CancelQueue)?;
    let worker = worker_client(&state)?;
    let Some(client) = worker else {
        return Err("cancelling an encode requires the worker process".to_string());
    };
    let running = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        sync_queue(&mut guard);
        guard.queue.active(guard.session.jobs())
    };
    for id in &running {
        let result = client.stop_encode().map_err(|e| e.to_string())?;
        if !result.was_successful {
            let reason = result.error.clone();
            record_problem(&state, "worker", format!("cancelling job {id}"), &reason);
            continue;
        }
        set_job_state(
            &state,
            *id,
            JobState::Cancelled,
            current_progress(&state, *id),
            Some("cancelled".into()),
        );
    }
    let view = queue_window(state)?;
    take_confirmation(&shared, AlertAction::CancelQueue);
    Ok(view)
}

/// Ask the worker to stop what it is encoding for one job.
///
/// Cancelling an encode always asks first, even for a single job, because the
/// work already spent is lost along with it.
#[tauri::command]
fn cancel_job_confirmed(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let shared: SharedState = state.inner().clone();
    require_confirmation(&shared, AlertAction::CancelJob)?;
    cancel_one(&shared, id)?;
    take_confirmation(&shared, AlertAction::CancelJob);
    Ok(())
}

/// Drive one job through the worker and mirror its progress into the queue.
fn run_through_worker(shared: SharedState, client: WorkerClient, id: JobId, spec: JobSpec) {
    push_log(&shared, format!("encoding job {}", id));
    let settings = shared
        .lock()
        .map(|guard| {
            (
                guard.preferences.settings().clone(),
                guard.storage.logs.file.clone(),
            )
        })
        .unwrap_or_default();
    let command = EncodeCommand {
        // The same initialise command a scan would send, so a preference that
        // was changed between the last scan and this encode takes effect on it.
        initialise_command: settings.0.to_worker_init_in(&settings.1),
        encode_job: spec.clone(),
    };
    match client.start_encode(&command) {
        Ok(result) if result.was_successful => {}
        Ok(CommandResult { error, .. }) => {
            push_log(&shared, format!("job {id} refused: {error}"));
            set_job_state(
                &shared,
                id,
                JobState::Failed,
                Progress::new(0.0),
                Some(error),
            );
            return;
        }
        Err(e) => {
            // A worker that died mid-job is a job failure, not an application
            // failure; the supervisor replaces it on the next operation.
            push_log(&shared, format!("job {id} lost its worker: {e}"));
            set_job_state(
                &shared,
                id,
                JobState::Failed,
                Progress::new(0.0),
                Some(e.to_string()),
            );
            return;
        }
    }

    loop {
        match client.poll_encode_progress() {
            Ok(report) => {
                if let Some(working) = &report.working {
                    let mut progress = Progress::new(working.progress);
                    progress.eta_ms = working.eta_seconds.map(|seconds| (seconds * 1000.0) as u64);
                    let paused = working.paused;
                    set_job_state(
                        &shared,
                        id,
                        if paused {
                            JobState::Paused
                        } else {
                            JobState::Running
                        },
                        progress,
                        Some(working.stage.clone()),
                    );
                }
                if report.state == vise_ipc::WorkState::WorkDone {
                    let done = report.work_done.unwrap_or_default();
                    set_job_state(
                        &shared,
                        id,
                        if done.was_successful {
                            JobState::Completed
                        } else {
                            JobState::Failed
                        },
                        Progress::new(if done.was_successful { 1.0 } else { 0.0 }),
                        Some(done.message.clone()),
                    );
                    push_log(
                        &shared,
                        if done.was_successful {
                            format!("completed job {id}")
                        } else {
                            format!("job {id} failed: {}", done.message)
                        },
                    );
                    // The post-action runs whether the encode worked or not: a
                    // program that was told to tidy up after a failed encode is
                    // exactly the program that wants to know about it.
                    run_post_action(&shared, id, &spec);
                    persist_queue(&shared);
                    refresh_system(&shared);
                    return;
                }
            }
            Err(e) => {
                set_job_state(
                    &shared,
                    id,
                    JobState::Failed,
                    Progress::new(0.0),
                    Some(e.to_string()),
                );
                push_log(&shared, format!("job {id} lost its worker: {e}"));
                return;
            }
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// Fallback path used only when process isolation is switched off.
fn run_in_process(shared: SharedState, id: JobId, spec: JobSpec) {
    push_log(&shared, format!("encoding job {} in process", id));
    match vise_core::Transcoder::default().transcode(&spec) {
        Ok(result) => {
            set_job_state(
                &shared,
                id,
                JobState::Completed,
                Progress::new(1.0),
                Some(result.output_path.display().to_string()),
            );
            push_log(&shared, format!("completed job {id}"));
            run_post_action(&shared, id, &spec);
        }
        Err(e) => {
            set_job_state(
                &shared,
                id,
                JobState::Failed,
                Progress::new(0.0),
                Some(e.to_string()),
            );
            push_log(&shared, format!("job {id} failed: {e}"));
        }
    }
}

#[tauri::command]
fn pause_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let worker = worker_client(&state)?;
    if let Some(client) = worker {
        let result = client.pause_encode().map_err(|e| e.to_string())?;
        if result.was_successful {
            set_job_state(
                &state,
                id,
                JobState::Paused,
                current_progress(&state, id),
                Some("paused".into()),
            );
            return Ok(());
        }
        return Err(result.error);
    }

    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let progress = current_progress(&state, id);
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Paused,
                progress,
                message: Some(
                    "pause requested; running encoder process control unavailable".into(),
                ),
            },
        )
        .map_err(|e| e.to_string())?;
    guard.activity.record(format!(
        "pause requested for job {id} (process control unavailable)"
    ));
    Ok(())
}

#[tauri::command]
fn resume_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let worker = worker_client(&state)?;
    if let Some(client) = worker {
        let result = client.resume_encode().map_err(|e| e.to_string())?;
        if result.was_successful {
            set_job_state(
                &state,
                id,
                JobState::Running,
                current_progress(&state, id),
                Some("resumed".into()),
            );
            return Ok(());
        }
        return Err(result.error);
    }
    Err("resume requires the worker process".to_string())
}

#[tauri::command]
fn cancel_job(state: State<'_, SharedState>, id: JobId) -> Result<(), String> {
    let shared: SharedState = state.inner().clone();
    cancel_one(&shared, id)
}

/// Stop what the worker is encoding for one job.
fn cancel_one(state: &SharedState, id: JobId) -> Result<(), String> {
    let worker = worker_client(state)?;
    if let Some(client) = worker {
        let result = client.stop_encode().map_err(|e| e.to_string())?;
        if result.was_successful {
            set_job_state(
                state,
                id,
                JobState::Cancelled,
                current_progress(state, id),
                Some("cancelled".into()),
            );
            return Ok(());
        }
        return Err(result.error);
    }

    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let progress = current_progress(state, id);
    guard
        .session
        .update_status(
            id,
            vise_core::JobStatus {
                state: JobState::Cancelled,
                progress,
                message: Some(
                    "cancel requested; running encoder process control unavailable".into(),
                ),
            },
        )
        .map_err(|e| e.to_string())?;
    guard.activity.record(format!(
        "cancel requested for job {id} (process control unavailable)"
    ));
    Ok(())
}

fn current_progress(state: &SharedState, id: JobId) -> Progress {
    state
        .lock()
        .ok()
        .and_then(|guard| guard.session.job(id).map(|job| job.status.progress.clone()))
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Activity window
// ---------------------------------------------------------------------------

/// The activity window, as the page renders it.
///
/// The worker is read by index rather than in full: only the lines this window
/// has not seen are fetched, so following the tail of a long encode costs the
/// same whether it has written ten lines or ten thousand.
#[tauri::command]
fn activity_window(
    state: State<'_, SharedState>,
) -> Result<windows::log::ActivityViewModel, String> {
    let cursor = state
        .lock()
        .map(|guard| guard.activity.cursor())
        .map_err(|e| e.to_string())?;
    let worker = worker_client(&state)?;
    let mut page = None;
    if let Some(client) = worker {
        match client.get_log_messages_from_index(cursor) {
            Ok(reply) => page = Some(reply),
            Err(e) => record_warning(
                &state,
                "worker",
                "reading the activity log",
                format!("the worker's log could not be read this time: {e}"),
            ),
        }
    }
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if let Some(page) = page {
        guard.activity.take_page(page);
    }
    Ok(guard.activity.view())
}

/// Set the level the activity window shows from.
#[tauri::command]
fn set_log_level(
    state: State<'_, SharedState>,
    level: String,
) -> Result<windows::log::ActivityViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.activity.set_min_level_name(&level)?;
    Ok(guard.activity.view())
}

/// Turn the follow switch on or off.
#[tauri::command]
fn set_follow_logs(
    state: State<'_, SharedState>,
    follow: bool,
) -> Result<windows::log::ActivityViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.activity.set_follow(follow);
    Ok(guard.activity.view())
}

/// Forget every line, here and in the worker.
#[tauri::command]
fn clear_logs(state: State<'_, SharedState>) -> Result<windows::log::ActivityViewModel, String> {
    let worker = worker_client(&state)?;
    if let Some(client) = worker {
        match client.reset_logging() {
            Ok(result) if !result.was_successful => record_warning(
                &state,
                "worker",
                "clearing the activity log",
                result.error.clone(),
            ),
            Ok(_) => {}
            Err(e) => record_warning(
                &state,
                "worker",
                "clearing the activity log",
                format!("the worker did not confirm the reset: {e}"),
            ),
        }
    }
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let cleared = guard.activity.clear();
    guard.activity.record(format!(
        "cleared {cleared} line(s) from the activity window"
    ));
    Ok(guard.activity.view())
}

// ---------------------------------------------------------------------------
// Diagnostics and developer tools
// ---------------------------------------------------------------------------

/// The per-encode log files as the worker left them.
///
/// The folder is read here rather than asked of the worker: the worker owns
/// writing them, and the rule about which of them survive is the same pure
/// policy the worker applied, so asking again would be a second opinion rather
/// than a second measurement.
fn encode_log_files(state: &SharedState) -> Vec<vise_ipc::enclog::LogFile> {
    let Some(folder) = state
        .lock()
        .ok()
        .map(|guard| guard.diagnostics.log_folder().clone())
    else {
        return Vec::new();
    };
    vise_ipc::enclog::list_encode_logs(&folder, vise_ipc::enclog::now_seconds())
}

/// Whether the real queue has an encode running.
fn queue_is_running(guard: &DesktopState) -> bool {
    !guard.queue.active(guard.session.jobs()).is_empty()
}

/// The diagnostics surface, as the page renders it.
#[tauri::command]
fn diagnostics_window(state: State<'_, SharedState>) -> Result<DiagnosticsViewModel, String> {
    let files = encode_log_files(&state);
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.diagnostics.view(
        &files,
        &vise_ipc::enclog::RotationPolicy::default(),
        queue_is_running(&guard),
    ))
}

/// Choose the quality range the next sweep would use.
#[tauri::command]
fn set_sweep_range(
    state: State<'_, SharedState>,
    from: u32,
    to: u32,
    step: u32,
) -> Result<DiagnosticsViewModel, String> {
    let range = vise_core::SweepRange::new(from, to, step);
    let files = encode_log_files(&state);
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let base = sweep_base_job(&guard);
    let folder = sweep_output_folder(&guard);
    guard.diagnostics.set_range(range);
    guard.diagnostics.set_base_job(base);
    guard.diagnostics.set_output_folder(folder);
    let running = queue_is_running(&guard);
    Ok(guard.diagnostics.view(
        &files,
        &vise_ipc::enclog::RotationPolicy::default(),
        running,
    ))
}

/// Confirm a range wider than this build will sweep on its own.
#[tauri::command]
fn confirm_sweep_range(
    state: State<'_, SharedState>,
    confirmed: bool,
) -> Result<DiagnosticsViewModel, String> {
    let files = encode_log_files(&state);
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.diagnostics.set_confirmed(confirmed);
    Ok(guard.diagnostics.view(
        &files,
        &vise_ipc::enclog::RotationPolicy::default(),
        queue_is_running(&guard),
    ))
}

/// The job a sweep's runs are copies of: the one that is loaded right now.
///
/// Built from the current tabs exactly as a queued job is, so a sweep measures
/// the settings the user has actually chosen rather than a default set. The
/// destination is inside the sweep's own folder, and each run replaces it with
/// one of its own.
fn sweep_base_job(guard: &DesktopState) -> JobSpec {
    let Some(source) = guard.flow.source().map(|source| source.path.clone()) else {
        return JobSpec::default();
    };
    let facts = title_facts(guard);
    let hardware =
        preferences::hardware::to_hardware_options(&guard.preferences.settings().hardware);
    let stem = Path::new(&source)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.trim().is_empty())
        .unwrap_or("title");
    let extension = output_extension(guard);
    let extension = if extension.is_empty() {
        "mkv".to_string()
    } else {
        extension
    };
    let output = format!(
        "{}\\{stem}.{extension}",
        vise_core::sweep::sweep_folder(&sweep_output_folder(guard))
    );
    guard
        .tabs
        .to_job_spec(&source, &output, &facts, &hardware)
        .unwrap_or_default()
}

/// Where a sweep's runs are written: the chosen output folder, or beside it.
fn sweep_output_folder(guard: &DesktopState) -> String {
    let folder = guard
        .preferences
        .settings()
        .output
        .folder
        .trim()
        .to_string();
    if folder.is_empty() {
        std::env::temp_dir().to_string_lossy().to_string()
    } else {
        folder
    }
}

/// Start a quality sweep.
///
/// The sweep runs on its own thread and its own jobs, and it is refused while
/// the queue has an encode running: two encodes on one machine would be a
/// measurement of contention rather than of quality.
#[tauri::command]
fn start_sweep(state: State<'_, SharedState>) -> Result<DiagnosticsViewModel, String> {
    let worker = worker_client(&state)?;
    let Some(client) = worker else {
        return Err(
            "a quality sweep needs the worker process; switch process isolation back on to run one."
                .to_string(),
        );
    };
    let shared = state.inner().clone();
    {
        let mut guard = shared.lock().map_err(|e| e.to_string())?;
        let base = sweep_base_job(&guard);
        let folder = sweep_output_folder(&guard);
        let running = queue_is_running(&guard);
        guard.diagnostics.set_base_job(base);
        guard.diagnostics.set_output_folder(folder);
        if let Some(reason) = guard.diagnostics.sweep_refusal(running) {
            return Err(reason);
        }
        guard.diagnostics.start_sweep()?;
        let plan_summary = guard.diagnostics.plan().summary();
        guard
            .activity
            .record(format!("started a quality sweep: {plan_summary}"));
    }
    thread::spawn(move || run_sweep(shared, client));
    diagnostics_window(state)
}

/// Stop the sweep from handing out another run, and stop the run in flight.
#[tauri::command]
fn cancel_sweep(state: State<'_, SharedState>) -> Result<DiagnosticsViewModel, String> {
    let worker = worker_client(&state)?;
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        if !guard.diagnostics.cancel_sweep() {
            return Err("there is no quality sweep to cancel.".to_string());
        }
        guard.activity.record("cancelled the quality sweep");
    }
    // A run already in flight is stopped at the worker rather than left to
    // finish: the user asked for the sweep to stop, not for it to stop asking.
    if let Some(client) = worker {
        if let Ok(result) = client.stop_encode() {
            if !result.was_successful {
                push_log(
                    &state,
                    format!("the running sweep encode was not stopped: {}", result.error),
                );
            }
        }
    }
    diagnostics_window(state)
}

/// Drive a sweep's runs, one at a time, through the worker.
///
/// The shape is the queue's own runner with the session left out of it: the
/// same encode command, the same poll, and nothing written back to a job.
fn run_sweep(shared: SharedState, client: WorkerClient) {
    loop {
        let (run, init) = {
            let Ok(mut guard) = shared.lock() else {
                return;
            };
            match guard.diagnostics.next_run() {
                Some(run) => (
                    Some(run),
                    guard
                        .preferences
                        .settings()
                        .to_worker_init_in(&guard.storage.logs.file),
                ),
                None => {
                    let summary = guard.diagnostics.sweep_progress().summary;
                    guard
                        .activity
                        .record(format!("quality sweep finished: {summary}"));
                    return;
                }
            }
        };
        let Some(run) = run else {
            return;
        };
        let command = EncodeCommand {
            initialise_command: init,
            encode_job: run.job.clone(),
        };
        let outcome = match client.start_encode(&command) {
            Ok(result) if result.was_successful => wait_for_sweep_run(&client, run.quality),
            Ok(CommandResult { error, .. }) => {
                push_log(
                    &shared,
                    format!("sweep run q{} refused: {error}", run.quality),
                );
                vise_core::SweepOutcome::failed(run.quality, error)
            }
            Err(e) => {
                push_log(
                    &shared,
                    format!("sweep run q{} lost its worker: {e}", run.quality),
                );
                vise_core::SweepOutcome::failed(run.quality, e.to_string())
            }
        };
        let mut outcome = outcome;
        if let Ok(mut guard) = shared.lock() {
            if guard.diagnostics.sweep_progress().cancelled {
                outcome.cancelled = true;
            }
            guard.diagnostics.finish_run(outcome);
        }
    }
}

/// Poll the worker until one sweep run has finished, and measure what it made.
fn wait_for_sweep_run(client: &WorkerClient, quality: u32) -> vise_core::SweepOutcome {
    let started = Instant::now();
    let mut path: Option<PathBuf> = None;
    loop {
        match client.poll_encode_progress() {
            Ok(report) => {
                if let Some(working) = &report.working {
                    if path.is_none() && !working.output_path.is_empty() {
                        path = Some(PathBuf::from(&working.output_path));
                    }
                }
                if report.state == vise_ipc::WorkState::WorkDone {
                    let done = report.work_done.unwrap_or_default();
                    let seconds = started.elapsed().as_secs_f64();
                    if !done.was_successful {
                        return vise_core::SweepOutcome::failed(quality, done.message);
                    }
                    let bytes = path
                        .as_ref()
                        .and_then(|path| std::fs::metadata(path).ok())
                        .map(|meta| meta.len())
                        .unwrap_or(0);
                    return vise_core::SweepOutcome::succeeded(quality, bytes, seconds);
                }
            }
            Err(e) => return vise_core::SweepOutcome::failed(quality, e.to_string()),
        }
        thread::sleep(Duration::from_millis(250));
    }
}

/// Write one file and report what was written.
fn write_report(
    guard: &mut DesktopState,
    kind: &str,
    folder: &Path,
    name: &str,
    bytes: &[u8],
    summary: String,
) -> ReportViewModel {
    let path = folder.join(name);
    let written = std::fs::create_dir_all(folder).and_then(|()| std::fs::write(&path, bytes));
    let report = match written {
        Ok(()) => {
            guard
                .activity
                .record(format!("wrote {kind} to {}", path.display()));
            ReportViewModel {
                kind: kind.to_string(),
                path: path.to_string_lossy().to_string(),
                summary,
                written: true,
                problem: String::new(),
            }
        }
        Err(e) => {
            let problem = format!("{kind} could not be written ({}): {e}", path.display());
            record_warning_in(guard, "diagnostics", kind, problem.clone());
            ReportViewModel {
                kind: kind.to_string(),
                path: path.to_string_lossy().to_string(),
                summary,
                written: false,
                problem,
            }
        }
    };
    guard.diagnostics.record_report(report.clone());
    report
}

/// Record a warning without taking the lock again.
fn record_warning_in(guard: &mut DesktopState, source: &str, context: &str, detail: String) {
    guard.errors.push_warning(source, context, detail);
}

/// Where exports are written: the diagnostics folder beside the state.
fn export_folder(guard: &DesktopState) -> PathBuf {
    guard.storage.logs.file.clone()
}

/// Export the finished queue as CSV and JSON.
#[tauri::command]
fn export_queue_results(
    state: State<'_, SharedState>,
    format: String,
) -> Result<DiagnosticsViewModel, String> {
    let files = encode_log_files(&state);
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let results =
        system::results::QueueResults::of(guard.session.jobs(), &guard.queue, &|spec: &JobSpec| {
            std::fs::metadata(&spec.output_path)
                .map(|meta| meta.len())
                .unwrap_or(0)
        });
    let wanted = format.trim().to_ascii_lowercase();
    let (extension, bytes, summary): (&str, Vec<u8>, String) = match wanted.as_str() {
        "json" => ("json", results.to_json().into_bytes(), results.summary()),
        "csv" | "" => ("csv", results.to_csv().into_bytes(), results.summary()),
        other => {
            return Err(format!(
                "'{other}' is not an export this build writes; choose csv or json."
            ))
        }
    };
    let folder = export_folder(&guard);
    write_report(
        &mut guard,
        "results",
        &folder,
        &format!("vise-queue-results.{extension}"),
        &bytes,
        summary,
    );
    Ok(guard.diagnostics.view(
        &files,
        &vise_ipc::enclog::RotationPolicy::default(),
        queue_is_running(&guard),
    ))
}

/// Ask the machine what it can do, and record the answer.
///
/// Detection is two subprocess calls, so it is done here rather than on every
/// read of the surface: the page re-asks through this command when the user asks
/// it to.
#[tauri::command]
fn detect_capabilities(state: State<'_, SharedState>) -> Result<DiagnosticsViewModel, String> {
    let report = vise_core::CapabilityReport::detect();
    let files = encode_log_files(&state);
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.diagnostics.observe_capabilities(report.clone());
    guard
        .activity
        .record(format!("capability report: {}", report.summary()));
    Ok(guard.diagnostics.view(
        &files,
        &vise_ipc::enclog::RotationPolicy::default(),
        queue_is_running(&guard),
    ))
}

/// Write the capability report out as text and as JSON.
#[tauri::command]
fn export_capabilities(state: State<'_, SharedState>) -> Result<DiagnosticsViewModel, String> {
    let files = encode_log_files(&state);
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let report = match guard.diagnostics.capabilities() {
        Some(report) => report.clone(),
        None => {
            let report = vise_core::CapabilityReport::detect();
            guard.diagnostics.observe_capabilities(report.clone());
            report
        }
    };
    let folder = export_folder(&guard);
    let summary = report.summary();
    write_report(
        &mut guard,
        "capabilities",
        &folder,
        "vise-capabilities.txt",
        report.to_text().as_bytes(),
        summary,
    );
    Ok(guard.diagnostics.view(
        &files,
        &vise_ipc::enclog::RotationPolicy::default(),
        queue_is_running(&guard),
    ))
}

/// Build the diagnostic bundle and write it out.
///
/// The worker's session token is handed to the redactor as a value, because that
/// is where it appears: inside an error message, not under a key with an obvious
/// name.
#[tauri::command]
fn export_diagnostics(state: State<'_, SharedState>) -> Result<DiagnosticsViewModel, String> {
    let files = encode_log_files(&state);
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let report = match guard.diagnostics.capabilities() {
        Some(report) => report.clone(),
        None => {
            let report = vise_core::CapabilityReport::detect();
            guard.diagnostics.observe_capabilities(report.clone());
            report
        }
    };
    let secrets = worker_token(&state);
    let redactor = system::bundle::Redactor::for_this_machine(secrets);
    let queue =
        system::results::QueueResults::of(guard.session.jobs(), &guard.queue, &|spec: &JobSpec| {
            std::fs::metadata(&spec.output_path)
                .map(|meta| meta.len())
                .unwrap_or(0)
        });
    let input = system::bundle::BundleInput {
        version: vise_core::VISE_VERSION.to_string(),
        settings: serde_json::to_string_pretty(guard.preferences.settings())
            .unwrap_or_else(|_| "{}".to_string()),
        capabilities: report.to_json(),
        capabilities_text: report.to_text(),
        log: guard.activity.copy_text(),
        queue: format!("{}\n{}", queue.summary(), queue.to_csv()),
        extra: Vec::new(),
    };
    let bundle = system::bundle::DiagnosticBundle::build(&input, &redactor);
    let summary = bundle.summary();
    let name = bundle.file_name().to_string();
    let bytes = bundle.to_bytes();
    let folder = export_folder(&guard);
    write_report(&mut guard, "bundle", &folder, &name, &bytes, summary);
    Ok(guard.diagnostics.view(
        &files,
        &vise_ipc::enclog::RotationPolicy::default(),
        queue_is_running(&guard),
    ))
}

/// The worker session token, so it can be kept out of an export.
fn worker_token(state: &SharedState) -> Vec<String> {
    match worker_client(state) {
        Ok(Some(client)) => vec![client.token().to_string()],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Problems, alerts, and the about view
// ---------------------------------------------------------------------------

/// Everything that went wrong, as the page renders it.
#[tauri::command]
fn error_view(state: State<'_, SharedState>) -> Result<ErrorViewModel, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.errors.view())
}

/// The whole problem list as plain text, for the clipboard.
#[tauri::command]
fn error_report(state: State<'_, SharedState>) -> Result<String, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(dialogs::report_text(guard.errors.entries()))
}

/// Forget every recorded problem.
#[tauri::command]
fn clear_errors(state: State<'_, SharedState>) -> Result<ErrorViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.errors.clear();
    Ok(guard.errors.view())
}

/// Ask for confirmation before an action that cannot be undone.
///
/// A destructive action also opens the alert as its own surface, so the
/// question is where the user is looking rather than tucked behind the control
/// they just pressed. The headline arrives from the page when it has something
/// specific to name; otherwise the window writes one, so the question never
/// appears as a bare verb.
#[tauri::command]
fn ask_for_alert(
    state: State<'_, SharedState>,
    action: AlertAction,
    headline: String,
    job_id: Option<u64>,
    preset_id: Option<String>,
) -> Result<dialogs::CountdownAlertViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let destructive = action.is_destructive();
    let headline = alert_headline(action, headline.trim(), job_id, preset_id.as_deref());
    guard.alerts.request(action, headline, job_id, preset_id);
    if destructive {
        guard.surfaces.open(dialogs::ALERT_SURFACE)?;
    }
    Ok(guard.alerts.view())
}

/// The question a confirmation asks.
///
/// An empty headline from the page becomes a sentence that still says what is
/// about to happen, so the alert is never "Are you sure? Confirm."
fn alert_headline(
    action: AlertAction,
    headline: &str,
    job_id: Option<u64>,
    preset_id: Option<&str>,
) -> String {
    if !headline.is_empty() {
        return headline.to_string();
    }
    match action {
        AlertAction::CancelJob | AlertAction::RemoveJob => match job_id {
            Some(id) => format!("{} job {id}?", action.verb()),
            None => format!("{}?", action.verb()),
        },
        AlertAction::DeletePreset => match preset_id {
            Some(id) if !id.trim().is_empty() => format!("{} '{id}'?", action.verb()),
            _ => format!("{}?", action.verb()),
        },
        AlertAction::ResetPreferences => {
            "Put every preference on every tab back to its default?".to_string()
        }
        AlertAction::CloseWindow => "The queue has finished. Close this window?".to_string(),
        _ => format!("{}?", action.verb()),
    }
}

/// Count one second off a pending alert.
#[tauri::command]
fn tick_alert(state: State<'_, SharedState>) -> Result<dialogs::CountdownAlertViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.alerts.tick();
    Ok(guard.alerts.view())
}

/// Confirm a pending action, once its countdown is over.
///
/// The action itself is carried back rather than performed here: only the
/// commands that own an action can carry it out, so a stale or replayed
/// confirmation cannot cancel something the caller never asked to cancel.
#[tauri::command]
fn confirm_alert(state: State<'_, SharedState>) -> Result<Option<dialogs::CountdownAlert>, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let confirmed = guard.alerts.take_confirmed();
    if let Some(alert) = &confirmed {
        guard
            .activity
            .record(format!("confirmed: {}", alert.headline));
        guard.confirmed = Some(alert.action);
    }
    guard.surfaces.close(dialogs::ALERT_SURFACE);
    Ok(confirmed)
}

/// Back out of a pending action.
#[tauri::command]
fn dismiss_alert(
    state: State<'_, SharedState>,
) -> Result<dialogs::CountdownAlertViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let dropped = guard
        .alerts
        .pending()
        .map(|pending| pending.headline.clone());
    guard.confirmed = None;
    if let Some(headline) = dropped {
        guard.activity.record(format!("backed out of: {headline}"));
    }
    guard.alerts.cancel();
    guard.surfaces.close(dialogs::ALERT_SURFACE);
    Ok(guard.alerts.view())
}

/// The about view.
#[tauri::command]
fn about_view(state: State<'_, SharedState>) -> Result<AboutViewModel, String> {
    let engine = state
        .lock()
        .ok()
        .and_then(|guard| guard.backend.as_ref().map(|backend| backend.describe()))
        .unwrap_or_default();
    Ok(dialogs::about(&engine))
}

// ---------------------------------------------------------------------------
// Presets
// ---------------------------------------------------------------------------

/// The stored presets, or none when the file cannot be read.
///
/// A library that will not read is treated as empty rather than as a failure:
/// the window must still open so the problem can be read out of it.
fn stored_presets(path: &Path) -> Vec<vise_core::UserPreset> {
    vise_core::load_user_presets(path).unwrap_or_default()
}

/// The preset manager, as the page renders it.
#[tauri::command]
fn preset_window(
    state: State<'_, SharedState>,
    change: Option<PresetInput>,
) -> Result<windows::presets::PresetView, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    check_presets_against_this_machine(&mut guard);
    let stored = stored_presets(&guard.presets.path);
    let built_in = vise_core::builtin_presets();
    let input = change.unwrap_or_default();
    let names: Vec<String> = stored.iter().map(|preset| preset.name.clone()).collect();
    let found = stored.iter().find(|preset| preset.id == input.id);
    Ok(guard.presets.view(
        &stored,
        &built_in,
        windows::presets::form_view(&input, &names, found),
    ))
}

/// Move the browser to a category, or to one entry inside it.
///
/// The page sends what the user pressed and the window decides what that means:
/// which entries the category holds, which one is open, and what each one will do
/// on this machine. A name the catalog does not file is refused here rather than
/// leaving the strip on a tab with nothing in it.
#[tauri::command]
fn browse_catalog(
    state: State<'_, SharedState>,
    change: windows::presets::CatalogInput,
) -> Result<windows::presets::PresetView, String> {
    {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        if !change.category.trim().is_empty() {
            guard.presets.set_catalog_category(&change.category)?;
        }
        if !change.entry.trim().is_empty() {
            guard.presets.set_catalog_entry(&change.entry)?;
        }
    }
    preset_window(state, None)
}

/// Resolve every preset against what this machine actually has.
///
/// The media tools are asked once, the first time the preset window is opened,
/// and the answer is kept: probing shells out to the encoder, so doing it on
/// every repaint would make the window slow for no new information. The user's
/// own hardware preference is re-read each time, because it can change without
/// the machine changing.
fn check_presets_against_this_machine(guard: &mut DesktopState) {
    let acceleration =
        preferences::hardware::to_hardware_options(&guard.preferences.settings().hardware);
    if guard.presets.is_checked() {
        let known = guard.presets.capabilities.clone();
        guard.presets.observe_hardware(&known, &acceleration);
        return;
    }
    let capabilities = vise_core::HardwareCapabilities::detect();
    guard.presets.observe_hardware(&capabilities, &acceleration);
}

/// Act on a preset: create, edit, copy, delete, or apply.
///
/// Everything is written through `vise_core`'s preset API, so the file on disk
/// is exactly what the core expects to read back on the next launch.
#[tauri::command]
fn apply_preset_action(
    state: State<'_, SharedState>,
    change: PresetInput,
) -> Result<windows::presets::PresetView, String> {
    let action = change.action()?;
    let path = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.presets.path.clone()
    };
    {
        let names: Vec<String> = stored_presets(&path)
            .into_iter()
            .map(|preset| preset.name)
            .collect();
        let blocking: Vec<String> = windows::presets::form_view(&change, &names, None)
            .problems
            .into_iter()
            .filter(|problem| !problem.starts_with("Another preset"))
            .collect();
        for problem in &blocking {
            record_problem(&state, "presets", action.as_str(), problem);
        }
        if !blocking.is_empty() {
            return Err(format!(
                "Fix this before storing the preset: {}",
                blocking.join(" ")
            ));
        }
    }

    // An action that destroys work says so in the log before it happens, so a
    // bug report that starts after the fact still shows what was asked for.
    if action.is_destructive() {
        push_log(
            &state,
            format!(
                "'{}' cannot be undone and asks for confirmation first",
                action.as_str()
            ),
        );
    }

    match action {
        PresetAction::Create | PresetAction::Clone => {
            let spec = preset_from_tabs(&state, &change)?;
            let created = vise_core::create_user_preset(&path, change.name.trim(), spec)
                .map_err(|e| e.to_string())?;
            let category = windows::presets::parse_category(&change.category)?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.set_category(&created.id, category);
            if change.default {
                guard.presets.set_default(&created.id);
            }
            guard
                .activity
                .record(format!("stored the preset '{}'", created.name));
        }
        PresetAction::Edit => {
            let name = change.name.trim().to_string();
            let stored = vise_core::update_user_preset(
                &path,
                change.id.trim(),
                Some(&name),
                Some(preset_from_tabs(&state, &change)?),
            )
            .map_err(|e| e.to_string())?;
            let category = windows::presets::parse_category(&change.category)?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.set_category(&stored.id, category);
            guard
                .activity
                .record(format!("renamed the preset '{}'", stored.name));
        }
        PresetAction::Delete => {
            require_confirmation(&state, AlertAction::DeletePreset)?;
            vise_core::delete_user_preset(&path, change.id.trim()).map_err(|e| e.to_string())?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.forget(change.id.trim());
            guard.activity.record("deleted a preset");
            drop(guard);
            take_confirmation(&state, AlertAction::DeletePreset);
        }
        PresetAction::SetDefault => {
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.set_default(change.id.trim());
            let name = guard
                .presets
                .default_name(&stored_presets(&path), &vise_core::builtin_presets());
            guard
                .activity
                .record(format!("'{name}' now starts every new job"));
        }
        PresetAction::ClearDefault => {
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.clear_default();
            guard
                .activity
                .record("no preset starts every new job any more");
        }
        PresetAction::Apply => {
            let stored = vise_core::load_user_presets(&path).map_err(|e| e.to_string())?;
            let preset = match stored.into_iter().find(|preset| preset.id == change.id) {
                Some(found) => found.preset,
                None => vise_core::builtin_preset(change.id.trim())
                    .map(|preset| preset.preset)
                    .ok_or_else(|| "There is no preset with that name.".to_string())?,
            };
            apply_preset_to_tabs(&state, &preset)?;
            push_log(&state, "applied a preset to the output settings");
        }
        PresetAction::Import => {
            if let Some(problem) = windows::presets::import_path_problem(&change.path) {
                return Err(problem);
            }
            let imported = vise_core::import_user_preset(std::path::Path::new(change.path.trim()))
                .map_err(|e| format!("That file could not be read as a preset: {e}"))?;
            let name = if imported.name.trim().is_empty() {
                return Err(
                    "A preset file needs a name; rename it in the file and try again.".to_string(),
                );
            } else {
                imported.name.trim().to_string()
            };
            let created = vise_core::create_user_preset(&path, &name, imported.preset)
                .map_err(|e| e.to_string())?;
            let category = windows::presets::parse_category(&change.category)?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard.presets.set_category(&created.id, category);
            if change.default {
                guard.presets.set_default(&created.id);
            }
            guard
                .activity
                .record(format!("imported the preset '{}'", created.name));
        }
        PresetAction::Export => {
            if let Some(problem) = windows::presets::export_path_problem(&change.path) {
                return Err(problem);
            }
            let stored = vise_core::load_user_presets(&path).map_err(|e| e.to_string())?;
            let wanted = change.id.trim();
            if wanted.is_empty() {
                return Err("Choose which preset to export.".to_string());
            }
            // A preset that ships with the build is exported as a stored preset
            // with the same name and description, so the file the user gets back
            // is one this window could import again.
            let preset = match stored.into_iter().find(|preset| preset.id == wanted) {
                Some(found) => found,
                None => match vise_core::builtin_preset(wanted) {
                    Some(found) => vise_core::UserPreset {
                        id: found.id.clone(),
                        name: found.name.clone(),
                        description: found.description.clone(),
                        preset: found.preset.clone(),
                    },
                    None => return Err("There is no preset with that name.".to_string()),
                },
            };
            let name = preset.name.clone();
            vise_core::export_user_preset(std::path::Path::new(change.path.trim()), &preset)
                .map_err(|e| format!("That preset could not be written: {e}"))?;
            let mut guard = state.lock().map_err(|e| e.to_string())?;
            guard
                .activity
                .record(format!("exported the preset '{name}'"));
        }
    }

    preset_window(state, None)
}

/// Build the preset the current tab settings describe.
///
/// A preset carries the choices that shape the output rather than the whole job,
/// so it stays meaningful against a different title: container, codecs, the
/// crop and size the user picked, and the quality. Everything else stays on the
/// tabs.
fn preset_from_tabs(
    state: &SharedState,
    change: &PresetInput,
) -> Result<vise_core::PresetSpec, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let (crop, scale) =
        tabs::dimensions::to_job_geometry(&guard.tabs.dimensions, &facts).unwrap_or((None, None));
    Ok(vise_core::PresetSpec {
        container: tabs::compatibility::container_of(&guard.tabs.summary.container),
        format: None,
        video_codec: tabs::compatibility::video_codec_of(&guard.tabs.summary.video_codec),
        video: None,
        audio_codec: tabs::compatibility::audio_codec_of(&guard.tabs.summary.audio_codec),
        audio: None,
        // The encoder, the rate control, and the size it aims at are what a
        // preset is for, so they travel with it rather than being retyped on the
        // video tab. The encoder comes from the hardware settings the tabs hold,
        // because that is where the window read it from.
        // The encoder and the rate control are what a preset is for, so they
        // travel with it rather than being retyped on the video tab.
        encoder: tabs::compatibility::encoder_of(&guard.tabs.summary.video_codec),
        video_encoding: {
            let options = tabs::video_encoding_options(&guard.tabs.video);
            (!options.is_empty()).then_some(options)
        },
        encoding_mode: tabs::compatibility::encoding_mode_of(&guard.tabs.video.rate_control),
        target_size: tabs::compatibility::target_size_bytes_of(
            &guard.tabs.video.rate_control,
            guard
                .tabs
                .video
                .target_size
                .whole("Target size")
                .ok()
                .flatten(),
        ),
        // No tab asks how long a title is, so a preset saved from the tabs
        // carries no cap: the cap belongs to the preset that was designed around
        // a length, not to every preset.
        duration_cap_seconds: None,
        subtitle_mode: None,
        subtitles: None,
        title_index: None,
        title: None,
        crop,
        scale_to: scale,
        quality: guard.tabs.video.quality.whole("Quality").ok().flatten(),
        name: Some(change.name.trim().to_string()),
        description: non_empty(change.description.trim()),
    })
}

/// The name the video tab knows a rate-control mode by.
///
/// The job model carries more modes than the tab offers, so the ones the tab has
/// no word for are not invented: a preset that asks for something the tab cannot
/// show keeps the tab's current rate control rather than being shown a mode that
/// does not exist.
fn rate_control_name(mode: vise_core::EncodingMode) -> &'static str {
    if mode.is_constant_quality() {
        "quality"
    } else if mode.is_average_bitrate() {
        "bitrate"
    } else if mode.is_target_size() {
        "target_size"
    } else if mode.is_lossless() {
        "lossless"
    } else {
        "quality"
    }
}

/// A description, or nothing rather than an empty string.
fn non_empty(text: &str) -> Option<String> {
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Write a preset back into the tab settings.
fn apply_preset_to_tabs(state: &SharedState, preset: &vise_core::PresetSpec) -> Result<(), String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if let Some(container) = preset.container_of() {
        guard.tabs.summary.container = format!("{container:?}").to_ascii_lowercase();
    }
    if let Some(codec) = preset.video_codec_of() {
        guard.tabs.summary.video_codec = format!("{codec:?}").to_ascii_lowercase();
    }
    if let Some(codec) = preset.audio_codec_of() {
        guard.tabs.summary.audio_codec = format!("{codec:?}").to_ascii_lowercase();
    }
    // The encoder the preset was written for decides which profiles, levels,
    // tunes, and speed presets the video tab offers, so the tab is moved to it
    // before any of those are set: a ProRes profile read against an H.264
    // picker would be refused as a value H.264 does not offer. A hardware
    // encoder lands on its own codec, because that is what the tab names — which
    // chip runs it is the hardware settings' answer, not the tab's.
    if let Some(codec) = preset
        .encoder
        .map(|encoder| encoder.codec())
        .or_else(|| preset.video_codec_of())
    {
        guard.tabs.video.encoder = tabs::compatibility::video_codec_id(codec).to_string();
    }
    if let Some(quality) = preset.quality {
        guard.tabs.video.quality = tabs::common::NumField::of(quality);
    }
    // The front-loaded index is an MP4 feature. A preset that names another
    // container decides what the output is, so a switch that describes an MP4
    // index is turned off with it rather than left on to be refused at the end of
    // the form; an MP4 preset leaves the user's own answer alone.
    if preset
        .container_of()
        .map(|container| container != vise_core::ContainerFormat::Mp4)
        .unwrap_or(false)
    {
        guard.tabs.summary.web_optimized = false;
        guard.tabs.summary.ipod_compatible = false;
    }
    // The rate control the preset names wins over the quality it carries: a
    // size-targeted preset also carries a quality value, and showing the tab as
    // a quality encode would be applying something other than what was asked for.
    if let Some(mode) = preset.encoding_mode {
        guard.tabs.video.rate_control = rate_control_name(mode).to_string();
    } else if preset.quality.is_some() {
        guard.tabs.video.rate_control = "quality".to_string();
    }
    // A size-targeted preset carries the size it was designed around, in bytes;
    // the tab asks in megabytes, so it is converted rather than shown raw.
    if let Some(bytes) = preset.target_size {
        guard.tabs.video.target_size = tabs::common::NumField::of((bytes / 1_000_000) as u32);
    }
    // Anything the preset says nothing about goes back to the encoder default
    // rather than keeping whatever the last preset set. A preset is the whole
    // instruction for the encode, so applying it twice is applying the same
    // thing: a ProRes profile left on the tab would otherwise be read by the
    // next preset and refused as a value FFV1 does not offer, or worse, carried
    // into an encode the user never asked for.
    //
    // `default` is what every picker on the video tab carries for "leave it to
    // the encoder", so it is what a field the instruction is silent about has
    // to go back to.
    let encoding = preset.video_encoding.clone().unwrap_or_default();
    let or_default = |value: Option<String>| value.unwrap_or_else(|| "default".to_string());
    guard.tabs.video.preset = or_default(encoding.preset.clone());
    guard.tabs.video.profile = or_default(encoding.profile.clone());
    guard.tabs.video.tune = or_default(encoding.tune.clone());
    guard.tabs.video.level = or_default(encoding.level.clone());
    guard.tabs.video.colour_range = encoding
        .colour_range
        .map(|range| format!("{range:?}").to_ascii_lowercase())
        .unwrap_or_else(|| "default".to_string());
    if let Some((_, _, width, height)) = preset.crop {
        guard.tabs.dimensions.crop_mode = "manual".to_string();
        guard.tabs.dimensions.crop_width = tabs::common::NumField::of(width);
        guard.tabs.dimensions.crop_height = tabs::common::NumField::of(height);
    }
    if let Some((width, height)) = preset.scale_to {
        guard.tabs.dimensions.scale_mode = "exact".to_string();
        guard.tabs.dimensions.scale_width = tabs::common::NumField::of(width);
        guard.tabs.dimensions.scale_height = tabs::common::NumField::of(height);
    }
    let facts = title_facts(&guard);
    guard.tabs.sync_with_title(&facts);
    let problems = guard.tabs.view(&facts).problems;
    drop(guard);
    if !problems.is_empty() {
        return Err(format!(
            "That preset does not fit this title: {}",
            problems.join(" ")
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Secondary windows
// ---------------------------------------------------------------------------

/// Which secondary windows and dialogs exist, and which are open.
#[tauri::command]
fn window_catalog(
    state: State<'_, SharedState>,
) -> Result<windows::SurfaceCatalogViewModel, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.surfaces.catalog())
}

/// Open a secondary window or dialog.
#[tauri::command]
fn open_window(state: State<'_, SharedState>, id: String) -> Result<windows::SurfaceSpec, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.surfaces.open(&id)
}

/// Close a secondary window or dialog.
///
/// An empty name, or `all`, closes every one of them; anything else has to
/// name a surface this build offers, so a stale page cannot quietly close a
/// surface the user opened on purpose.
#[tauri::command]
fn close_window(
    state: State<'_, SharedState>,
    id: String,
) -> Result<windows::SurfaceStateViewModel, String> {
    let wanted = id.trim();
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    if wanted.is_empty() || wanted == "all" {
        guard.surfaces.close_all();
        return Ok(guard.surfaces.view());
    }
    if !SurfaceState::is_surface(wanted) {
        return Err(format!("'{wanted}' is not a window this build offers."));
    }
    guard.surfaces.close(wanted);
    Ok(guard.surfaces.view())
}

/* ----------------------------------------------------------------- appearance */

/* --------------------------------------------------------------- preferences */

/// One change to one preference, as the interface sends it.
///
/// The value is JSON rather than a fixed type because the surface has one
/// control per kind and this is the one place that knows which kind each field
/// is. A value of the wrong shape is refused by name rather than coerced.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PreferenceChange {
    /// Which preference is being changed.
    pub field: String,
    /// What it is being changed to.
    pub value: serde_json::Value,
}

/// The preferences surface, as the page renders it.
#[tauri::command]
fn preferences_view(state: State<'_, SharedState>) -> Result<PreferencesViewModel, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.preferences.view())
}

/// Show one preferences tab.
#[tauri::command]
fn set_preference_tab(
    state: State<'_, SharedState>,
    tab: String,
) -> Result<PreferencesViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preferences.set_active(&tab)?;
    Ok(guard.preferences.view())
}

/// What a language switch moved, for the page to apply.
///
/// Two answers and not three: the configuration tabs are deliberately absent,
/// because redrawing them is what would throw away what the user has typed. The
/// page is told what changed rather than being asked to reload everything, so it
/// cannot refresh a surface the window did not mean to touch.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LanguageChange {
    /// Which language is in force, and what it can be drawn with.
    pub language: LanguageViewModel,
    /// The appearance answer, because a language can move the layout.
    pub appearance: AppearanceViewModel,
    /// The preferences answer, because the picker and every other label on the
    /// tabs is written in the language that just changed.
    pub preferences: PreferencesViewModel,
}

/// Apply a chosen language to the open window.
///
/// Split out from the command so the rule that a language switch touches nothing
/// but the words can be tested without a running application: what this returns
/// is exactly what the page is sent, and the state behind the configuration tabs
/// is not read, rebuilt, or reset on the way through.
fn apply_language(state: &SharedState, requested: &str) -> Result<LanguageChange, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    let direction_mode = guard.appearance.direction_mode();
    let switched: LanguageSwitch =
        locale::switch_language(&mut guard.appearance, direction_mode, requested)?;
    // The catalog's descriptions are in the same language as everything else, so
    // one switch moves them: a browser showing English descriptions under a
    // German interface would be two answers to one question.
    guard.presets.use_language(requested);
    Ok(LanguageChange {
        language: switched.view,
        appearance: switched.appearance,
        preferences: guard.preferences.view(),
    })
}

/// Change one preference.
///
/// A change to a preference that decides which kind of backend there is discards
/// the running worker, so the next operation is built from the answer the user
/// just gave rather than from the one it was started with. A change to the log
/// level moves the activity window's floor with it, so the two filters cannot
/// disagree about what is being kept.
#[tauri::command]
fn set_preference(
    app: AppHandle,
    state: State<'_, SharedState>,
    change: PreferenceChange,
) -> Result<PreferencesViewModel, String> {
    // The record is snapshotted before the change and compared after it, so the
    // window reacts to what actually moved rather than to which field name the
    // page sent. One preference can move another — the collision switch settles
    // the collision choice — and this way that is noticed too.
    let before = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.preferences.settings().clone()
    };
    let view = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.preferences.set(&change.field, &change.value)?;
        guard.preferences.view()
    };
    let after = {
        let guard = state.lock().map_err(|e| e.to_string())?;
        guard.preferences.settings().clone()
    };
    if before.advanced.process_isolation != after.advanced.process_isolation {
        rebuild_backend_after_preference(&state, "process isolation was switched");
    }
    if before.advanced.log_level != after.advanced.log_level {
        // The activity window's floor moves with the preference so the two
        // filters cannot disagree about what is being kept.
        if let Ok(mut guard) = state.lock() {
            guard.activity.set_min_level(after.advanced.log_level);
        }
    }
    if before.hardware != after.hardware {
        push_log(
            &state,
            "the hardware preferences changed; the next job uses them",
        );
    }
    if before.general != after.general {
        let stills = after.general.preview_count;
        // The preview strip follows the preference until the user changes it for
        // one source, rather than the two holding different answers.
        if let Ok(mut guard) = state.lock() {
            let _ = guard.preview.set_still_count(stills);
        }
    }
    if before.general.language != after.general.language {
        // The language is the one preference that changes what is already on
        // screen, so it is applied here rather than being left for the next read:
        // the window pushes the two answers that moved and the page applies them.
        // The record has already been written by the setter above, which is why
        // this only has to say what to draw.
        let requested = after.general.language.tag();
        let change = apply_language(&state, &requested)?;
        push_log(
            &state,
            format!(
                "the interface language changed: {}",
                change.language.summary
            ),
        );
        if let Err(e) = app.emit(LANGUAGE_EVENT, &change) {
            eprintln!("the new language could not be sent to the page: {e}");
        }
    }
    if before.advanced != after.advanced || before.updates != after.updates {
        // These preferences are read by the desktop integration rather than by the
        // page: the hold on the machine, the sleep scope, the placement of the
        // worker, the commands around an encode, the notifications, and whether a
        // release should be looked for. So the tray and the log are brought up to
        // date here rather than at the next job.
        refresh_system(&state);
    }
    Ok(view)
}

/// Look for hardware again.
///
/// Detection is a pair of subprocess calls, so it is not done on every read; it
/// is done at start-up and whenever the user asks for it here.
#[tauri::command]
fn recheck_hardware(state: State<'_, SharedState>) -> Result<PreferencesViewModel, String> {
    let capabilities = vise_core::HardwareCapabilities::detect();
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preferences.observe_hardware(&capabilities);
    Ok(guard.preferences.view())
}

/// Restore every preference to its default, once the countdown has confirmed it.
///
/// The answer can be taken back: the previous record is held for one reset and
/// [`undo_preference_reset`] puts it back, so a destructive control is offered
/// here without the user having to trust that they meant to press it.
#[tauri::command]
fn reset_preferences(state: State<'_, SharedState>) -> Result<PreferencesViewModel, String> {
    require_confirmation(&state, AlertAction::ResetPreferences)?;
    let view = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        guard.preferences.reset();
        guard.preferences.observe_portable(&portable_settings());
        guard.preferences.view()
    };
    take_confirmation(&state, AlertAction::ResetPreferences);
    // Announced rather than left to be noticed: a reset that silently changed
    // every tab would look like the window losing the user's settings.
    rebuild_backend_after_preference(&state, "the preferences were restored to their defaults");
    push_log(
        &state,
        format!(
            "every preference was restored to its default; {}",
            view.reset.summary
        ),
    );
    Ok(view)
}

/// Put back the preferences a reset replaced.
///
/// One reset is undoable and only one: holding a longer history would let a
/// take-back be replayed into a record the user has since changed.
#[tauri::command]
fn undo_preference_reset(state: State<'_, SharedState>) -> Result<PreferencesViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.preferences.undo_reset()?;
    Ok(guard.preferences.view())
}

/// The portable settings that apply to this build.
///
/// Read on demand rather than cached at start-up, so a portable file that is
/// added or removed while the window is open is seen the next time the isolation
/// answer is needed.
fn portable_settings() -> PortableSettings {
    let executable_dir = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("."));
    vise_ipc::portable::load_for(&executable_dir).unwrap_or_default()
}

/// Act on the When-done preferences once the queue has drained.
///
/// Returns what the window should do rather than doing it, so the rule is a plain
/// function that a test can pin without a window closing itself. `None` is the
/// answer when the queue has not actually drained, which is the common case: a
/// job finishes while three more are queued and nothing is acted on.
pub fn when_done_plan(
    settings: &PreferencesSettings,
    pending: usize,
    running: usize,
) -> Option<WhenDonePlan> {
    if pending > 0 || running > 0 {
        return None;
    }
    let action = settings.when_done.action;
    if action == preferences::whendone::WhenDoneAction::Nothing {
        return None;
    }
    if !action.is_available() {
        // Refused before it is stored, so reaching here means a record from a
        // newer build got here; acting on nothing is better than pretending.
        return None;
    }
    Some(WhenDonePlan {
        action,
        delay_seconds: settings.when_done.delay_seconds,
    })
}

/// What the window is going to do because the queue finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WhenDonePlan {
    /// What to do.
    pub action: preferences::whendone::WhenDoneAction,
    /// How long to wait first.
    pub delay_seconds: u32,
}

/// Close the window the When-done preferences asked to close.
///
/// The confirmation is spent here, and only here, so one answer to the countdown
/// closes one window and cannot be replayed into something else. The page
/// reaches this because the window opened the alert itself; a page that pressed
/// the confirm control without a confirmation behind it is refused.
#[tauri::command]
fn when_done_confirm(app: AppHandle, state: State<'_, SharedState>) -> Result<(), String> {
    require_confirmation(&state, AlertAction::CloseWindow)?;
    take_confirmation(&state, AlertAction::CloseWindow);
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return Err("There is no window to close.".to_string());
    };
    window
        .close()
        .map_err(|e| format!("the window could not be closed: {e}"))
}

#[tauri::command]
fn appearance_view(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<AppearanceViewModel, String> {
    // The platform is asked on every read rather than only at start-up, because
    // the window theme is the one observation available before the page has run
    // and the one that costs nothing to re-ask.
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    observe_platform_theme(&app, &mut guard.appearance);
    Ok(guard.appearance.view())
}

/// Choose the theme, and keep the choice.
#[tauri::command]
fn set_theme_mode(
    mode: String,
    state: State<'_, SharedState>,
) -> Result<AppearanceViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.appearance.set_theme_mode(&mode)
}

/// Choose the layout direction, and keep the choice.
#[tauri::command]
fn set_direction_mode(
    mode: String,
    state: State<'_, SharedState>,
) -> Result<AppearanceViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.appearance.set_direction_mode(&mode)
}

/// Record the theme the page reports for the `prefers-color-scheme` query.
///
/// The page is a sensor here and nowhere else: it says which way the media query
/// is pointing and the window decides what that means. Reporting the query
/// rather than a theme name is deliberate, so the colours on screen and the
/// answer in this state cannot be produced by two different readings of the
/// same preference.
#[tauri::command]
fn report_system_theme(
    dark: bool,
    state: State<'_, SharedState>,
) -> Result<AppearanceViewModel, String> {
    record_system_theme(&state, dark)
}

/// Record what the page read off the media query, and hand back the answer.
///
/// Split out from the command so the rule about what is worth a line in the
/// activity window can be tested without a running application. What is logged is
/// the theme that was on screen before and after, not whether the record moved:
/// the page reporting the same theme the platform already reported hands the
/// window a stronger witness without changing a single pixel, and that handover
/// is not something to put in the log.
fn record_system_theme(state: &SharedState, dark: bool) -> Result<AppearanceViewModel, String> {
    let (view, was, now) = {
        let mut guard = state.lock().map_err(|e| e.to_string())?;
        let was = guard.appearance.view().theme;
        guard
            .appearance
            .observe_theme(SystemTheme::of_dark(dark), true);
        let view = guard.appearance.view();
        let now = view.theme.clone();
        (view, was, now)
    };
    if was != now {
        push_log(state, "the system theme changed; the window follows it");
    }
    Ok(view)
}

/// Record the locale the page is showing in.
#[tauri::command]
fn report_locale(
    locale: String,
    state: State<'_, SharedState>,
) -> Result<AppearanceViewModel, String> {
    let mut guard = state.lock().map_err(|e| e.to_string())?;
    guard.appearance.observe_locale(&locale);
    Ok(guard.appearance.view())
}

/// The language picker, as the window decides it.
///
/// Separate from the change itself because a page that only wants to redraw the
/// picker must not have to send a preference to ask.
#[tauri::command]
fn language_view(state: State<'_, SharedState>) -> Result<LanguageViewModel, String> {
    let guard = state.lock().map_err(|e| e.to_string())?;
    // Asked of the appearance state rather than of the record, because that is
    // where the language in force is: the record says what was chosen and the
    // window says what it resolved to, and those are different answers on a
    // machine set to a language this build does not ship.
    let tag = guard.appearance.language().to_string();
    Ok(locale::language_view(
        &vise_core::resolve_catalog(&tag),
        guard.appearance.direction_mode(),
    ))
}

/// Look for a newer release now, and answer with the Updates tab.
///
/// The button is closed with a reason when there is nothing to look at, so this is
/// only reachable when a feed has been configured. The fetch is done on its own
/// thread because a network call is not something a window can wait on while
/// painting, and the view it answers with is the one from before the fetch: the
/// last check is a row on the tab, and the page reads it again when it repaints.
#[tauri::command]
fn check_for_updates_now(state: State<'_, SharedState>) -> Result<PreferencesViewModel, String> {
    let shared: SharedState = state.inner().clone();
    if shared
        .lock()
        .map(|guard| guard.preferences.settings().updates.has_feed())
        .unwrap_or(false)
    {
        thread::spawn(move || check_for_releases(&shared));
    }
    let guard = state.lock().map_err(|e| e.to_string())?;
    Ok(guard.preferences.view())
}
/// Ask the platform what theme it is in, and record the answer.
///
/// Not every platform answers, and one that does not is left at the light theme
/// rather than failing: a window that opens readable and then corrects itself is
/// better than one that refuses to open.
fn observe_platform_theme(app: &AppHandle, appearance: &mut AppearanceState) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    let Ok(theme) = window.theme() else {
        return;
    };
    appearance.observe_theme(SystemTheme::of_window(theme), false);
}

fn main() {
    // Where state is written is decided before anything reads any of it: a portable
    // build keeps its settings, its queue, and its presets beside the executable,
    // and an installed build keeps them in the configuration directory. The rules
    // are in `vise_ipc::portable` and are tested there, so this is a resolution
    // rather than a decision.
    let portable = portable_settings();
    let installed_root = sources::default_settings_path()
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let storage =
        vise_ipc::portable::resolve_storage(&portable, &executable_dir(), &installed_root);
    let settings_path = storage.settings.file.clone();
    let recents = RecentSources::load(&settings_path);
    let preset_path = storage.presets.file.clone();
    // The theme starts light and the locale comes from the environment, so a
    // window has an answer before the page has said anything. The setup hook
    // replaces the theme with the platform's own as soon as there is a window
    // to ask.
    let mut appearance = AppearanceState::load(&settings_path, SystemTheme::Light, None);
    // The preferences are read before anything else is built, because three of
    // them decide what gets built: whether there is a worker process, how much
    // is logged, and how many stills a scan extracts.
    let mut preferences = PreferencesState::load(&settings_path, &portable);
    // The stored language decides the words the window opens in, and the layout
    // that follows from it, before anything is painted rather than on the first
    // poll: a window that opens in the wrong language for one frame and then
    // corrects itself is a window the reader has already read.
    let stored_language = preferences.settings().general.language.tag();
    let opening_direction = appearance.direction_mode();
    if let Err(problem) =
        locale::switch_language(&mut appearance, opening_direction, &stored_language)
    {
        eprintln!("the stored interface language could not be applied: {problem}");
    }
    // Detection is a pair of subprocess calls, so it is done once here rather
    // than on every read of the hardware tab.
    preferences.observe_hardware(&vise_core::HardwareCapabilities::detect());
    let stills = preferences.settings().preview_count();
    let log_floor = preferences.settings().advanced.log_level;
    let isolate = preferences.settings().advanced.process_isolation;
    let check_on_start = preferences.settings().updates.has_feed()
        && system::updates::UpdateState::new().due(
            preferences.settings().updates.frequency,
            preferences::updates::today(),
        );
    // The queue comes back before the window opens, so the queue window shows the
    // truth rather than an empty list that fills in a moment later. A job that was
    // interrupted comes back waiting, marked, and is never started on its own.
    let recovered = system::persistence::load(&storage.queue.file);
    let mut session = vise_core::Session::new();
    let recovered_queue = recovered.install(&mut session);
    let diagnostics = DiagnosticsState::new(storage.logs.file.clone());
    let state: SharedState = Arc::new(Mutex::new(DesktopState {
        // Build the backend before the window opens so the first scan does not
        // pay for process start-up.
        backend: Some(build_backend(isolate)),
        recents,
        settings_path,
        presets: PresetState::new(preset_path),
        tabs: TabSettings::new(),
        appearance,
        preferences,
        preview: windows::preview::PreviewState::with_still_count(stills),
        integration: Integration::new(storage.queue.file.clone()),
        storage,
        diagnostics,
        queue: recovered_queue,
        session,
        ..DesktopState::default()
    }));
    // The activity window's filter floor is the stored preference, so the two
    // places that decide what is kept start from one answer.
    if let Ok(mut guard) = state.lock() {
        guard.activity.set_min_level(log_floor);
        // What this machine can do is asked once here rather than on every paint of
        // the Advanced tab: a processor layout and a notification area are
        // questions for the platform, and the window keeps the answers.
        let notifications = system::notify::NotificationState {
            available: guard.integration.notifier_available(),
            description: guard.integration.notifier_description(),
            scope: notify_policy(&guard).scope,
            window_visible: false,
        };
        let refusals = guard.storage.refusals.clone();
        let storage_summary = guard.storage.summary();
        guard
            .preferences
            .observe_machine(preferences::advanced::MachineFacts::of(notifications));
        for refusal in refusals {
            guard.activity.record(refusal);
        }
        guard
            .activity
            .record(format!("state is written: {storage_summary}"));
        for note in &recovered.notes {
            guard.activity.record(note.clone());
        }
        if !recovered.items.is_empty() {
            guard
                .activity
                .record(format!("queue recovery: {}", recovered.summary()));
        }
    }
    // The builder needs the state in several places and moves each closure, so each
    // one is handed its own handle to the same lock.
    let start_up = state.clone();
    let observing = state.clone();
    let focusing = state.clone();
    let start_check = state.clone();
    tauri::Builder::default()
        .manage(state)
        .setup(move |app| {
            // The window exists by now, so the platform can be asked before the
            // page has run. That is the only observation available at this point,
            // and it is what the page starts from if it has nothing of its own.
            if let Some(window) = app.get_webview_window(MAIN_WINDOW) {
                if let Ok(theme) = window.theme() {
                    if let Ok(mut guard) = start_up.lock() {
                        guard
                            .appearance
                            .observe_theme(SystemTheme::of_window(theme), false);
                    }
                }
                // The notification area needs the window to show against, so the
                // notifier is built here rather than before the window existed.
                if let Ok(handle) = window.hwnd() {
                    if let Ok(mut guard) = start_up.lock() {
                        guard.integration.use_notifier(std::sync::Arc::new(
                            system::notify::BalloonNotifier::new(
                                handle.0 as isize,
                                "Vise Transcoder",
                            ),
                        ));
                        let described = guard.integration.notifier_description();
                        guard.activity.record(described);
                    }
                }
            }
            build_tray(app);
            let _ = HANDLE.set(app.handle().clone());
            // The tray is published once here, so the icon is never sitting there
            // with an empty menu while the first refresh works out what it says.
            refresh_system(&start_up);
            // The first look for a release is on its own thread: it is a network
            // call, and by this point the window is already open.
            if check_on_start {
                let shared = start_check.clone();
                thread::spawn(move || check_for_releases(&shared));
            }
            Ok(())
        })
        // The operating system can change theme while the window is open, and the
        // window itself can come to the front or go behind another one. Both are
        // read here, because both decide whether a notification is worth showing:
        // somebody watching the window has already seen what happened.
        .on_window_event(move |window, event| {
            let focus_state = focusing.clone();
            match event {
                WindowEvent::Focused(focused) => {
                    if let Ok(guard) = focus_state.lock() {
                        guard.window_visible.store(*focused, Ordering::SeqCst);
                    }
                    let shared = focus_state.clone();
                    thread::spawn(move || refresh_system(&shared));
                }
                WindowEvent::Destroyed => {
                    // The queue is written as the window goes, so a closed window is
                    // never the reason a queued job is gone tomorrow.
                    persist_queue(&focus_state);
                }
                _ => {}
            }
            let WindowEvent::ThemeChanged(theme) = event else {
                return;
            };
            let view = {
                let Ok(mut guard) = observing.lock() else {
                    return;
                };
                if !guard
                    .appearance
                    .observe_theme(SystemTheme::of_window(*theme), false)
                {
                    return;
                }
                guard.appearance.view()
            };
            if let Err(e) = window.emit(APPEARANCE_EVENT, &view) {
                eprintln!("appearance could not be sent to the page: {e}");
            }
        })
        .invoke_handler(tauri::generate_handler![
            select_source,
            inspect_sources,
            clear_source,
            recent_sources,
            forget_recent_source,
            clear_recent_sources,
            source_selection,
            begin_scan,
            scan_status,
            choose_title,
            choose_range,
            choose_tracks,
            tab_set,
            set_active_tab,
            set_summary,
            set_dimensions,
            set_filter,
            set_video,
            set_audio_defaults,
            set_audio_track,
            set_subtitles,
            set_subtitle_track,
            set_chapters,
            change_marker,
            import_chapters,
            set_metadata,
            import_subtitles,
            preview_window,
            fetch_preview_still,
            set_preview_still_count,
            select_preview_still,
            set_preview_crop,
            reset_preview_crop,
            apply_preview_crop,
            add_job,
            validate_job_spec,
            queue_selection,
            queue_selection_window,
            set_queue_selection,
            queue_titles,
            queue_window,
            move_job,
            set_job_priority,
            list_jobs,
            remove_job,
            run_job,
            start_queue,
            cancel_all,
            clear_finished_jobs,
            empty_queue,
            pause_job,
            resume_job,
            cancel_job,
            cancel_job_confirmed,
            activity_window,
            set_log_level,
            set_follow_logs,
            clear_logs,
            diagnostics_window,
            set_sweep_range,
            confirm_sweep_range,
            start_sweep,
            cancel_sweep,
            export_queue_results,
            detect_capabilities,
            export_capabilities,
            export_diagnostics,
            error_view,
            error_report,
            clear_errors,
            ask_for_alert,
            tick_alert,
            confirm_alert,
            dismiss_alert,
            about_view,
            preset_window,
            browse_catalog,
            apply_preset_action,
            window_catalog,
            open_window,
            close_window,
            appearance_view,
            set_theme_mode,
            set_direction_mode,
            language_view,
            report_system_theme,
            report_locale,
            preferences_view,
            set_preference_tab,
            set_preference,
            recheck_hardware,
            reset_preferences,
            undo_preference_reset,
            when_done_confirm,
            check_for_updates_now
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    //! The contract between the page and these commands.
    //!
    //! The page sends plain JSON and reads plain JSON. These tests pin both
    //! directions so a rename on either side fails here rather than in the
    //! window.

    use super::*;
    use tabs::{SummarySettings, VideoSettings};

    /// A window whose queue holds `specs`, with nothing running.
    fn window_with(specs: &[JobSpec]) -> DesktopState {
        let mut session = vise_core::Session::new();
        let mut queue = QueueState::default();
        for spec in specs {
            let id = session.add_job(spec.clone());
            queue.push(id);
        }
        DesktopState {
            session,
            queue,
            ..DesktopState::default()
        }
    }

    fn spec_named(name: &str) -> JobSpec {
        JobSpec {
            input_path: format!("C:\\media\\{name}.mkv"),
            output_path: format!("D:\\out\\{name}.mp4"),
            ..JobSpec::default()
        }
    }

    #[test]
    fn the_facts_the_tray_is_built_from_are_the_queue_s_own() {
        // The tray, the notifications, and the saved queue all describe the same
        // queue, so they read it through the queue's own methods rather than each
        // keeping a count of its own.
        let window = window_with(&[spec_named("One"), spec_named("Two")]);
        let facts = system_facts(&window);
        assert_eq!(facts.queued, 2);
        assert_eq!(facts.running, 0);
        assert_eq!(facts.finished, 0);
        assert_eq!(facts.last_failure, None);
        assert!(
            facts.in_process,
            "a window with no worker behind it runs the pipeline itself"
        );
        assert!(facts.queue_paused.is_none());
    }

    #[test]
    fn the_last_failure_is_carried_with_the_file_and_the_reason() {
        // A notification about a failure names the file, so the facts carry both
        // rather than leaving the window to go looking for them.
        let mut window = window_with(&[spec_named("One"), spec_named("Two")]);
        let ids: Vec<JobId> = window.queue.pending(window.session.jobs());
        let second = ids[1];
        set_job_state_holding(&mut window, second, JobState::Failed, "no space left");
        let facts = system_facts(&window);
        assert_eq!(facts.failed, 1);
        assert_eq!(
            facts.last_failure,
            Some(("D:\\out\\Two.mp4".to_string(), "no space left".to_string()))
        );
    }

    /// Change a job's state in the window directly, so a test can arrange one
    /// without going through a worker.
    fn set_job_state_holding(window: &mut DesktopState, id: JobId, to: JobState, why: &str) {
        let _ = window.session.update_status(
            id,
            vise_core::JobStatus {
                state: to,
                progress: Progress::new(0.0),
                message: Some(why.to_string()),
            },
        );
    }

    #[test]
    fn a_window_without_a_worker_reports_itself_as_running_the_pipeline() {
        let mut window = window_with(&[]);
        window.backend = Some(Backend::InProcess);
        assert!(system_facts(&window).in_process);
        // A worker behind it means the hold belongs to that worker instead.
        window.backend = Some(Backend::Worker(Box::new(vise_ipc::WorkerSupervisor::new(
            vise_ipc::WorkerConfig::new("vise-worker.exe"),
        ))));
        assert!(!system_facts(&window).in_process);
    }

    #[test]
    fn the_settings_a_job_carries_are_the_ones_the_preferences_hold() {
        // The three preferences the worker reads are the three the window sends,
        // read from the one record rather than from a second table in main.rs.
        let settings = PreferencesSettings::new();
        let init = settings.to_worker_init();
        assert!(init.prevent_sleep, "an encode holds the machine awake");
        assert_eq!(init.power_scope, 0, "the screen is not held by default");
        assert_eq!(
            init.numa_mode, 0,
            "the placement is decided from the machine"
        );
        assert_eq!(init.process_priority, 0);
    }

    fn idle_flow() -> ScanFlow {
        ScanFlow::default()
    }

    #[test]
    fn duration_options_are_validated_before_they_reach_the_worker() {
        assert_eq!(
            parse_duration(None, "shortest").expect("none"),
            TimeSpan::ZERO
        );
        assert_eq!(
            parse_duration(Some("  "), "shortest").expect("blank"),
            TimeSpan::ZERO
        );
        assert_eq!(
            parse_duration(Some("00:00:30"), "longest")
                .expect("valid")
                .to_string(),
            "00:00:30"
        );
        let error = parse_duration(Some("99:99:99"), "shortest").expect_err("invalid");
        assert!(error.contains("shortest title duration"), "{error}");
        assert!(error.contains("99:99:99"), "{error}");
    }

    /// A job whose source and destination really exist, so a test about the rules
    /// can leave the paths alone.
    fn real_job_stub() -> (JobSpec, vise_core::ScanResult) {
        let dir = std::env::temp_dir().join("vise-desktop-validation");
        let _ = std::fs::create_dir_all(&dir);
        let input = dir.join("in.mkv");
        std::fs::write(&input, b"not real media").expect("write");
        let spec = JobSpec {
            container: vise_core::ContainerFormat::Mkv,
            input_path: input.to_string_lossy().to_string(),
            output_path: dir.join("out.mkv").to_string_lossy().to_string(),
            quality: Some(22),
            ..Default::default()
        };
        let facts = tabs::source::TitleFacts {
            number: 1,
            width: 640,
            height: 480,
            duration_seconds: 10.0,
            audio: vec![tabs::source::AudioFacts {
                number: 1,
                channel_count: 2,
                ..Default::default()
            }],
            ..Default::default()
        };
        (spec, facts.to_scan_result())
    }

    #[test]
    fn a_job_is_judged_before_it_is_queued_and_every_problem_is_reported() {
        let (mut job, scan) = real_job_stub();
        assert!(
            judge_job(&job, Some(scan.clone())).is_valid(),
            "a plain job with real paths and matching facts passes"
        );

        // Three separate mistakes, all of them named against their field.
        job.output_path = std::env::temp_dir()
            .join("vise-desktop-validation")
            .join("out.mp4")
            .to_string_lossy()
            .to_string();
        job.video_codec = vise_core::VideoCodec::ProRes;
        job.audio_processing.channels = Some(6);
        let report = judge_job(&job, Some(scan.clone()));
        let fields: Vec<&str> = report.issues.iter().map(|i| i.field.as_str()).collect();
        assert!(fields.contains(&"OutputPath"), "{fields:?}");
        assert!(fields.contains(&"Audio.Channels"), "{fields:?}");
        let message = refusal(&report);
        assert!(message.contains("Audio.Channels"), "{message}");
        assert!(message.contains("Nothing was queued"), "{message}");
        assert_eq!(
            message.lines().count(),
            report.issues.len() + 2,
            "one line per problem, plus the two sentences around them"
        );
    }

    #[test]
    fn facts_are_only_offered_for_the_source_that_was_scanned() {
        let (job, _) = real_job_stub();
        let mut window = DesktopState::default();
        // Nothing has been scanned, so there are no facts to offer.
        assert!(source_facts_for(&window, &job).is_none());
        window.flow.set_source(Some(sources::SourceViewModel {
            path: job.input_path.clone(),
            name: "in.mkv".to_string(),
            location: String::new(),
            kind: sources::SourceKind::File,
            label: "in.mkv".to_string(),
        }));
        // The source is known but no title is chosen, so there is still nothing.
        assert!(source_facts_for(&window, &job).is_none());

        // A job against a different file is never judged with the facts of the
        // scanned one: they are answers about the wrong media.
        let mut other = job.clone();
        other.input_path = "C:\\media\\somewhere-else.mkv".to_string();
        assert!(source_facts_for(&window, &other).is_none());
    }

    #[test]
    fn a_job_that_cannot_run_is_refused_by_the_queue_before_it_is_claimed() {
        let (mut job, _) = real_job_stub();
        // The source is deleted, so no encode can start.
        job.input_path = std::env::temp_dir()
            .join("vise-desktop-validation")
            .join("not-there.mkv")
            .to_string_lossy()
            .to_string();
        let state: SharedState = Arc::new(Mutex::new(window_with(&[job])));
        let id = state.lock().expect("lock").session.jobs()[0].id;

        // The refusal is about this job alone, so the queue behind it carries on.
        match prepare_job(&state, id) {
            Err(ClaimStop::Job { reason }) => assert!(reason.contains("InputPath"), "{reason}"),
            Err(ClaimStop::Queue { reason }) => panic!("expected a job-level refusal: {reason}"),
            Ok(_) => panic!("a job with no source must not be claimed"),
        }
        assert_eq!(
            state.lock().expect("lock").session.jobs()[0].status.state,
            JobState::Queued,
            "a refused job was never claimed, so it is still waiting"
        );
    }

    #[test]
    fn a_job_the_tabs_would_queue_is_claimed_and_marked_running() {
        let (job, _) = real_job_stub();
        let state: SharedState = Arc::new(Mutex::new(window_with(&[job])));
        let id = state.lock().expect("lock").session.jobs()[0].id;
        prepare_job(&state, id).expect("a sound job is claimed");
        assert_eq!(
            state.lock().expect("lock").session.jobs()[0].status.state,
            JobState::Running
        );
    }

    #[test]
    fn scan_options_build_the_worker_command() {
        let options: ScanOptions = serde_json::from_str(
            r#"{"previewCount":99,"minDuration":"","maxDuration":"00:10:00",
                "fileExclusions":[" sample.mkv ","  "]}"#,
        )
        .expect("options");
        let mut settings = PreferencesSettings::new();
        settings.general.keep_duplicate_titles = true;
        let command = options
            .to_command(
                vec!["C:\\media\\clips".to_string()],
                &settings,
                Path::new("logs"),
            )
            .expect("command");
        assert_eq!(command.paths, vec!["C:\\media\\clips".to_string()]);
        assert_eq!(command.preview_count, 32, "clamped to something sane");
        assert_eq!(
            command.min_duration,
            TimeSpan::ZERO,
            "an empty value means no limit"
        );
        assert_eq!(command.max_duration.to_string(), "00:10:00");
        // The duplicate-title switch is one answer, held by the General
        // preferences rather than sent by the page alongside it.
        assert!(command.initialise_command.keep_duplicate_titles);
        assert_eq!(command.file_exclusion_list, vec!["sample.mkv".to_string()]);
    }

    #[test]
    fn a_scan_is_told_where_to_write_its_logs_from_the_resolved_storage_root() {
        // The same folder the storage rules resolved, so a portable install's
        // worker logs beside the executable and an installed one's stay in the
        // configuration directory.
        let settings = PreferencesSettings::new();
        let folder = Path::new("C:\\cfg\\vise\\logs");
        let init = settings.to_worker_init_in(folder);
        assert!(init.enable_disk_logging);
        assert_eq!(init.log_directory, folder.to_string_lossy());
        assert_eq!(init.log_folder().as_deref(), Some(folder));
        assert!(init.log_file_path().is_some());
    }

    #[test]
    fn a_sweep_without_a_source_says_so_rather_than_planning_an_encode_of_nothing() {
        let mut window = DesktopState {
            diagnostics: windows::diagnostics::DiagnosticsState::new(PathBuf::from("logs")),
            ..DesktopState::default()
        };
        assert_eq!(sweep_base_job(&window).input_path, "");
        window
            .diagnostics
            .set_range(vise_core::SweepRange::new(20, 24, 2));
        let reason = window.diagnostics.sweep_refusal(false).expect("a refusal");
        assert!(reason.contains("open a source"), "{reason}");
    }

    #[test]
    fn a_sweep_is_refused_while_the_real_queue_has_an_encode_running() {
        let jobs = [spec_named("One")];
        let mut window = window_with(&jobs);
        window
            .diagnostics
            .set_range(vise_core::SweepRange::new(20, 24, 2));
        assert!(!queue_is_running(&window), "nothing is running yet");

        let id = window.session.jobs()[0].id;
        set_job_state_holding(&mut window, id, JobState::Running, "encoding");
        assert!(queue_is_running(&window));
        let reason = window.diagnostics.sweep_refusal(true).expect("a refusal");
        assert!(reason.contains("competing"), "{reason}");
    }

    #[test]
    fn the_diagnostics_surface_opens_with_an_empty_honest_table() {
        let window = DesktopState {
            diagnostics: windows::diagnostics::DiagnosticsState::new(PathBuf::from("logs")),
            ..DesktopState::default()
        };
        let view =
            window
                .diagnostics
                .view(&[], &vise_ipc::enclog::RotationPolicy::default(), false);
        assert!(view.encode_logs.is_empty());
        assert!(view.capability.is_none());
        assert!(view.reports.is_empty());
        assert_eq!(view.log_folder, "logs");
        assert!(
            view.summary.contains("No quality sweep"),
            "{}",
            view.summary
        );
    }

    #[test]
    fn an_export_that_cannot_be_written_is_reported_rather_than_saved_over() {
        // A file where a folder should be: the only way to be sure of an
        // unwritable path on a machine whose temporary folder is writable.
        let blocker = std::env::temp_dir().join("vise-desktop-export-blocker");
        std::fs::write(&blocker, b"not a folder").expect("blocker");
        let mut guard = DesktopState::default();
        let report = write_report(
            &mut guard,
            "results",
            &blocker,
            "vise-queue-results.csv",
            b"a,b\n1,2\n",
            "one row".to_string(),
        );
        assert!(!report.written, "{report:?}");
        assert!(
            report.problem.contains("could not be written"),
            "{report:?}"
        );
        assert!(
            !guard.errors.entries().is_empty(),
            "and it is on the problem list rather than only in the reply"
        );
        let _ = std::fs::remove_file(&blocker);
    }

    #[test]
    fn an_export_that_can_be_written_is_recorded_once_per_kind() {
        let dir = std::env::temp_dir().join("vise-desktop-export-test");
        let _ = std::fs::remove_dir_all(&dir);
        let mut guard = DesktopState::default();
        let first = write_report(
            &mut guard,
            "bundle",
            &dir,
            "one.zip",
            b"first",
            "first".to_string(),
        );
        let second = write_report(
            &mut guard,
            "bundle",
            &dir,
            "two.zip",
            b"second",
            "second".to_string(),
        );
        assert!(first.written && second.written, "{first:?} {second:?}");
        assert_eq!(
            std::fs::read(dir.join("two.zip")).expect("written"),
            b"second"
        );
        assert_eq!(guard.diagnostics.reports().len(), 1);
        assert_eq!(guard.diagnostics.reports()[0].path, second.path);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_options_default_to_no_previews_and_no_limits() {
        let options: ScanOptions = serde_json::from_str("{}").expect("empty options");
        let settings = PreferencesSettings {
            general: preferences::general::GeneralSettings {
                preview_count: 0,
                ..Default::default()
            },
            ..Default::default()
        };
        let command = options
            .to_command(vec!["in.mkv".to_string()], &settings, Path::new("logs"))
            .expect("command");
        assert_eq!(command.preview_count, 0);
        assert!(command.file_exclusion_list.is_empty());
        assert_eq!(command.title_index, 0, "zero asks for every title");
    }

    #[test]
    fn a_scan_that_says_nothing_about_stills_uses_the_stored_count() {
        // The preview window overrides this once it has been changed, so the two
        // are the same answer read from two places rather than two answers.
        let options: ScanOptions = serde_json::from_str("{}").expect("empty options");
        let settings = PreferencesSettings::new();
        assert_eq!(
            options
                .to_command(vec!["in.mkv".to_string()], &settings, Path::new("logs"))
                .expect("command")
                .preview_count,
            preferences::general::DEFAULT_PREVIEW_FRAMES
        );
        let asked: ScanOptions =
            serde_json::from_str(r#"{"previewCount":3}"#).expect("asked for three");
        assert_eq!(
            asked
                .to_command(vec!["in.mkv".to_string()], &settings, Path::new("logs"))
                .expect("command")
                .preview_count,
            3,
            "what the page asked for wins"
        );
    }

    #[test]
    fn the_page_sends_scan_options_the_command_accepts() {
        let payload = r#"{"previewCount":0,"minDuration":"","maxDuration":"",
            "fileExclusions":[]}"#;
        let options: ScanOptions = serde_json::from_str(payload).expect("camel case options");
        assert_eq!(options.preview_count, Some(0));
        assert!(options.file_exclusions.is_empty());
    }

    #[test]
    fn a_range_choice_arrives_as_point_to_point_with_its_points() {
        let payload = r#"{"mode":"point_to_point","startSeconds":90.0,
            "endSeconds":120.0,"angle":2}"#;
        let choice: RangeChoice = serde_json::from_str(payload).expect("choice");
        assert_eq!(choice.mode, "point_to_point");
        assert_eq!(choice.start_seconds, Some(90.0));
        assert_eq!(choice.end_seconds, Some(120.0));
        assert_eq!(choice.angle, Some(2));

        let selection = RangeSelection::default()
            .with_mode_name(&choice.mode)
            .expect("mode");
        assert_eq!(selection.mode, viewmodel::RangeMode::PointToPoint);

        // An empty mode keeps the mode already in force, which is how the point
        // pickers work without the page tracking the mode.
        let blank: RangeChoice = serde_json::from_str(r#"{"mode":""}"#).expect("blank");
        assert!(blank.mode.trim().is_empty());
    }

    #[test]
    fn an_unknown_range_mode_is_refused_with_a_reason() {
        let error = RangeSelection::default()
            .with_mode_name("sideways")
            .expect_err("unknown mode");
        assert!(error.contains("sideways"), "{error}");
    }

    #[test]
    fn a_track_choice_arrives_as_track_numbers() {
        let payload = r#"{"audio":[2,1],"subtitle":[1]}"#;
        let input: TrackSelectionInput = serde_json::from_str(payload).expect("tracks");
        assert_eq!(input.audio, vec![2, 1]);
        assert_eq!(input.subtitle, vec![1]);
        // Numbers outside the title are dropped before they are stored.
        let reconciled = input.reconcile(&vise_ipc::contract::ScanTitle {
            audio_list: vec![vise_ipc::contract::ScanAudioTrack {
                track_number: 1,
                ..Default::default()
            }],
            subtitle_list: vec![vise_ipc::contract::ScanSubtitleTrack {
                track_number: 1,
                ..Default::default()
            }],
            ..Default::default()
        });
        assert_eq!(reconciled.audio, vec![1]);
        assert_eq!(reconciled.subtitle, vec![1]);
    }

    #[test]
    fn scan_status_serialises_for_the_page() {
        let status = ScanStatusViewModel::of(&idle_flow());
        let json = serde_json::to_value(&status).expect("status");
        assert_eq!(json["phase"], "idle");
        assert_eq!(json["phaseName"], "idle");
        assert_eq!(json["running"], false);
        assert_eq!(json["titleCount"], 0);
        assert!(json["progress"]["percent"].is_number());
    }

    #[test]
    fn a_dropped_path_list_is_answered_with_accepted_and_refused_entries() {
        let dir = std::env::temp_dir().join("vise-desktop-drop");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("movie.mkv"), b"x").expect("file");

        let report = inspect_sources(vec![
            String::new(),
            dir.join("movie.mkv").to_string_lossy().to_string(),
            dir.join("gone.mkv").to_string_lossy().to_string(),
        ])
        .expect("report");
        assert_eq!(report.accepted.len(), 1);
        assert_eq!(report.rejected.len(), 2);
        let json = serde_json::to_value(&report).expect("serialise");
        assert_eq!(json["accepted"][0]["kind"], "file");
        assert!(!json["rejected"][0]["reason"].as_str().unwrap().is_empty());
    }

    #[test]
    fn the_tab_set_arrives_as_one_object_the_page_can_render() {
        let settings = TabSettings::new();
        let view = settings.view(&TitleFacts::default());
        let json = serde_json::to_value(&view).expect("serialise");
        assert_eq!(json["tabs"].as_array().map(|tabs| tabs.len()), Some(8));
        assert_eq!(json["tabs"][0]["id"], "summary");
        assert_eq!(json["active"], "summary");
        assert_eq!(json["source"]["chosen"], false);
        assert!(json["summary"]["container"]["options"].is_array());
        assert!(json["dimensions"]["cropMode"]["selected"].is_string());
        assert!(json["filters"]["rows"].is_array());
        assert_eq!(json["video"]["rateControl"]["selected"], "quality");
        assert!(json["audio"]["tracks"].is_array());
        assert!(json["subtitles"]["behaviour"]["options"].is_array());
        assert!(json["chapters"]["mode"]["options"].is_array());
        assert!(json["metadata"]["mode"]["options"].is_array());
        assert!(json["metadata"]["tags"].is_array());
        assert_eq!(json["metadata"]["mode"]["selected"], "copy");
        assert!(json["problems"].is_array());
    }

    #[test]
    fn a_tab_change_arrives_as_camel_case_the_command_accepts() {
        let payload = r#"{"container":"mkv","videoCodec":"vp9","audioCodec":"opus",
            "webOptimized":false,"ipodCompatible":false,"alignAudioVideo":true,
            "passthroughMetadata":false}"#;
        let change: SummaryInput = serde_json::from_str(payload).expect("camel case change");
        let mut settings = SummarySettings::default();
        change.apply(&mut settings).expect("apply");
        assert_eq!(settings.container, "mkv");
        assert_eq!(settings.video_codec, "vp9");
        assert!(!settings.web_optimized);
        assert!(settings.align_audio_video);

        let video = r#"{"encoder":"av1","rateControl":"bitrate","bitrate":"4000",
            "twoPass":true,"preset":"slow","colourRange":"full"}"#;
        let change: VideoInput = serde_json::from_str(video).expect("video change");
        let mut video_settings = VideoSettings::default();
        change.apply(&mut video_settings).expect("video");
        assert_eq!(video_settings.encoder, "av1");
        assert_eq!(video_settings.bitrate.text, "4000");
        assert!(video_settings.two_pass);
        assert_eq!(video_settings.colour_range, "full");

        let filter = r#"{"filter":"denoise","enabled":true,"choice":"custom","value":"7"}"#;
        let change: tabs::FilterInput = serde_json::from_str(filter).expect("filter change");
        let mut filters = tabs::FilterSettings::default();
        change.apply(&mut filters).expect("filter");
        assert!(filters.denoise);
        assert_eq!(filters.denoise_value(), Some(7));

        let marker = r#"{"action":"add","name":"Extra","start":"10","end":"20"}"#;
        let change: MarkerInput = serde_json::from_str(marker).expect("marker change");
        let mut chapters = tabs::ChapterSettings::default();
        change.apply(&mut chapters).expect("marker");
        assert_eq!(chapters.markers.len(), 1);
        assert_eq!(chapters.markers[0].name, "Extra");
    }

    #[test]
    fn only_subtitle_files_are_accepted_for_import() {
        assert!(subtitle_extension("extra.srt"));
        assert!(subtitle_extension("captions.ASS"));
        assert!(subtitle_extension("subs.webvtt"));
        assert!(!subtitle_extension("movie.mkv"));
        assert!(!subtitle_extension("subtitles"));
    }

    #[test]
    fn a_tab_the_build_does_not_offer_is_refused_by_name() {
        let settings = TabSettings::new();
        assert!(settings.is_tab("chapters"));
        assert!(settings.is_tab("metadata"));
        assert!(!settings.is_tab("advanced"));
    }

    #[test]
    fn a_queue_selection_arrives_as_the_names_and_choices_it_needs() {
        let payload = r#"{"outputDirectory":"C:\\out","namePattern":"{title} ({n})",
            "names":{"2":"Second"},"titles":[1,2]}"#;
        let input: windows::queue::QueueSelectionInput =
            serde_json::from_str(payload).expect("selection");
        assert_eq!(input.output_directory, "C:\\out");
        assert_eq!(input.name_pattern, "{title} ({n})");
        assert_eq!(input.titles, vec![1, 2]);
        assert_eq!(input.names.get("2").map(String::as_str), Some("Second"));

        let titles = vec![(1u32, "One".to_string()), (2, "One".to_string())];
        let plan = windows::queue::plan(
            &input,
            &titles,
            "mp4",
            windows::queue::CollisionPolicy::Ask,
            output_file_exists,
        )
        .expect("plan");
        assert_eq!(
            plan,
            vec![
                (1, "C:\\out\\One (1).mp4".to_string()),
                (2, "C:\\out\\Second.mp4".to_string())
            ]
        );
    }

    #[test]
    fn a_metadata_change_arrives_as_camel_case_the_tab_accepts() {
        let payload = r#"{"action":"add","key":"comment","value":"mine","checked":false}"#;
        let change: MetadataInput = serde_json::from_str(payload).expect("metadata change");
        let mut settings = tabs::MetadataSettings::default();
        change
            .apply(&mut settings, &TitleFacts::default())
            .expect("add");
        assert!(settings.entries.iter().any(|entry| entry.key == "comment"));

        let mode = r#"{"action":"mode","mode":"edit"}"#;
        let change: MetadataInput = serde_json::from_str(mode).expect("mode change");
        change
            .apply(&mut settings, &TitleFacts::default())
            .expect("mode");
        assert!(settings.applies_edits());
    }

    #[test]
    fn a_preset_change_arrives_as_the_action_and_the_preset_it_names() {
        let payload = r#"{"action":"clone","id":"fast-1080p","name":"Mine",
            "description":"for the shelf","category":"web","default":true}"#;
        let input: PresetInput = serde_json::from_str(payload).expect("preset change");
        assert_eq!(input.action().expect("clone"), PresetAction::Clone);
        assert_eq!(input.id, "fast-1080p");
        assert_eq!(input.category, "web");
        assert!(input.default);
        assert!(PresetAction::Delete.is_destructive());
        assert!(PresetAction::Edit.needs_name());
        assert!(!PresetAction::SetDefault.needs_name());

        let view = windows::presets::form_view(&input, &["Fast 1080p".to_string()], None);
        assert!(view.can_submit, "{:?}", view.problems);
        assert_eq!(view.selected_category, "web");
        assert!(view.default);
    }

    #[test]
    fn the_output_folder_is_only_accepted_when_it_exists() {
        let (valid, problem) = check_output_directory("   ");
        assert!(!valid);
        assert!(problem.contains("folder"));

        let (valid, problem) = check_output_directory("C:\\definitely\\not\\here");
        assert!(!valid);
        assert!(problem.contains("definitely"), "{problem}");

        let dir = std::env::temp_dir().join("vise-desktop-output");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("dir");
        assert_eq!(
            check_output_directory(dir.to_str().unwrap()),
            (true, String::new())
        );
    }

    #[test]
    fn a_window_this_build_does_not_offer_is_refused_and_a_whole_close_is_not() {
        let mut surfaces = SurfaceState::default();
        assert!(surfaces.open("queue").is_ok());
        assert!(surfaces.open("registry editor").is_err());
        assert_eq!(surfaces.open_surfaces().len(), 1);
        assert!(!surfaces.open_surfaces().is_empty());
        surfaces.close_all();
        assert!(surfaces.open_surfaces().is_empty());

        let catalog = surfaces.catalog();
        assert_eq!(catalog.surfaces.len(), 10);
        assert!(catalog
            .surfaces
            .iter()
            .any(|surface| surface.id == preferences::SURFACE));
        assert!(catalog
            .surfaces
            .iter()
            .any(|surface| surface.id == windows::diagnostics::SURFACE));
        assert!(catalog
            .surfaces
            .iter()
            .all(|surface| surface.role == "dialog" && surface.modal));
    }

    #[test]
    fn the_alert_asks_a_question_even_when_the_page_names_nothing() {
        assert_eq!(
            alert_headline(AlertAction::RemoveJob, "", Some(7), None),
            "Remove from the queue job 7?"
        );
        assert_eq!(
            alert_headline(AlertAction::CancelJob, "", None, None),
            "Cancel the encode?"
        );
        assert_eq!(
            alert_headline(AlertAction::DeletePreset, "", None, Some("  ")),
            "Delete the preset?"
        );
        assert_eq!(
            alert_headline(AlertAction::DeletePreset, "", None, Some("user-1234")),
            "Delete the preset 'user-1234'?"
        );
        assert_eq!(
            alert_headline(AlertAction::ClearQueue, "", None, None),
            "Empty the queue?"
        );
        // A headline the page did write is kept, word for word.
        assert_eq!(
            alert_headline(
                AlertAction::CancelQueue,
                "Cancel 3 encodes and lose their progress?",
                None,
                None
            ),
            "Cancel 3 encodes and lose their progress?"
        );
        assert_eq!(
            alert_headline(AlertAction::StartQueue, "", None, None),
            "Start the queue?"
        );
    }

    #[test]
    fn a_preset_exchange_change_carries_the_file_it_reads_or_writes() {
        let payload = r#"{"action":"import","path":"C:\\out\\mine.json","category":"web"}"#;
        let change: PresetInput = serde_json::from_str(payload).expect("import change");
        let action = change.action().expect("import");
        assert_eq!(action, PresetAction::Import);
        assert_eq!(change.path, "C:\\out\\mine.json");
        assert!(action.needs_path(), "an import reads a file");

        let export = r#"{"action":"export","id":"fast-1080p","path":"C:\\out\\fast.json"}"#;
        let change: PresetInput = serde_json::from_str(export).expect("export change");
        let action = change.action().expect("export");
        assert_eq!(action, PresetAction::Export);
        assert_eq!(change.id, "fast-1080p");
        assert!(action.needs_path(), "an export writes a file");
        assert!(!action.needs_name(), "the exported preset is not renamed");
    }

    #[test]
    fn a_preset_file_that_is_not_there_is_refused_before_anything_is_written() {
        assert!(windows::presets::import_path_problem("C:\\nowhere\\mine.json").is_some());
        assert!(windows::presets::export_path_problem("C:\\nowhere\\mine.txt").is_some());
    }

    #[test]
    fn the_when_done_answer_only_arrives_once_the_queue_has_nothing_left_to_do() {
        let mut settings = PreferencesSettings::new();
        settings.when_done.action = preferences::whendone::WhenDoneAction::CloseWindow;
        settings.when_done.delay_seconds = 30;
        // Nothing is decided while there is still work, however the answer is set:
        // a window that closed itself with three jobs queued would be the wrong
        // answer to give to anybody who has not asked for it.
        for (pending, running) in [(0, 1), (1, 0), (3, 2), (1, 1)] {
            assert!(
                when_done_plan(&settings, pending, running).is_none(),
                "{pending} pending, {running} running"
            );
        }
        let plan = when_done_plan(&settings, 0, 0).expect("drained");
        assert_eq!(
            plan.action,
            preferences::whendone::WhenDoneAction::CloseWindow
        );
        assert_eq!(plan.delay_seconds, 30, "the wait is carried, not dropped");

        settings.when_done.action = preferences::whendone::WhenDoneAction::Nothing;
        assert!(
            when_done_plan(&settings, 0, 0).is_none(),
            "doing nothing is not a plan"
        );

        settings.when_done.action = preferences::whendone::WhenDoneAction::SignOut;
        assert!(
            when_done_plan(&settings, 0, 0).is_none(),
            "an action this build cannot do is never acted on, even from a stored record"
        );
    }

    #[test]
    fn asking_before_the_window_closes_and_closing_it_reach_the_same_question() {
        // Two spellings of one answer are two settings to learn for no reason, so
        // both reach the same countdown.
        let mut settings = PreferencesSettings::new();
        for action in [
            preferences::whendone::WhenDoneAction::Ask,
            preferences::whendone::WhenDoneAction::CloseWindow,
        ] {
            settings.when_done.action = action;
            let plan = when_done_plan(&settings, 0, 0).expect("a question");
            assert_eq!(plan.action, action);
            assert_eq!(plan.delay_seconds, 0);
        }
        assert_eq!(
            alert_headline(AlertAction::CloseWindow, "", None, None),
            "The queue has finished. Close this window?"
        );
        assert!(AlertAction::CloseWindow.is_destructive());
    }

    /// A theme the page reports is a fact about the media query, not a request.
    #[test]
    fn a_reported_system_theme_is_a_media_query_answer() {
        assert_eq!(SystemTheme::of_dark(true), SystemTheme::Dark);
        assert_eq!(SystemTheme::of_dark(false), SystemTheme::Light);
        // The page sends the boolean rather than a keyword, so nothing on this
        // side has to agree with a CSS spelling.
        let payload = r#"{"dark":true}"#;
        let report: serde_json::Value = serde_json::from_str(payload).expect("report");
        assert_eq!(
            SystemTheme::of_dark(report["dark"].as_bool().unwrap()),
            SystemTheme::Dark
        );
    }

    /// The main window's label is named once, and every read of the platform
    /// theme goes through it.
    #[test]
    fn the_appearance_event_has_a_name_the_page_can_subscribe_to() {
        assert_eq!(APPEARANCE_EVENT, "appearance-changed");
        assert_eq!(MAIN_WINDOW, "main");
    }

    /// Only a theme that actually changed is worth a line in the activity window.
    ///
    /// The page reports the media query once at start-up and again on every
    /// change, so the first report routinely repeats what the platform already
    /// said. Putting that in the log would fill the window with lines describing
    /// nothing that happened.
    #[test]
    fn a_report_that_moves_no_colours_is_not_logged() {
        let state: SharedState = Arc::new(Mutex::new(DesktopState::default()));
        let lines = |state: &SharedState| state.lock().expect("state").activity.view().lines.len();

        // The machine is dark and the window is following it. The page reports
        // the same dark query, which hands the window a stronger witness and
        // changes nothing on screen.
        {
            let mut guard = state.lock().expect("state");
            guard.appearance.observe_theme(SystemTheme::Dark, false);
        }
        let view = record_system_theme(&state, true).expect("report");
        assert_eq!(view.theme, "dark");
        assert_eq!(lines(&state), 0, "the colours did not move");

        // The query really does go dark, and this time the window follows it.
        let view = record_system_theme(&state, false).expect("report");
        assert_eq!(view.theme, "light");
        assert_eq!(lines(&state), 1, "the theme on screen changed");

        // And the machine switching back is worth the second line.
        let view = record_system_theme(&state, true).expect("report");
        assert_eq!(view.theme, "dark");
        assert_eq!(lines(&state), 2);
    }

    /// A forced theme ignores the machine, and so does the log.
    #[test]
    fn a_forced_theme_is_never_moved_by_a_report() {
        // A scratch settings file, because choosing a preference writes one and
        // a state built the default way has nowhere to put it.
        let dir = std::env::temp_dir().join("vise-desktop-forced-theme");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let settings_path = dir.join("settings.json");
        let state: SharedState = Arc::new(Mutex::new(DesktopState {
            appearance: AppearanceState::load(&settings_path, SystemTheme::Light, None),
            settings_path,
            ..DesktopState::default()
        }));
        {
            let mut guard = state.lock().expect("state");
            guard.appearance.set_theme_mode("light").expect("light");
        }
        for dark in [false, true, false, true] {
            let view = record_system_theme(&state, dark).expect("report");
            assert_eq!(view.theme, "light", "the user asked for light");
        }
        assert_eq!(state.lock().expect("state").activity.view().lines.len(), 0);
    }

    /// The page paints from the resolved pair, so the answer it is handed has to
    /// name the theme and the direction rather than the preference behind them.
    #[test]
    fn the_page_is_handed_the_theme_and_direction_it_paints_from() {
        let payload = r#"{
            "themeMode":"system",
            "directionMode":"auto",
            "theme":"dark",
            "direction":"rtl",
            "followsSystemTheme":true,
            "followsLocale":true,
            "systemTheme":"dark",
            "locale":"ar-EG",
            "language":"ar",
            "directionLanguage":"من اليمين إلى اليسار",
            "themeOptions":[
                {"id":"system","label":"Match the system","facts":"","available":true,"reason":"","selected":true},
                {"id":"light","label":"Light","facts":"","available":true,"reason":"","selected":false},
                {"id":"dark","label":"Dark","facts":"","available":true,"reason":"","selected":false}
            ],
            "directionOptions":[
                {"id":"auto","label":"Match my language","facts":"","available":true,"reason":"","selected":true},
                {"id":"ltr","label":"Left to right","facts":"","available":true,"reason":"","selected":false},
                {"id":"rtl","label":"Right to left","facts":"","available":true,"reason":"","selected":false}
            ],
            "summary":"The system asks for Dark, so the window will be Dark. / ar-EG reads right to left, so the window is laid out right to left.",
            "flowGlyph":"\u2192",
            "observationNote":"Your system is in its dark theme, reported by this window; your locale is ar-EG, reported by this window."
        }"#;
        let view: AppearanceViewModel = serde_json::from_str(payload).expect("appearance answer");
        assert_eq!(view.theme, "dark");
        assert_eq!(view.direction, "rtl");
        assert_eq!(view.system_theme, "dark");
        assert_eq!(view.locale, "ar-EG");
        assert_eq!(view.language, "ar");
        assert_eq!(view.theme_options.len(), 3);
        assert_eq!(view.direction_options.len(), 3);
        assert_eq!(view.flow_glyph, "\u{2192}");
        assert!(
            view.observation_note.contains("ar-EG"),
            "{}",
            view.observation_note
        );

        // The stylesheet keys its colours and its mirroring off exactly these two
        // values, so neither may be spelled any other way.
        assert!(["light", "dark"].contains(&view.theme.as_str()));
        assert!(["ltr", "rtl"].contains(&view.direction.as_str()));
    }

    /// A window whose preferences and appearance can actually be written.
    ///
    /// `DesktopState::default()` leaves both with nowhere to save to, which is right
    /// for a state that only answers questions and wrong for one that keeps a
    /// preference: a change that cannot be stored is refused rather than made, so
    /// these tests load both the way the window does at start-up.
    fn ready_state(name: &str) -> SharedState {
        let state = scratch_state(name);
        let mut guard = state.lock().expect("lock");
        let path = guard.settings_path.clone();
        guard.appearance = AppearanceState::load(&path, SystemTheme::Light, None);
        guard.preferences = PreferencesState::load(&path, &PortableSettings::default());
        drop(guard);
        state
    }

    #[test]
    fn every_name_the_page_asks_for_is_a_key_this_build_has() {
        // The page holds no text: an element names the message it wants and the
        // window's language answers. That only works while both sides agree on the
        // names, so the markup is checked against the catalog here rather than
        // leaving a rename on one side to be found on screen.
        let page = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../ui/index.html");
        let markup = std::fs::read_to_string(&page).expect("the page");
        let names: Vec<&str> = markup
            .match_indices("data-i18n=\"")
            .map(|(at, found)| {
                let rest = &markup[at + found.len()..];
                let end = rest.find('"').expect("a closed attribute");
                &rest[..end]
            })
            .collect();
        assert!(!names.is_empty(), "the page marks nothing at all");
        for name in &names {
            assert!(
                vise_core::MessageKey::of_name(name).is_some(),
                "the page asks for '{name}', which is not a key this build has"
            );
            assert!(!vise_core::Catalog::base()
                .text(vise_core::MessageKey::of_name(name).expect(name))
                .is_empty());
        }
        // And every shipped language answers every one of them.
        for table in vise_core::message_tables() {
            let resolved = vise_core::resolve_catalog(table.tag);
            for name in &names {
                let key = vise_core::MessageKey::of_name(name).expect(name);
                assert!(
                    resolved.catalog.message(key).source.is_translated(),
                    "{}: '{name}' falls back in a shipped locale",
                    table.tag
                );
            }
        }
    }

    /// A window whose configuration tabs hold work in progress.
    fn window_with_typed_input() -> SharedState {
        let state = ready_state("vise-desktop-language-switch");
        let mut guard = state.lock().expect("lock");
        guard.tabs.active = "dimensions".to_string();
        guard.tabs.dimensions.scale_width = tabs::common::NumField::of("1280");
        guard.tabs.dimensions.scale_height = tabs::common::NumField::of("720");
        guard.tabs.dimensions.aspect_ratio = "16:9".to_string();
        guard.tabs.video.encoder = "libx265".to_string();
        guard.tabs.subtitles.behaviour = "burn".to_string();
        guard.tabs.metadata.mode = "edit".to_string();
        drop(guard);
        state
    }

    #[test]
    fn a_language_switch_re_emits_the_view_models_and_keeps_what_was_typed() {
        let state = window_with_typed_input();
        let before = {
            let guard = state.lock().expect("lock");
            guard.tabs.clone()
        };
        let change = apply_language(&state, "ar").expect("arabic is a language");
        assert_eq!(change.language.locale, "ar");
        assert_eq!(change.language.language, "العربية");
        assert_eq!(change.appearance.direction, "rtl");
        assert_eq!(change.appearance.language, "ar");
        // The configuration tabs are not in the payload at all: they are what the
        // user has half filled in, and a payload that could redraw them is a
        // payload that loses the work.
        let payload = serde_json::to_value(&change).expect("payload");
        assert!(payload.get("tabs").is_none(), "{}", payload);
        assert!(payload.get("language").is_some());
        assert!(payload.get("appearance").is_some());
        assert!(payload.get("preferences").is_some());
        let after = {
            let guard = state.lock().expect("lock");
            guard.tabs.clone()
        };
        assert_eq!(after, before, "nothing about the tabs moved");
        assert_eq!(after.dimensions.scale_width.text, "1280");
        assert_eq!(after.video.encoder, "libx265");
        assert_eq!(after.metadata.mode, "edit");
    }

    #[test]
    fn a_pinned_direction_is_not_moved_by_a_language_change() {
        let state = ready_state("vise-desktop-language-pinned");
        {
            let mut guard = state.lock().expect("lock");
            guard
                .appearance
                .set_direction_mode("ltr")
                .expect("the direction is kept");
        }
        let change = apply_language(&state, "ar").expect("arabic is a language");
        assert_eq!(
            change.language.direction, "rtl",
            "the language reads this way"
        );
        assert!(!change.language.follows_language);
        assert_eq!(
            change.appearance.direction, "ltr",
            "and the layout still does not, because the user pinned it"
        );
    }

    #[test]
    fn a_language_the_build_does_not_ship_still_opens_a_window() {
        let state = ready_state("vise-desktop-language-unshipped");
        let change = apply_language(&state, "kl").expect("an unshipped language resolves");
        assert_eq!(change.language.locale, "en");
        assert!(
            change.language.summary.contains("kl"),
            "{}",
            change.language.summary
        );
        assert_eq!(change.appearance.language, "en");
    }

    #[test]
    fn the_language_the_record_holds_is_the_one_the_picker_shows() {
        let state = ready_state("vise-desktop-language-record");
        {
            let mut guard = state.lock().expect("lock");
            guard
                .preferences
                .set(preferences::general::LANGUAGE, &serde_json::json!("ja"))
                .expect("the preference is accepted");
        }
        let applied = apply_language(&state, "ja").expect("japanese");
        assert_eq!(applied.language.locale, "ja");
        let guard = state.lock().expect("lock");
        assert_eq!(
            guard.preferences.settings().general.language.tag(),
            "ja",
            "and the record still holds it"
        );
        assert_eq!(guard.appearance.language(), "ja");
    }
    /// A preset built for a machine this one does not resemble still has to reach
    /// the output settings rather than being half-applied, because a preset that
    /// silently sets nothing is a control that lies about what it did.
    fn scratch_state(name: &str) -> SharedState {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("temp dir");
        let settings_path = dir.join("settings.json");
        Arc::new(Mutex::new(DesktopState {
            settings_path,
            ..DesktopState::default()
        }))
    }

    #[test]
    fn applying_a_catalog_preset_puts_what_it_sets_onto_the_tabs() {
        let state = scratch_state("vise-desktop-apply-catalog-preset");
        for (id, container, video, audio, rate) in [
            (
                "general-super-hq-720-30-h264",
                "mp4",
                "h264",
                "aac",
                "quality",
            ),
            // The professional codecs and uncompressed audio are what the tab
            // pickers were extended for: a preset that asks for them has to be
            // able to say so on the tabs rather than quietly changing nothing.
            (
                "professional-prores-422-hq",
                "mkv",
                "prores",
                "pcm",
                "quality",
            ),
            ("professional-ffv1-flac", "mkv", "ffv1", "flac", "lossless"),
            ("matroska-vp9-1080-30", "mkv", "vp9", "opus", "quality"),
            ("hardware-qsv-hevc-1080-30", "mp4", "h265", "aac", "quality"),
        ] {
            let preset = vise_core::builtin_preset(id)
                .unwrap_or_else(|| panic!("'{id}' is not a preset this build carries"));
            apply_preset_to_tabs(&state, &preset.preset)
                .unwrap_or_else(|error| panic!("'{id}' could not be applied: {error}"));
            let guard = state.lock().expect("state");
            assert_eq!(guard.tabs.summary.container, container, "{id}");
            assert_eq!(guard.tabs.summary.video_codec, video, "{id}");
            assert_eq!(guard.tabs.summary.audio_codec, audio, "{id}");
            assert_eq!(guard.tabs.video.rate_control, rate, "{id}");
        }

        // A size-targeted preset is refused on a title whose duration nothing
        // reports, and says why: a file size cannot be aimed at without knowing
        // how long the title is, and pretending otherwise would produce a file of
        // the wrong length rather than a wrong number on the tab.
        let social = vise_core::builtin_preset("web-social-10mb-60s").expect("a social preset");
        let refused = apply_preset_to_tabs(&state, &social.preset).expect_err("no duration");
        assert!(
            refused.contains("source duration"),
            "'{refused}' does not say what is missing"
        );
        // What a preset sets that is not a container or a codec reaches the tabs
        // as well, so applying it twice is applying the same thing.
        {
            let mut guard = state.lock().expect("state");
            guard.tabs.video.preset = String::new();
        }
        let preset = vise_core::builtin_preset("general-super-hq-720-30-h264").expect("a preset");
        apply_preset_to_tabs(&state, &preset.preset).expect("apply");
        assert_eq!(
            state.lock().expect("state").tabs.video.preset,
            "slow",
            "the encoder speed the family is named for reaches the tab"
        );
        let _ =
            std::fs::remove_dir_all(std::env::temp_dir().join("vise-desktop-apply-catalog-preset"));
    }

    #[test]
    fn a_preset_saved_from_the_tabs_carries_what_the_tabs_hold() {
        let state = scratch_state("vise-desktop-preset-from-tabs");
        {
            let mut guard = state.lock().expect("state");
            guard.tabs.summary.container = "mkv".to_string();
            guard.tabs.summary.video_codec = "h265".to_string();
            guard.tabs.summary.audio_codec = "flac".to_string();
            guard.tabs.video.rate_control = "target_size".to_string();
            guard.tabs.video.target_size = tabs::common::NumField::of(25);
            guard.tabs.video.preset = "slow".to_string();
            guard.tabs.video.tune = "film".to_string();
        }
        let input = windows::presets::PresetInput {
            name: "Mine".to_string(),
            ..Default::default()
        };
        let spec = preset_from_tabs(&state, &input).expect("a preset from the tabs");
        assert_eq!(
            spec.encoder,
            Some(vise_core::EncoderId::X265),
            "the codec the tabs hold names the encoder that writes it"
        );
        assert_eq!(
            spec.encoding_mode.map(|mode| mode.is_target_size()),
            Some(true)
        );
        assert_eq!(
            spec.target_size,
            Some(25_000_000),
            "the size is in bytes, in the megabytes the queue already uses"
        );
        assert_eq!(
            spec.video_encoding.as_ref().and_then(|e| e.preset.clone()),
            Some("slow".to_string())
        );
        assert_eq!(spec.name.as_deref(), Some("Mine"));
        let _ = std::fs::remove_dir_all(std::env::temp_dir().join("vise-desktop-preset-from-tabs"));
    }
}
