# Graph Report - vise  (2026-10-20)

## Corpus Check
- 228 files · ~505,838 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 16 file(s) not represented in the graph (top: (none) 7, .toml 5, .cmd 1)

## Summary
- 6744 nodes · 16122 edges · 254 communities (234 shown, 20 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 268 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `2efd4680`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Apple Design Skill
- live-browser.js
- Frame-level Smoothness
- Design Process (prototype, test)
- Eight Design Principles
- Initial Response
- Materials & Depth (translucency)
- Multimodal Feedback (motion + sound + haptics)
- Rubber-banding (soft boundaries)
- Spatial Consistency (symmetric paths, anchored origins)
- initPageChat
- src-tauri/src/main.rs
- modern-screenshot.umd.js
- HardwareApi
- SubtitleProcessingOptions
- MetadataProcessingOptions
- el
- connectSSE
- handleManualEditActivity
- applyEditing
- AudioProcessingOptions
- media.rs
- ScanResult
- Session
- new-work.md
- scan.rs
- MessageKey
- live.md
- CoreError
- Vise Transcoder — Roadmap
- Responsive Design
- pipeline.rs
- onboard.md
- createLiveBrowserDomHelpers
- SKILL.md
- The Toolkit
- onAnnotDown
- vise-core/src/lib.rs
- JobSpec
- createLiveBrowserSessionState
- server.rs
- tabs/video.rs
- VideoProcessingOptions
- preferences/mod.rs
- animate.md
- Handle `generate`
- settings_versioning.rs
- AudioCodec
- Generate Report
- showToast
- catalog.rs
- New visual work
- optimize.md
- mountSvelteComponentVariant
- ops.rs
- geometry.rs
- Impeccable Asset Producer
- document.md
- compatibility.rs
- critique.md
- Simplify the Design
- Hardening Dimensions
- WorkerClient
- timespan.rs
- initGlobalBar
- AGENTS.md
- clarify.md
- Nielsen's 10 Heuristics
- Generate Combined Critique Report
- windows/log.rs
- polish.md
- quieter.md
- handleKeyDown
- numa.rs
- Init flow
- WorkerSupervisor
- HardwareCapabilities
- pathbuf
- Common Cognitive Load Violations
- iOS platform
- Shape
- portable.rs
- video_processing.rs
- platform.rs
- validation.rs
- Android platform
- dialogs.rs
- Persona-Based Design Testing
- hdr.rs
- package.json
- worker_api.rs
- LogService
- Generate Report
- Cognitive Load Assessment
- Impeccable Finish Reviewer
- app.js
- live-browser-ignores.js
- vise-ipc/src/lib.rs
- Vise Transcoder {VERSION} Release Notes
- src/optical.rs
- Visualize: Direction Comps & Asset Production
- preset_catalog.rs
- Project Context
- Reproducible Build Process
- contract.rs
- capability.rs
- .view
- advanced.rs
- Heuristics Scoring Guide
- preview.rs
- bundle.rs
- dimensions.rs
- tooling.rs
- [0.1.0] - 2026-10-19
- router.rs
- Route
- Vise Desktop
- vise-cli/src/main.rs
- vise-core
- DesktopState
- crate::Transcoder
- http.rs
- supervisor.rs
- SubtitleFormat
- crash.rs
- Local Setup
- Vise Transcoder
- encode.rs
- runtime.rs
- LogLevel
- actOnAgentTarget
- DirectionMode
- tests/subtitles.rs
- token.rs
- vise-worker/src/main.rs
- i18n.rs
- SharedState
- localization.rs
- actions.rs
- queue.rs
- window_with
- preferences/updates.rs
- viewmodel.rs
- tabs/audio.rs
- general.rs
- preferences/hardware.rs
- appearance.rs
- output.rs
- State
- diskspace.rs
- TitleFacts
- windows/diagnostics.rs
- sources.rs
- EncoderId
- whendone.rs
- notify.rs
- filters.rs
- enclog.rs
- Notification
- persistence.rs
- Job
- DiagnosticsViewModel
- tabs/metadata.rs
- tabs/mod.rs
- sweep.rs
- chapter.rs
- results.rs
- source.rs
- Locale
- tray.rs
- scanflow.rs
- Integration
- system/updates.rs
- chapters.rs
- src/power.rs
- locale.rs
- windows/presets.rs
- SystemTheme
- Catalog
- super
- form_view
- load_settings
- tests/optical.rs
- .apply
- RangeMode
- begin_scan_with
- JobRun
- SubtitleCodec
- output_and_chapters.rs
- roadmap.rs
- system/power.rs
- AppcastError
- windows/mod.rs
- serialize
- want_one_of
- synced
- AppSettings
- TrayAction
- ContainerFormat
- free_space
- CurlSource
- optical_end_to_end.rs
- IsolationPolicy
- L
- ne
- CheckOutcome
- Extract Flow
- ScanPhase
- SweepProgress
- track_selection
- ChapterProcessingOptions
- partial.rs
- PresetCategory
- media_depth.rs
- StorageKind
- MessageFileVisitor
- PreferenceTabViewModel
- TrackKind
- Diagnostic Scan
- ChapterFormat
- PresetSpec
- read_settings
- AppcastItem
- fetch-media-tools.sh
- VideoCodec
- build-release.sh
- fmt
- Milestones
- verify-media-tools.sh
- Operate mode depth (and Read notes)
- ChapterHandling
- ClaimStop
- TokenRejection
- QueueActionViewModel
- Product
- .parse_element
- .detect_crop
- decide
- Design System
- build-linux.sh
- check-linux.sh
- check-linux-package.sh
- check-linux-layout.sh
- fetch-macos-digests.sh
- SourceError
- HardwareFamily
- subtitle.rs
- WorkerDeath
- Resolution
- PortableSettings
- Vise Transcoder 0.1.0
- move_entry_by
- release-notes.sh

## God Nodes (most connected - your core abstractions)
1. `JobSpec` - 141 edges
2. `CoreError` - 114 edges
3. `MessageKey` - 108 edges
4. `TitleFacts` - 60 edges
5. `WorkerClient` - 52 edges
6. `call()` - 51 edges
7. `PresetSpec` - 50 edges
8. `el()` - 48 edges
9. `ClientError` - 45 edges
10. `LogService` - 45 edges

## Surprising Connections (you probably didn't know these)
- `Output Contract` --references--> `failed()`  [INFERRED]
  .agents/skills/impeccable/reference/degraded/manual-edit-applier.md → apps/vise-desktop/ui/app.js
- `Group 18 — Localization` --references--> `de()`  [INFERRED]
  docs/ROADMAP.md → .agents/skills/impeccable/scripts/modern-screenshot.umd.js
- `Project` --references--> `main()`  [INFERRED]
  AGENTS.md → scripts/make-icon.py
- `Step 2: Act by severity` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/doctor.md → .agents/skills/impeccable/scripts/live-browser.js
- `What this owns, and what it does not` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/doctor.md → .agents/skills/impeccable/scripts/live-browser.js

## Import Cycles
- 1-file cycle: `crates/vise-ipc/src/numa.rs -> crates/vise-ipc/src/numa.rs`
- 2-file cycle: `crates/vise-cli/src/cli.rs -> crates/vise-cli/src/ops.rs -> crates/vise-cli/src/cli.rs`
- 2-file cycle: `apps/vise-desktop/src-tauri/src/system/mod.rs -> apps/vise-desktop/src-tauri/src/system/power.rs -> apps/vise-desktop/src-tauri/src/system/mod.rs`
- 2-file cycle: `apps/vise-desktop/src-tauri/src/system/mod.rs -> apps/vise-desktop/src-tauri/src/system/tray.rs -> apps/vise-desktop/src-tauri/src/system/mod.rs`
- 2-file cycle: `apps/vise-desktop/src-tauri/src/preferences/mod.rs -> apps/vise-desktop/src-tauri/src/preferences/output.rs -> apps/vise-desktop/src-tauri/src/preferences/mod.rs`
- 2-file cycle: `crates/vise-core/src/catalog.rs -> crates/vise-core/src/preset.rs -> crates/vise-core/src/catalog.rs`

## Communities (254 total, 20 thin omitted)

### Community 0 - "Apple Design Skill"
Cohesion: 0.21
Nodes (13): Apple Design Skill, Damping Ratio, Direct Manipulation (1:1 tracking), Four Human Needs (safety/predictability, understanding, achievement, joy), Interruptibility, Momentum Projection, Pointer Events / setPointerCapture, Reduced Motion & Accessibility (+5 more)

### Community 1 - "live-browser.js"
Cohesion: 0.03
Nodes (138): applyGlobalBarLabelState(), applyParamDefaults(), applyParamValue(), applyPlaceholderSizingStyles(), averageRgb01(), bindEditBadgeProxy(), bufferToBase64(), buildAnnotationsForCapture() (+130 more)

### Community 10 - "initPageChat"
Cohesion: 0.09
Nodes (47): armPageChatForTyping(), attachSteerFocusDebug(), attachSteerFocusGuard(), clearSteerAwaitTimer(), clearSteerFocusRecoverTimer(), collapsePageChat(), configureVoiceContext(), expandPageChat() (+39 more)

### Community 11 - "src-tauri/src/main.rs"
Cohesion: 0.03
Nodes (62): AppearanceViewModel, a_dropped_path_list_is_answered_with_accepted_and_refused_entries(), a_forced_theme_is_never_moved_by_a_report(), a_job_the_user_stopped_is_cancelled_and_never_counted_against_the_worker(), a_language_switch_re_emits_the_view_models_and_keeps_what_was_typed(), a_language_the_build_does_not_ship_still_opens_a_window(), a_partial_the_sweep_cannot_remove_is_named_rather_than_hidden(), a_pinned_direction_is_not_moved_by_a_language_change() (+54 more)

### Community 12 - "modern-screenshot.umd.js"
Cohesion: 0.15
Nodes (31): ae(), be(), de(), dt(), _e(), er(), fe(), ft() (+23 more)

### Community 13 - "HardwareApi"
Cohesion: 0.07
Nodes (24): HardwareAcceleration, HardwareApi, Amf, Auto, Cuda, D3d11va, Dxva2, Nvenc (+16 more)

### Community 14 - "SubtitleProcessingOptions"
Cohesion: 0.20
Nodes (5): to_job_processing(), build_subtitle_filter(), build_subtitle_filter_chain(), build_subtitles(), SubtitleProcessingOptions

### Community 15 - "MetadataProcessingOptions"
Cohesion: 0.19
Nodes (5): build_metadata(), build_metadata_filter(), build_metadata_filter_chain(), MetadataProcessingOptions, MetadataTrackConfig

### Community 16 - "el"
Cohesion: 0.07
Nodes (57): actionLabel(), applyConfigureBarChrome(), bindConfigureCountPillTooltip(), bindConfigureInlineControlHover(), bindConfigureModifierPillHover(), buildConfigureActionControl(), buildConfigureCountControl(), buildConfigureRow() (+49 more)

### Community 17 - "connectSSE"
Cohesion: 0.08
Nodes (86): abortSvelteComponentInjection(), applySavedSessionMeta(), cancelEditing(), cancelEditingToPicking(), cleanup(), cleanupAcceptedSession(), clearHandled(), clearMountErrorCard() (+78 more)

### Community 18 - "handleManualEditActivity"
Cohesion: 0.19
Nodes (24): clearStoredManualApplyState(), fetchPendingCount(), handleManualEditActivity(), hidePendingApplyDock(), manualApplyLoadingText(), manualApplyStateKey(), manualEditEventForCurrentPage(), numberOrNull() (+16 more)

### Community 19 - "applyEditing"
Cohesion: 0.06
Nodes (43): addManualContextText(), applyEditing(), buildLocatorForLeaf(), canRestoreManualEditElement(), collectEditableTextRows(), visit(), collectManualContextPieces(), walk() (+35 more)

### Community 20 - "AudioProcessingOptions"
Cohesion: 0.08
Nodes (21): AudioMixdown, Auto, Dolby, Mono, Stereo, Surround51, Surround71, AudioMode (+13 more)

### Community 21 - "media.rs"
Cohesion: 0.08
Nodes (24): application_root(), ChapterInfo, convert_chapter(), convert_format(), convert_stream(), encoder_input_arguments(), FfprobeChapter, FfprobeFormat (+16 more)

### Community 22 - "ScanResult"
Cohesion: 0.13
Nodes (19): ScanResult, encoder_is_intra_only(), ensure_valid(), known_duration(), MachineFacts, reports_missing_paths(), source_audio_channels(), source_geometry() (+11 more)

### Community 23 - "Session"
Cohesion: 0.14
Nodes (4): Session, session_add_and_read(), session_remove_missing_errors(), SessionSnapshot

### Community 24 - "new-work.md"
Cohesion: 0.05
Nodes (34): Recommended Actions, Craft (deprecated alias), Impeccable Documenter, Input Contract, Output Contract, Workflow, Monorepo notes, Opting out of the boot check (+26 more)

### Community 25 - "scan.rs"
Cohesion: 0.07
Nodes (28): StreamInfo, ScanCommand, ScanProgress, WorkDone, WorkState, ScanDone, Unknown, WorkDone (+20 more)

### Community 26 - "MessageKey"
Cohesion: 0.02
Nodes (95): MessageKey, ActionAddSource, ActionAddToQueue, ActionOpenPreferences, ActionOpenPresets, ActionOpenPreview, ActionOpenQueue, ActionScan (+87 more)

### Community 27 - "live.md"
Cohesion: 0.06
Nodes (33): Apply at system scale, Audit before choosing, Choose a strategy, Contrast and perception, Live-mode signature params, Verify, Visitor mode, Step 1: Parse the request (+25 more)

### Community 28 - "CoreError"
Cohesion: 0.08
Nodes (42): CoreError, CommandFailed, Invalid, InvalidMedia, Io, JobNotFound, MissingTool, Parse (+34 more)

### Community 29 - "Vise Transcoder — Roadmap"
Cohesion: 0.07
Nodes (28): Architecture decision snapshot, Deferred / Future Work, Group 0 — Project foundation, Group 10 — Encoding control and job reliability, Group 11 — Hardware acceleration, Group 12 - Platform packaging and release, Group 13 — Worker process and JSON IPC, Group 14 — Desktop parity with reference product (+20 more)

### Community 30 - "Responsive Design"
Cohesion: 0.06
Nodes (32): Assess Adaptation Challenge, Breakpoints: Content-Driven, Content Adaptation, Desktop Adaptation (Mobile → Desktop), Detect Input Method, Not Just Screen Size, Email Adaptation (Web → Email), Implement Adaptations, Layout Adaptation Patterns (+24 more)

### Community 31 - "pipeline.rs"
Cohesion: 0.12
Nodes (10): audio_encoder(), chapter_sidecar_path(), infer_container(), merge_video_options(), parse_codecs(), transcode(), transcode_file(), TranscodePlan (+2 more)

### Community 32 - "onboard.md"
Cohesion: 0.09
Nodes (22): Assess Onboarding Needs, Context Over Ceremony, Contextual Help, Design Onboarding Experiences, Documentation & Help, Empty State Design, Feature Discovery & Adoption, Guided Tours & Walkthroughs (+14 more)

### Community 33 - "createLiveBrowserDomHelpers"
Cohesion: 0.16
Nodes (13): createLiveBrowserDomHelpers(), cloneWithoutChrome(), cssId(), liveUiRoot(), makeFrozenAnchor(), raiseHitModal(), rectIsUsableAnchor(), syncTopLayerHost() (+5 more)

### Community 34 - "SKILL.md"
Cohesion: 0.08
Nodes (21): Before you finish, Scope is sovereign, The amplification, The skeleton test, Why it reads flat, Craft floor, Refuse, Verify (+13 more)

### Community 35 - "The Toolkit"
Cohesion: 0.10
Nodes (20): Animate complex properties, Assess What "Extraordinary" Means Here, For data-heavy interfaces, For functional UI, For performance-critical UI, For visual/marketing surfaces, Implement with Discipline, Interact with the device (+12 more)

### Community 36 - "onAnnotDown"
Cohesion: 0.12
Nodes (24): applyPlaceholderDimensions(), beginEditPin(), buildPlaceholderResizeHandles(), cancelEditingPin(), clampPlaceholderSize(), cursorForPlaceholderEdge(), finalizeEditingPin(), initAnnotOverlay() (+16 more)

### Community 37 - "vise-core/src/lib.rs"
Cohesion: 0.06
Nodes (4): a_sweep_is_plannable_and_runnable_from_outside_the_crate(), request(), source(), the_bounds_are_reachable_as_constants_and_are_enforced()

### Community 38 - "JobSpec"
Cohesion: 0.06
Nodes (26): a_job_is_judged_before_it_is_queued_and_every_problem_is_reported(), add_job(), check_job(), judge_job(), refusal(), validate_job_spec(), EncodingMode, Abr (+18 more)

### Community 39 - "createLiveBrowserSessionState"
Cohesion: 0.22
Nodes (15): createLiveBrowserSessionState(), clearHandled(), clearScrollY(), clearSession(), isHandled(), loadSession(), markHandled(), nextCheckpointRevision() (+7 more)

### Community 40 - "server.rs"
Cohesion: 0.06
Nodes (15): builds_loopback_addresses_only(), DEFAULT_TIMEOUT, serialises_command_bodies(), BIND_ADDRESS, bind_listener(), binds_loopback_only_and_reports_its_port(), falls_back_when_the_requested_port_is_busy(), handle_connection() (+7 more)

### Community 41 - "tabs/video.rs"
Cohesion: 0.08
Nodes (31): rate_control(), a_bitrate_mode_wants_a_bit_rate_and_offers_two_pass(), a_fixed_frame_rate_needs_a_sane_number(), a_quality_value_outside_the_range_is_refused(), a_source_with_hdr_metadata_opens_both_hdr_rows(), a_target_size_needs_a_duration_to_work_out_from(), a_valid_change_is_applied(), an_hdr_choice_reaches_the_job_and_an_unknown_one_is_refused() (+23 more)

### Community 42 - "VideoProcessingOptions"
Cohesion: 0.11
Nodes (10): build_video_filter_chain(), ColourRange, Full, Limited, FrameRateMode, Auto, Constant, Variable (+2 more)

### Community 43 - "preferences/mod.rs"
Cohesion: 0.06
Nodes (51): a_change_is_validated_before_it_is_written(), a_change_reaches_the_code_paths_that_consume_it(), a_decode_device_cannot_be_named_before_the_media_tools_have_been_looked_for(), a_decode_device_this_machine_does_not_have_is_refused_rather_than_stored(), a_document_with_no_record_adopts_the_folder_it_already_named(), a_folder_that_has_not_been_chosen_is_shown_rather_than_refusing_everything_else(), a_record_from_a_newer_build_is_read_never_written_and_never_lost(), a_record_that_is_not_readable_gives_the_defaults_and_is_left_alone() (+43 more)

### Community 44 - "animate.md"
Cohesion: 0.12
Nodes (14): Accessibility and control, Choose material by meaning, Find the job, Implement to the runtime, Set the motion thesis, Timing and easing, Verify, Visitor mode (+6 more)

### Community 45 - "Handle `generate`"
Cohesion: 0.12
Nodes (16): 1. Read the screenshot (if present), 2. Wrap the element, 3. Load the action's reference, 4. Plan three variants: identity first, then mode, then axes, 5. Apply the freeform prompt (if present), 6. Deliver variants, 7. Parameters (composition-sized, 0-4 per variant), 8. Signal done (+8 more)

### Community 46 - "settings_versioning.rs"
Cohesion: 0.12
Nodes (4): applying_preset_changes_job_fields(), roundtrip_import_export(), tmp_path(), user_create_edit_delete_persists()

### Community 47 - "AudioCodec"
Cohesion: 0.10
Nodes (26): audio_codec_of(), audio_label(), AudioCodec, Aac, Ac3, Eac3, Flac, Mp3 (+18 more)

### Community 48 - "Generate Report"
Cohesion: 0.13
Nodes (14): 1. Accessibility (A11y), 2. Performance, 3. Theming, 4. Responsive Design, 5. Implementation Integrity (CRITICAL), Audit Health Score, Detailed Findings by Severity, Diagnostic Scan (+6 more)

### Community 49 - "showToast"
Cohesion: 0.15
Nodes (17): abandonForeignSession(), abandonSupersededGo(), captureAndEmit(), checkpointPayload(), compileShader(), discardOrphanedSession(), handleDiscard(), markSessionHandled() (+9 more)

### Community 50 - "catalog.rs"
Cohesion: 0.07
Nodes (48): a_codec_entry_is_migrated_and_an_unknown_one_is_refused(), a_copy_is_independent_of_the_preset_it_was_copied_from(), a_hardware_encoder_this_machine_has_is_used_as_written(), a_hardware_encoder_this_machine_lacks_becomes_its_software_stand_in(), a_software_preset_needs_no_hardware_to_answer_for(), builtin_catalog(), BUILTIN_CATALOG_JSON, builtin_entries_in() (+40 more)

### Community 51 - "New visual work"
Cohesion: 0.10
Nodes (19): 1. Decide what is already true, 2. Ask what will change the work, 3. Choose the right amount of invention, 4. Commit the world, 5. Record the decision, 6. Build with full commitment, 7. Inspect and finish, Both paths (+11 more)

### Community 52 - "optimize.md"
Cohesion: 0.14
Nodes (13): Animation Performance, Assess Performance Issues, Core Web Vitals Optimization, Cumulative Layout Shift (CLS < 0.1), Interaction to Next Paint (INP < 200ms), Largest Contentful Paint (LCP < 2.5s), Loading Performance, Network Optimization (+5 more)

### Community 53 - "mountSvelteComponentVariant"
Cohesion: 0.10
Nodes (35): acceptedDomAlreadyClean(), applyOriginalAttrsToSvelteAnchor(), clearHandledWrapperReloadStamp(), commitAcceptedSvelteComponentToDom(), componentModuleCandidates(), deferredRecoverySuperseded(), describeMountFailure(), detectDevServerBase() (+27 more)

### Community 54 - "ops.rs"
Cohesion: 0.11
Nodes (20): BatchArgs, Cli, Commands, Batch, Presets, Transcode, parse(), PresetCommands (+12 more)

### Community 55 - "geometry.rs"
Cohesion: 0.21
Nodes (5): best_rational(), display_aspect_nonsquare_par(), display_aspect_ratio(), gcd(), reduce_ratio()

### Community 56 - "Impeccable Asset Producer"
Cohesion: 0.15
Nodes (11): Act on the receipt, Assemble and review, Plan and asset review, Plan, capture, serve, Core Rule, Decision Comps, Impeccable Asset Producer, Input Contract (+3 more)

### Community 57 - "document.md"
Cohesion: 0.08
Nodes (23): Component translation rules, Narrative mapping, Pitfalls, Scan mode (approach C: auto-extract, then confirm descriptive language), Schema, Seed mode, Step 1: Find the design assets, Step 1: Route through new-work's workshop (+15 more)

### Community 58 - "compatibility.rs"
Cohesion: 0.09
Nodes (34): apply_common(), AudioDefaultsInput, Change, rules_view(), ChoiceOption, audio_codec_availability_follows_the_container(), audio_codec_options(), audio_codecs_for() (+26 more)

### Community 59 - "critique.md"
Cohesion: 0.17
Nodes (11): Action Summary, Ask the User, Assessment A: Design Review, Assessment B: Detector + Browser Evidence, Assessment Orchestration, Deliver the Report, Hard Invariants, Persist the Snapshot (+3 more)

### Community 60 - "Simplify the Design"
Cohesion: 0.17
Nodes (11): Assess Current State, Code Simplification, Content Simplification, Document Removed Complexity, Information Architecture, Interaction Simplification, Layout Simplification, Plan Simplification (+3 more)

### Community 61 - "Hardening Dimensions"
Cohesion: 0.17
Nodes (11): Accessibility Resilience, Assess Hardening Needs, Edge Cases & Boundary Conditions, Error Handling, Hardening Dimensions, Input Validation & Sanitization, Internationalization (i18n), Performance Resilience (+3 more)

### Community 62 - "WorkerClient"
Cohesion: 0.06
Nodes (30): done_report(), scanning_report(), DEFAULT_POLL_INTERVAL, DEFAULT_POLL_TIMEOUT, describe(), is_success(), is_terminal(), poll_encode_until() (+22 more)

### Community 63 - "timespan.rs"
Cohesion: 0.07
Nodes (18): duration_round_trip(), negative_spans_are_accepted_as_zero(), parse_clock_field(), parses_clock_only_forms(), parses_day_components_and_fractions(), serde_round_trip_and_error_message(), TimeSpan, .ZERO (+10 more)

### Community 64 - "initGlobalBar"
Cohesion: 0.07
Nodes (52): agentHasWorkInFlight(), agentStatusText(), applyLiveBarPreference(), barPaletteForTheme(), brandMarkSvg(), buildDesignHeader(), buildSteerQueueHint(), cursorForInsertAxis() (+44 more)

### Community 65 - "AGENTS.md"
Cohesion: 0.07
Nodes (23): Architecture Non-Negotiables, Autonomous Orchestration Mode, Branch Lifecycle, Build, Test, and Development Commands, Git Workflow, Graphify, Obsidian / Cross-Project Knowledge, Project (+15 more)

### Community 66 - "clarify.md"
Cohesion: 0.18
Nodes (10): Actions and navigation, Audit the language, Errors and permissions, Forms, Help and instructional text, Loading, empty, and success states, Rewrite by function, Set the message hierarchy (+2 more)

### Community 67 - "Nielsen's 10 Heuristics"
Cohesion: 0.18
Nodes (11): 10. Help and Documentation, 1. Visibility of System Status, 2. Match Between System and Real World, 3. User Control and Freedom, 4. Consistency and Standards, 5. Error Prevention, 6. Recognition Rather Than Recall, 7. Flexibility and Efficiency of Use (+3 more)

### Community 68 - "Generate Combined Critique Report"
Cohesion: 0.18
Nodes (11): Design Health Score, Design Specificity Verdict, Generate Combined Critique Report, Minor Observations, Overall Impression, Persona Red Flags, Priority Issues, Questions to Consider (+3 more)

### Community 69 - "windows/log.rs"
Cohesion: 0.13
Nodes (32): a_cleared_worker_buffer_is_a_reset_too(), a_filter_level_this_build_does_not_name_is_refused(), a_page_with_no_session_is_treated_as_no_information_rather_than_as_a_reset(), a_stream_keeps_the_order_the_worker_sent_and_asks_only_for_what_is_new(), a_window_that_cannot_keep_every_line_says_so_instead_of_growing(), a_worker_that_dropped_older_lines_says_so_and_keeps_saying_so(), a_worker_that_starts_numbering_again_is_not_waited_on_forever(), ActivityViewModel (+24 more)

### Community 70 - "polish.md"
Cohesion: 0.18
Nodes (10): 1. Establish the system, 2. Gather the evidence, 3. Triage, 4. Polish the whole path, 5. Verify and finish, Color, imagery, and icons, Content and code, Flow and hierarchy (+2 more)

### Community 71 - "quieter.md"
Cohesion: 0.18
Nodes (10): Assess Current State, Color Refinement, Composition Refinement, Motion Reduction, Plan Refinement, Refine the Design, Simplification, Verify Quality (+2 more)

### Community 72 - "handleKeyDown"
Cohesion: 0.14
Nodes (31): beginNewLiveConfiguration(), cancelInsertConfigure(), clearAnnotations(), clearInsertPicking(), closeTunePopover(), exitConfigureToPicking(), handleClick(), handleKeyDown() (+23 more)

### Community 73 - "numa.rs"
Cohesion: 0.05
Nodes (43): a_group_that_does_exist_is_honoured_and_the_mode_reads_back(), a_group_that_does_not_exist_is_refused_by_name(), a_machine_that_cannot_be_asked_is_never_pinned(), apply(), applying_a_pin_reports_the_platform_rather_than_pretending(), ascii(), available(), GetCurrentThread() (+35 more)

### Community 74 - "Init flow"
Cohesion: 0.20
Nodes (10): Completion gate, Init flow, Step 1: Load current state, Step 2: Explore the project, Step 3: Interview for product truth, Step 4: Write PRODUCT.md, Step 5: Record workflow defaults, Step 6: Wrap up or resume (+2 more)

### Community 75 - "WorkerSupervisor"
Cohesion: 0.12
Nodes (11): signal_of(), SupervisorError, Client, HandshakeTimeout, IsolationDisabled, Shutdown, Spawn, WorkerExited (+3 more)

### Community 76 - "HardwareCapabilities"
Cohesion: 0.12
Nodes (19): a_device_for_one_vendor_does_not_unlock_another(), a_hardware_encoder_with_its_device_is_offered(), a_hardware_encoder_without_its_device_is_not_offered(), a_machine_with_a_card_offers_it(), a_machine_with_no_card_offers_the_software_encoder(), a_software_encoder_needs_no_device(), an_encoder_that_is_not_listed_is_not_offered_whatever_the_device(), detect() (+11 more)

### Community 77 - "pathbuf"
Cohesion: 0.12
Nodes (11): batch_transcode(), ffmpeg(), preset_transcode(), single_transcode(), source(), crop_detection_runs_on_generated_media(), generate_basic_media(), generate_chaptered_media() (+3 more)

### Community 78 - "Common Cognitive Load Violations"
Cohesion: 0.22
Nodes (9): 1. The Wall of Options, 2. The Memory Bridge, 3. The Hidden Navigation, 4. The Jargon Barrier, 5. The Visual Noise Floor, 6. The Inconsistent Pattern, 7. The Multi-Task Demand, 8. The Context Switch (+1 more)

### Community 79 - "iOS platform"
Cohesion: 0.22
Nodes (9): Color & materials, Components & controls, iOS platform, Layout & structure, Motion, The iOS slop test, Touch targets, Typography (+1 more)

### Community 80 - "Shape"
Cohesion: 0.22
Nodes (8): Cadence, Confirm and stop, Phase 1: Discovery interview, Phase 2: Resolve the design direction, Phase 3: Write the brief, Round 1: purpose, people, and outcome, Round 2: material, behavior, and boundaries, Shape

### Community 81 - "portable.rs"
Cohesion: 0.15
Nodes (25): a_kind_can_be_pointed_somewhere_else_on_its_own(), a_portable_build_writes_its_state_beside_the_executable(), a_relative_directory_that_climbs_out_is_refused_and_says_which_line(), a_relative_directory_that_stays_inside_is_allowed_however_deep(), an_absolute_directory_is_taken_as_written(), escapes_root(), GENERAL_DIRECTORY_KEY, LOGS_FOLDER (+17 more)

### Community 82 - "video_processing.rs"
Cohesion: 0.11
Nodes (6): generate_source(), tool(), transcode_with_audio_processing(), generate_source(), tool(), transcode_with_video_processing()

### Community 83 - "platform.rs"
Cohesion: 0.07
Nodes (40): parent_is_alive(), a_platform_that_cannot_move_a_priority_says_so_rather_than_pretending(), ABOVE_NORMAL_PRIORITY_CLASS, ascii(), BELOW_NORMAL_PRIORITY_CLASS, CloseHandle(), ControlError, Failed (+32 more)

### Community 84 - "validation.rs"
Cohesion: 0.16
Nodes (38): a_container_that_can_carry_the_chosen_codecs_is_accepted(), a_deblock_strength_outside_the_filters_range_is_refused(), a_destination_that_cannot_be_written_is_refused(), a_destination_that_is_a_folder_is_refused(), a_filter_the_encoder_cannot_apply_is_refused(), a_layout_the_source_cannot_produce_is_refused(), a_lossless_job_that_carries_no_rate_is_accepted(), a_lossless_request_may_not_carry_a_rate() (+30 more)

### Community 85 - "Android platform"
Cohesion: 0.25
Nodes (8): Android platform, Color & theming, Components & motion, Layout & structure, The Android slop test, Touch targets, Typography, Verifying the build

### Community 86 - "dialogs.rs"
Cohesion: 0.05
Nodes (50): alert_headline(), ask_for_alert(), clear_errors(), confirm_alert(), dismiss_alert(), error_view(), is_confirmed(), tick_alert() (+42 more)

### Community 87 - "Persona-Based Design Testing"
Cohesion: 0.25
Nodes (8): 1. Impatient Power User: "Alex", 2. Confused First-Timer: "Jordan", 3. Accessibility-Dependent User: "Sam", 4. Deliberate Stress Tester: "Riley", 5. Distracted Mobile User: "Casey", Persona-Based Design Testing, Project-Specific Personas, Selecting Personas

### Community 88 - "hdr.rs"
Cohesion: 0.10
Nodes (18): capabilities_are_read_from_the_encoder_probe(), dolby_vision_can_be_stripped_or_refused(), dolby_vision_pass_through_needs_hevc_and_a_capable_encoder(), DolbyVisionAction, ExtractRpu, PassThrough, Strip, DolbyVisionOptions (+10 more)

### Community 89 - "package.json"
Cohesion: 0.25
Nodes (7): name, private, scripts, build, dev, tauri:build, version

### Community 90 - "worker_api.rs"
Cohesion: 0.12
Nodes (33): a_user_requested_stop_is_not_reported_as_a_crash(), a_worker_killed_mid_encode_fails_only_its_job_and_the_next_job_still_runs(), a_worker_that_dies_without_being_asked_is_detected_and_replaced(), consecutive_worker_deaths_are_replaced_and_then_reported_rather_than_looping(), encode_contract_rejects_an_invalid_job(), encode_contract_runs_pauses_resumes_and_stops(), encode_contract_stops_a_running_job(), encode_progress() (+25 more)

### Community 91 - "LogService"
Cohesion: 0.10
Nodes (17): a_folder_that_is_inside_its_bounds_loses_nothing(), a_log_the_folder_does_not_own_is_never_touched(), a_worker_with_no_log_folder_writes_no_encode_logs_and_says_nothing(), an_encode_log_is_opened_under_the_resolved_folder_and_never_above_it(), an_encode_writes_its_own_file_and_the_session_file_both(), an_open_encode_log_is_never_the_one_a_rotation_removes(), append_line(), drops_oldest_entries_and_reports_truncation() (+9 more)

### Community 92 - "Generate Report"
Cohesion: 0.29
Nodes (7): Audit Health Score, Detailed Findings by Severity, Executive Summary, Generate Report, Patterns & Systemic Issues, Platform Conformance Verdict, Positive Findings

### Community 93 - "Cognitive Load Assessment"
Cohesion: 0.29
Nodes (7): Cognitive Load Assessment, Cognitive Load Checklist, Extraneous Load: Bad Design, Germane Load: Learning Effort, Intrinsic Load: The Task Itself, The Working Memory Rule, Three Types of Cognitive Load

### Community 94 - "Impeccable Finish Reviewer"
Cohesion: 0.29
Nodes (6): Checks, in order, Disposition, Impeccable Finish Reviewer, Input Contract, Output Contract, Verdict Pass

### Community 95 - "app.js"
Cohesion: 0.06
Nodes (159): acceptPaths(), actionButton(), activatePreferenceTab(), activateTab(), appearanceOptions(), applyAppearance(), applyChoice(), applyLanguage() (+151 more)

### Community 96 - "live-browser-ignores.js"
Cohesion: 0.52
Nodes (6): globToRegex(), matchesScope(), normalizeIgnoreRule(), normalizeIgnoreValue(), pageCandidates(), resolveDetectIgnores()

### Community 97 - "vise-ipc/src/lib.rs"
Cohesion: 0.08
Nodes (4): LOOPBACK_HOST, PROTOCOL_VERSION, WORKER_NAME, TokenGuard

### Community 98 - "Vise Transcoder {VERSION} Release Notes"
Cohesion: 0.29
Nodes (6): Changes, Highlights, Known Issues, Platforms, Verification, Vise Transcoder {VERSION} Release Notes

### Community 99 - "src/optical.rs"
Cohesion: 0.15
Nodes (11): OpticalSourceKind, BluRay, Dvd, Iso, a_build_with_only_one_library_reports_only_that_one(), a_build_with_the_libraries_reports_both(), a_build_without_the_libraries_reports_neither(), an_optical_image_is_a_dvd_as_far_as_the_library_goes() (+3 more)

### Community 100 - "Visualize: Direction Comps & Asset Production"
Cohesion: 0.33
Nodes (5): After approval: the comp becomes a spec, Generate three compositional options, One approval point, Plates and provenance, Visualize: Direction Comps & Asset Production

### Community 102 - "preset_catalog.rs"
Cohesion: 0.05
Nodes (28): builtin_entries(), builtin_preset(), builtin_presets(), a_catalog_entry_that_breaks_a_rule_is_reported_rather_than_shipped(), a_directory_of_exported_presets_can_be_imported_again(), a_file_written_before_the_version_existed_is_migrated_and_kept(), a_hardware_preset_reaches_the_encoder_the_pipeline_reads(), a_library_can_be_reordered_without_losing_anything() (+20 more)

### Community 103 - "Project Context"
Cohesion: 0.13
Nodes (10): At a Glance, Context Loading Rules, Project Context, Project-Specific Constraints, Where to Look, Bundled media tooling, How a tool is found, Platform notes (+2 more)

### Community 104 - "Reproducible Build Process"
Cohesion: 0.33
Nodes (5): Build, Prerequisites, Reproducibility notes, Reproducible Build Process, Verification

### Community 105 - "contract.rs"
Cohesion: 0.09
Nodes (11): CANCELLED, FAILED, init_command_reports_unsupported_switches(), init_command_resolves_disk_log_path(), init_command_resolves_the_per_encode_log_folder(), InitCommand, INVALID, LOG_BUFFER_LIMIT (+3 more)

### Community 106 - "capability.rs"
Cohesion: 0.12
Nodes (21): CapabilityViewModel, a_machine_with_no_accelerator_says_encoding_runs_on_the_processor(), a_machine_with_no_tools_gets_a_report_that_says_so(), a_report_is_built_from_the_injected_detection(), an_empty_list_is_shown_as_none_rather_than_as_a_blank_section(), CapabilityReport, detected(), HARDWARE_MARKERS (+13 more)

### Community 107 - ".view"
Cohesion: 0.20
Nodes (16): a_category_and_an_entry_this_build_does_not_have_are_refused_by_name(), a_hardware_preset_is_closed_on_a_machine_without_the_chip(), a_machine_that_was_never_probed_closes_nothing_and_says_so(), a_preset_with_no_software_stand_in_is_closed_with_its_reason(), a_stored_preset_is_resolved_too_rather_than_assumed_runnable(), a_translated_description_is_used_and_the_rest_fall_back_to_the_catalog(), a_user_presets_own_description_is_left_alone(), default_category() (+8 more)

### Community 108 - "advanced.rs"
Cohesion: 0.05
Nodes (40): a_portable_settings_file_that_took_isolation_away_closes_the_control_with_its_reason(), ACTION_ON_FAILURE, ACTION_TIMEOUT, ActionSettings, AdvancedSettings, available(), level_effect(), LOG_LEVEL (+32 more)

### Community 109 - "Heuristics Scoring Guide"
Cohesion: 0.50
Nodes (4): Heuristics Scoring Guide, Issue Severity (P0–P3), Reference Material, Score Summary

### Community 110 - "preview.rs"
Cohesion: 0.07
Nodes (28): a_failed_fetch_clears_the_frame_rather_than_leaving_a_stale_one(), a_frame_with_no_reported_size_cannot_be_cropped(), a_scan_without_stills_is_reported_rather_than_shown_as_an_empty_frame(), a_single_still_sits_at_the_start_and_a_title_of_no_length_falls_back_to_zero(), a_still_count_this_build_will_not_extract_is_refused_rather_than_clamped(), an_overlay_that_leaves_the_frame_is_refused_with_a_reason(), changing_the_title_forgets_the_previous_frame(), covering_the_whole_frame_is_reported_as_nothing_to_crop() (+20 more)

### Community 111 - "bundle.rs"
Cohesion: 0.07
Nodes (43): a_bundle_is_byte_for_byte_the_same_for_the_same_session(), a_different_session_produces_a_different_bundle(), a_document_that_cannot_be_read_is_still_redacted_rather_than_dropped(), a_log_too_long_to_send_is_cut_on_a_line_and_says_so(), a_path_inside_a_sentence_is_reduced_rather_than_the_whole_sentence(), a_secret_is_removed_by_its_key_as_well_as_by_its_value(), a_secret_shorter_than_a_word_is_not_scrubbed_out_of_ordinary_text(), a_token_appears_nowhere_in_the_bundle() (+35 more)

### Community 112 - "dimensions.rs"
Cohesion: 0.07
Nodes (38): a_crop_must_be_even_or_the_encoder_refuses_it(), a_manual_crop_is_checked_against_the_frame(), a_shape_crop_centres_the_largest_rectangle_it_can(), a_shape_crop_needs_a_frame_size(), aspect_ratio_value(), crop_rect(), CropMode, AspectRatio (+30 more)

### Community 113 - "tooling.rs"
Cohesion: 0.13
Nodes (24): a_build_with_nothing_bundled_says_so_rather_than_pretending(), a_bundled_file_that_cannot_be_run_is_not_taken_for_one(), a_directory_named_like_a_tool_is_not_taken_for_one(), a_tool_beside_the_executable_wins_over_the_search_path(), both_tools_are_reported_together(), bundled_names(), BUNDLED_SUBDIRECTORIES, every_packaged_layout_is_searched() (+16 more)

### Community 114 - "[0.1.0] - 2026-10-19"
Cohesion: 0.29
Nodes (6): [0.1.0] - 2026-10-19, Added, Changed, Changelog, Fixed, [Unreleased]

### Community 115 - "router.rs"
Cohesion: 0.11
Nodes (36): HttpRequest, HttpResponse, answers_idle_progress_routes(), answers_title_routes_before_a_scan(), answers_token_routes(), bad_request(), control_routes_report_when_nothing_is_running(), decode_body() (+28 more)

### Community 116 - "Route"
Cohesion: 0.08
Nodes (21): Route, .ALL, GetAllLogMessages, GetLogMessagesFromIndex, GetMainTitle, GetPreview, GetTitles, GetWorkerInfo (+13 more)

### Community 117 - "Vise Desktop"
Cohesion: 0.40
Nodes (4): Boundaries, Build and test, Layout, Vise Desktop

### Community 119 - "vise-core"
Cohesion: 0.60
Nodes (5): vise-cli, vise-core, vise-desktop, vise-ipc, vise-worker

### Community 120 - "DesktopState"
Cohesion: 0.11
Nodes (30): apply_preview_crop(), check_output_directory(), collision_policy(), DesktopState, import_subtitles(), non_empty(), output_extension(), point_preview_at_title() (+22 more)

### Community 122 - "http.rs"
Cohesion: 0.12
Nodes (11): find_head_end(), HttpParseError, InvalidContentLength, MalformedHead, MalformedHeader, MalformedRequestLine, TruncatedBody, MAX_BODY_BYTES (+3 more)

### Community 123 - "supervisor.rs"
Cohesion: 0.10
Nodes (18): DEFAULT_HANDSHAKE_TIMEOUT, ENV_HANDSHAKE, ENV_PARENT_PID, ENV_PORT, ENV_TOKEN, Handshake, handshake_files_round_trip(), pipe_is_closed() (+10 more)

### Community 124 - "SubtitleFormat"
Cohesion: 0.12
Nodes (15): SubtitleFormat, Ass, Auto, ClosedCaption, Dvb, Dvd, MovText, Pgs (+7 more)

### Community 125 - "crash.rs"
Cohesion: 0.20
Nodes (12): a_job_that_was_not_stopped_is_not_cancelled_by_one_that_was(), a_user_requested_stop_is_not_a_crash(), a_worker_killed_mid_job_fails_only_that_job_and_names_the_exit_status(), a_worker_that_answered_is_recorded_as_finished(), a_worker_that_answers_again_clears_the_death_streak(), an_encode_failure_is_a_failed_job_and_not_a_crash(), consecutive_worker_deaths_back_off_and_then_report_instead_of_looping(), death() (+4 more)

### Community 126 - "Local Setup"
Cohesion: 0.29
Nodes (6): 1. Install Rust, 2. Install FFmpeg, 3. Install Visual Studio C++ Build Tools, 4. Build and test, 5. Manual smoke test, Local Setup

### Community 127 - "Vise Transcoder"
Cohesion: 0.20
Nodes (10): Build from source, Contributing, Cutting a release, How it is put together, Install, Licence, Screenshots, The Linux package, built in a container (+2 more)

### Community 128 - "encode.rs"
Cohesion: 0.07
Nodes (26): PowerScope, System, SystemAndDisplay, close_encode_log(), control_routes_require_a_running_encode(), encode_log_label(), EncodeControl, EncodeService (+18 more)

### Community 129 - "runtime.rs"
Cohesion: 0.08
Nodes (18): ServerBanner, WorkerInfo, monitor_leaves_a_live_parent_alone(), parent_monitor_ignores_pid_zero(), parent_monitor_terminates_when_the_parent_is_gone(), run(), spawn_parent_monitor(), stdin_is_pipe() (+10 more)

### Community 130 - "LogLevel"
Cohesion: 0.09
Nodes (14): ActivityLog, copied_lines_carry_their_level_and_which_half_wrote_them(), LogLine, LogSource, Desktop, Worker, message(), LogLevel (+6 more)

### Community 131 - "actOnAgentTarget"
Cohesion: 0.29
Nodes (17): actOnAgentTarget(), agentTargetBusyReason(), agentTargetOverlayGone(), agentTargetTaken(), claimAgentTarget(), claimAndActOnAgentTarget(), declineAgentTargetBusy(), declineAgentTargetUnresolvable() (+9 more)

### Community 132 - "DirectionMode"
Cohesion: 0.10
Nodes (16): an_unknown_choice_is_refused_with_the_names_that_would_work(), DEFAULT_DIRECTION_MODE, DirectionMode, .ALL, Auto, Ltr, Rtl, observation_note() (+8 more)

### Community 133 - "tests/subtitles.rs"
Cohesion: 0.18
Nodes (6): burn_in_runs(), generate_source(), scanner(), srt_to_mp4_text_conversion(), text_subtitle_passthrough(), tool()

### Community 134 - "token.rs"
Cohesion: 0.19
Nodes (8): accepts_the_registered_token(), generate_token(), GENERATED_TOKEN_LEN, generated_tokens_are_long_and_unique(), next_counter(), rejects_everything_when_no_token_is_registered(), rejects_missing_and_invalid_tokens(), splitmix64()

### Community 135 - "vise-worker/src/main.rs"
Cohesion: 0.20
Nodes (8): command_line_overrides_the_environment(), main(), Overrides, parse_arguments(), parses_arguments_in_both_forms(), refuses_to_start_without_a_port_and_token(), resolve_settings(), STANDALONE_NOTICE

### Community 136 - "i18n.rs"
Cohesion: 0.07
Nodes (44): a_duplicate_key_is_refused_rather_than_silently_dropped(), a_malformed_file_is_refused_with_the_file_named(), a_missing_key_and_an_unknown_key_are_both_reported(), a_placeholder_that_does_not_match_the_key_is_a_problem(), a_region_tag_reads_its_own_language_before_the_base(), a_translation_that_is_still_the_base_sentence_is_reported(), an_incomplete_locale_falls_back_and_says_so(), AR_DESCRIPTIONS (+36 more)

### Community 137 - "SharedState"
Cohesion: 0.13
Nodes (56): activity_window(), apply_outcome(), apply_when_done(), build_tray(), cancel_all(), cancel_job(), cancel_job_confirmed(), cancel_one() (+48 more)

### Community 138 - "localization.rs"
Cohesion: 0.09
Nodes (13): a_resolved_catalog_carries_both_answers_together(), description_tables(), DescriptionTable, every_shipped_locale_resolves_every_key_its_own_way(), message_tables(), MessageTable, resolve_catalog(), a_language_the_build_does_not_ship_still_gets_a_window() (+5 more)

### Community 139 - "actions.rs"
Cohesion: 0.06
Nodes (32): a_command_that_cannot_be_started_is_reported_rather_than_panicking(), a_command_that_will_not_stop_is_killed_at_the_timeout(), a_malformed_command_stops_the_encode_whatever_the_preference_is(), a_post_action_cannot_fail_an_encode_that_has_been_written(), a_pre_action_that_fails_can_stop_the_encode_or_only_say_so(), a_pre_action_that_works_lets_the_encode_go(), a_real_command_runs_and_its_exit_code_is_the_answer(), a_title_with_shell_characters_in_it_is_only_a_file_name() (+24 more)

### Community 140 - "queue.rs"
Cohesion: 0.07
Nodes (46): a_batch_with_nothing_left_to_queue_is_refused_rather_than_succeeding_with_nothing(), a_collision_inside_the_batch_names_both_titles_and_one_on_disk_names_the_file(), a_name_that_differs_only_in_case_is_still_a_collision(), a_name_the_filesystem_would_refuse_is_refused_with_the_character_named(), a_policy_that_numbers_the_names_shows_the_numbered_names_on_the_sheet(), a_queued_title_only_differs_from_the_base_job_by_its_index_and_output(), a_source_with_no_titles_tells_the_user_to_scan_first(), an_empty_queue_offers_nothing_to_clear_empty_or_cancel() (+38 more)

### Community 141 - "window_with"
Cohesion: 0.12
Nodes (19): a_job_that_cannot_run_is_refused_by_the_queue_before_it_is_claimed(), a_job_the_tabs_would_queue_is_claimed_and_marked_running(), a_sweep_is_refused_while_the_real_queue_has_an_encode_running(), a_window_without_a_worker_reports_itself_as_running_the_pipeline(), about_view(), Backend, InProcess, Worker (+11 more)

### Community 142 - "preferences/updates.rs"
Cohesion: 0.06
Nodes (30): a_day_count_becomes_the_date_everybody_writes(), a_frequency_of_never_resolves_to_no_interval_and_no_date(), add_days(), CHANNEL, CheckFrequency, .ALL, Daily, Monthly (+22 more)

### Community 143 - "viewmodel.rs"
Cohesion: 0.10
Nodes (32): a_source_without_angles_reports_a_single_one(), an_angle_beyond_the_source_is_refused(), AngleViewModel, chapter(), chapter_range_uses_the_chosen_boundaries(), chapter_view(), chapter_views(), ChapterViewModel (+24 more)

### Community 144 - "tabs/audio.rs"
Cohesion: 0.13
Nodes (25): a_copied_track_closes_every_control_that_would_re_encode_it(), a_mixdown_cannot_invent_speakers_the_track_does_not_have(), a_track_change_only_touches_that_track(), a_track_inherits_the_defaults_until_it_says_otherwise(), a_track_that_cannot_be_mixed_refuses_the_whole_job(), an_encoder_with_a_quality_value_needs_one(), audio(), AudioRules (+17 more)

### Community 145 - "general.rs"
Cohesion: 0.07
Nodes (31): a_language_the_picker_sent_is_read_and_one_it_cannot_show_is_kept(), a_stored_value_outside_the_bounds_is_reported_rather_than_trusted(), an_unknown_range_is_refused_with_the_names_that_would_work(), DEFAULT_PREVIEW_FRAMES, DEFAULT_RANGE_MODE, FOLLOW_SYSTEM, GeneralSettings, KEEP_DUPLICATE_TITLES (+23 more)

### Community 146 - "preferences/hardware.rs"
Cohesion: 0.08
Nodes (29): a_device_detection_did_not_find_is_offered_as_missing_with_a_reason(), a_device_this_machine_does_not_have_is_not_stored_rather_than_saved_and_ignored(), a_machine_with_no_media_tools_says_so_rather_than_saying_no_hardware(), a_stored_device_this_machine_does_not_have_closes_the_control_with_the_reason(), ACCELERATION, AccelerationPreference, .ALL, Automatic (+21 more)

### Community 147 - "appearance.rs"
Cohesion: 0.12
Nodes (23): a_chosen_theme_paints_in_both_observations(), a_platform_report_does_not_override_the_page(), a_settings_file_that_is_not_json_reads_as_the_defaults(), a_stored_value_this_build_does_not_know_falls_back_to_the_default(), an_observation_that_did_not_change_says_so(), APPEARANCE_KEY, AppearanceOptionViewModel, AppearanceSettings (+15 more)

### Community 148 - "output.rs"
Cohesion: 0.08
Nodes (31): text_control(), a_folder_that_exists_is_accepted_and_trimmed(), a_folder_that_is_not_there_is_refused_with_the_path_that_was_typed(), a_record_whose_two_answers_disagree_is_reported_and_the_queue_gets_one(), an_empty_folder_is_refused_by_name_rather_than_stored(), an_unknown_collision_is_refused_with_the_names_that_would_work(), COLLISION, CollisionPreference (+23 more)

### Community 149 - "State"
Cohesion: 0.09
Nodes (34): check_for_updates_now(), choose_range(), choose_title(), choose_tracks(), clear_recent_sources(), clear_source(), close_window(), error_report() (+26 more)

### Community 150 - "diskspace.rs"
Cohesion: 0.12
Nodes (19): a_file_that_already_exists_is_a_better_estimate_than_any_formula(), a_job_of_known_size_needs_twice_its_size_and_the_reserve(), a_target_size_the_job_states_wins_over_any_formula(), a_volume_that_could_not_be_read_does_not_stop_the_queue(), a_volume_that_is_already_mostly_full_is_refused_even_for_a_tiny_job(), a_volume_with_no_size_still_has_to_clear_the_reserve(), BITS_PER_BYTE, enough_room_starts_the_job() (+11 more)

### Community 151 - "TitleFacts"
Cohesion: 0.10
Nodes (33): burn_in_available(), MetadataFact, SubtitleFacts, TitleFacts, a_bitmap_track_cannot_be_written_into_an_mp4(), a_burn_in_chosen_forced_track_reaches_the_job_model(), a_track_can_step_outside_the_job_behaviour(), an_imported_file_is_refused_when_the_container_cannot_carry_it() (+25 more)

### Community 152 - "windows/diagnostics.rs"
Cohesion: 0.11
Nodes (22): a_folder_with_no_logs_says_so_rather_than_showing_an_empty_table(), a_range_is_planned_the_moment_it_is_set(), a_range_nobody_confirmed_is_refused_and_the_surface_says_why(), a_refused_plan_reaches_the_page_rather_than_only_the_test(), a_report_is_remembered_once_per_kind(), a_second_sweep_cannot_start_while_the_first_is_running(), a_sweep_hands_out_one_run_at_a_time_and_keeps_its_own_table(), a_sweep_is_refused_while_the_real_queue_is_encoding() (+14 more)

### Community 153 - "sources.rs"
Cohesion: 0.09
Nodes (31): a_missing_settings_file_reads_as_an_empty_list(), ALTERNATE_SEPARATOR, classifies_files_and_folders(), config_root(), default_settings_path(), drop_report_keeps_accepted_and_refused_paths(), file_name_of(), forget_and_clear_remove_entries() (+23 more)

### Community 154 - "EncoderId"
Cohesion: 0.07
Nodes (20): EncoderId, AmfAv1, AmfH264, AmfH265, Av1, Dnxhr, Ffv1, NvencAv1 (+12 more)

### Community 155 - "whendone.rs"
Cohesion: 0.09
Nodes (22): a_stored_action_this_build_cannot_do_is_reported_on_the_tab_and_in_the_problems(), a_wait_with_nothing_to_act_on_says_it_has_no_effect(), ACTION, an_answer_this_build_cannot_give_is_refused_rather_than_stored(), DELAY_SECONDS, MAX_DELAY_SECONDS, read_delay(), tab_view() (+14 more)

### Community 156 - "notify.rs"
Cohesion: 0.07
Nodes (24): a_message_longer_than_the_field_is_cut_rather_than_run_past_it(), balloon_available(), BalloonNotifier, destroy_icon(), GetModuleHandleW(), GetProcAddress(), IDI_APPLICATION, IMAGE_ICON (+16 more)

### Community 157 - "filters.rs"
Cohesion: 0.08
Nodes (32): a_number_field_reads_whole_numbers_and_reports_anything_else(), NumberViewModel, NumField, a_colour_space_is_chosen_or_the_source_one_is_kept(), a_custom_value_outside_its_range_is_reported_and_not_applied(), a_deblock_block_size_outside_the_range_is_reported_not_accepted(), a_filter_that_does_not_exist_is_refused_with_its_name(), a_known_filter_change_is_applied() (+24 more)

### Community 158 - "enclog.rs"
Cohesion: 0.09
Nodes (25): a_folder_inside_both_bounds_is_left_alone(), a_log_that_is_being_written_is_never_removed_and_never_moved(), a_log_the_user_is_reading_is_never_removed(), a_very_long_title_is_capped_rather_than_rejected(), an_encounter_of_protected_files_leaves_the_bound_unmet_rather_than_deleting_one(), decide_rotation(), ENCODE_LOG_PREFIX, ENCODE_LOG_SUFFIX (+17 more)

### Community 159 - "Notification"
Cohesion: 0.09
Nodes (13): a_refused_notification_falls_back_to_the_log_rather_than_being_lost(), FallbackNotifier, LogNotifier, Notification, Notifier, NotifyError, Refused, Unsupported (+5 more)

### Community 160 - "persistence.rs"
Cohesion: 0.10
Nodes (30): a_corrupt_file_is_reported_and_left_alone(), a_file_from_a_newer_build_is_read_and_not_thrown_away(), a_finished_job_stays_finished_and_a_failed_one_stays_failed(), a_job_that_is_not_in_the_queue_is_not_in_the_snapshot(), a_priority_for_a_job_that_is_not_there_is_dropped_rather_than_resurrected(), a_queue_that_was_never_saved_is_not_reported_as_lost(), a_running_job_comes_back_interrupted_and_never_as_a_job_that_just_started(), a_summary_names_how_many_jobs_were_interrupted() (+22 more)

### Community 161 - "Job"
Cohesion: 0.19
Nodes (9): is_finished(), JobPriority, High, Low, Normal, position_in(), queue_view(), QueueState (+1 more)

### Community 162 - "DiagnosticsViewModel"
Cohesion: 0.24
Nodes (18): an_export_that_can_be_written_is_recorded_once_per_kind(), an_export_that_cannot_be_written_is_reported_rather_than_saved_over(), cancel_sweep(), confirm_sweep_range(), detect_capabilities(), diagnostics_window(), encode_log_files(), export_capabilities() (+10 more)

### Community 163 - "tabs/metadata.rs"
Cohesion: 0.06
Nodes (42): TextViewModel, a_change_arrives_as_camel_case_and_is_applied(), a_copied_value_cannot_be_changed_until_the_mode_says_so(), a_removed_tag_is_shown_as_such_and_can_be_restored(), a_removed_tag_stays_removed_across_a_change_of_title(), a_tag_can_be_added_edited_restored_and_removed(), a_tag_name_the_media_tools_would_misread_is_refused(), a_tag_row_with_no_name_is_reported_as_a_problem() (+34 more)

### Community 164 - "tabs/mod.rs"
Cohesion: 0.12
Nodes (17): MetadataInput, a_default_set_of_choices_can_be_queued(), a_problem_in_any_tab_blocks_the_queue(), a_target_size_without_a_duration_refuses_the_job(), a_turn_moves_the_filter_before_the_scale(), every_choice_the_tabs_show_reaches_the_job_spec(), facts(), frame_rate() (+9 more)

### Community 165 - "sweep.rs"
Cohesion: 0.08
Nodes (34): a_finished_sweep_asks_for_nothing_more(), a_narrow_range_is_planned_exactly_as_it_was_asked(), a_plan_is_one_job_per_quality_value_and_nothing_else(), a_range_outside_the_scale_is_clamped_rather_than_refused(), a_range_too_long_to_sample_is_narrowed_by_widening_the_step_not_by_changing_the_range(), a_range_with_no_step_or_no_order_is_refused_with_a_reason(), a_run_in_flight_when_the_sweep_is_cancelled_still_reports_and_says_it_was_stopped(), a_run_reported_twice_is_reported_once() (+26 more)

### Community 166 - "chapter.rs"
Cohesion: 0.13
Nodes (19): Chapter, ChapterImportError, ChapterTrackConfig, collect_named(), csv_accepts_a_header_and_a_two_column_row(), csv_reads_starts_ends_and_names(), csv_refuses_a_bad_row_with_its_line(), parse_chapters() (+11 more)

### Community 167 - "results.rs"
Cohesion: 0.12
Nodes (21): a_job_with_no_quality_value_says_so_rather_than_guessing_one(), a_title_with_a_comma_and_a_quote_round_trips_through_the_csv(), an_empty_queue_exports_a_header_and_nothing_else(), COLUMNS, container_name(), csv_field(), EXPORT_VERSION, file_name() (+13 more)

### Community 168 - "source.rs"
Cohesion: 0.11
Nodes (21): audio(), a_disc_says_so_and_a_file_says_nothing(), a_forced_subtitle_track_is_recognised_from_its_description(), a_quarter_turn_swaps_the_frame_axes(), an_unknown_language_is_named_once(), audio(), audio_facts(), facts_follow_the_chosen_title() (+13 more)

### Community 169 - "Locale"
Cohesion: 0.08
Nodes (20): BASE_LOCALE, Language, .ALL, Arabic, English, French, German, Japanese (+12 more)

### Community 170 - "tray.rs"
Cohesion: 0.16
Nodes (19): a_closed_row_says_why_in_its_own_place(), a_pause_and_a_resume_are_never_both_on_offer(), a_portable_build_says_where_its_state_lives(), a_release_that_is_on_offer_can_be_pressed_and_one_that_is_not_cannot(), build(), facts(), item(), status_line() (+11 more)

### Community 171 - "scanflow.rs"
Cohesion: 0.12
Nodes (25): a_failed_scan_reports_the_reason_and_clears_results(), a_new_flow_starts_idle_and_empty(), a_new_source_clears_everything_the_previous_scan_produced(), a_range_without_a_title_is_refused(), a_scan_that_finished_without_titles_is_a_problem(), an_unusable_range_is_refused_with_its_reason(), choosing_a_title_moves_the_track_and_range_selection(), choosing_tracks_drops_numbers_the_title_does_not_have() (+17 more)

### Community 172 - "Integration"
Cohesion: 0.14
Nodes (16): a_queue_paused_for_want_of_space_is_announced_once(), a_refresh_with_nothing_happening_says_nothing_and_publishes_once(), an_encode_in_a_worker_process_leaves_the_hold_to_the_worker(), an_encode_in_this_window_holds_and_then_lets_go(), Effect, CheckForUpdates, HoldPower, Log (+8 more)

### Community 173 - "system/updates.rs"
Cohesion: 0.11
Nodes (19): a_feed_is_read_into_the_releases_it_lists(), a_feed_that_declares_an_entity_is_refused_rather_than_resolved(), ALLOWED_SCHEMES, an_artifact_that_is_not_over_https_is_refused(), attribute(), child_text(), element(), element_from() (+11 more)

### Community 174 - "chapters.rs"
Cohesion: 0.21
Nodes (24): a_change_that_names_nothing_is_refused(), a_file_that_does_not_parse_is_refused_and_the_list_is_untouched(), a_marker_can_be_edited_retimed_and_removed(), a_marker_cannot_end_after_the_title_does(), a_marker_with_a_time_that_is_not_a_number_is_named_in_the_message(), a_parsed_chapter_file_is_shown_before_it_replaces_anything(), ChapterImportOutcomeViewModel, ChapterImportSettings (+16 more)

### Community 175 - "src/power.rs"
Cohesion: 0.12
Nodes (17): a_hold_carries_the_continuous_flag_and_a_release_does_not(), ascii(), available(), ES_CONTINUOUS, GetModuleHandleW(), GetProcAddress(), kernel32_function(), PowerError (+9 more)

### Community 176 - "locale.rs"
Cohesion: 0.13
Nodes (21): a_language_this_build_does_not_ship_falls_back_and_says_so(), a_left_to_right_language_under_a_pinned_right_to_left_direction_stays_mirrored(), a_pinned_direction_survives_a_right_to_left_language(), a_regional_tag_reads_its_own_language(), an_empty_language_is_refused_with_the_names_that_would_work(), choosing_a_language_moves_the_layout_through_the_observation(), choosing_the_same_language_twice_reports_nothing_changed(), every_shipped_language_is_offered_in_its_own_name() (+13 more)

### Community 177 - "windows/presets.rs"
Cohesion: 0.07
Nodes (30): a_duplicate_name_is_a_warning_rather_than_a_refusal(), a_preset_name_is_required_before_anything_is_stored(), a_preset_summary_names_what_it_leaves_alone(), action_word(), can_submit(), CatalogInput, CatalogViewModel, categories() (+22 more)

### Community 178 - "SystemTheme"
Cohesion: 0.09
Nodes (12): DEFAULT_THEME_MODE, ResolvedTheme, Dark, Light, SystemTheme, Dark, Light, ThemeMode (+4 more)

### Community 179 - "Catalog"
Cohesion: 0.09
Nodes (13): a_key_no_file_has_is_named_rather_than_shown_as_a_key(), capitalise(), Catalog, every_key_has_a_name_and_every_name_is_a_key(), formatting_fills_what_it_is_given_and_leaves_the_rest_visible(), Gap, MessageSource, Base (+5 more)

### Community 180 - "super"
Cohesion: 0.07
Nodes (20): a_group_keeps_the_options_it_was_given(), blocked_options_keep_their_reason(), ChoiceGroup, ChoiceGroupViewModel, ChoiceOptionViewModel, number_in_range(), ranged_numbers_are_checked_against_their_bounds(), ToggleViewModel (+12 more)

### Community 181 - "form_view"
Cohesion: 0.16
Nodes (16): a_built_in_preset_cannot_be_renamed_or_deleted_but_can_be_copied(), a_default_that_points_at_a_deleted_preset_is_reported_as_no_name(), an_action_this_window_cannot_do_is_refused_by_name(), exactly_one_preset_is_the_default(), form_view(), input(), only_the_exchange_actions_read_or_write_a_file(), parse_category() (+8 more)

### Community 182 - "load_settings"
Cohesion: 0.21
Nodes (11): a_preference_is_read_and_written_in_the_settings_file(), recent_sources_survive_a_settings_round_trip(), load_settings(), save_settings(), settings_persistence(), a_file_from_a_newer_build_is_never_downgraded(), a_file_that_declares_no_version_is_read_as_the_oldest_one_and_migrated_forward(), a_missing_empty_or_broken_file_reads_as_the_defaults_or_an_error() (+3 more)

### Community 183 - "tests/optical.rs"
Cohesion: 0.28
Nodes (11): a_blu_ray_folder_is_recognised(), a_blu_ray_is_read_through_its_protocol(), a_dvd_folder_is_recognised(), a_dvd_is_read_through_its_demuxer(), a_tool_with_the_libraries_reaches_the_dvd_reader(), an_image_is_recognised_as_an_optical_source(), an_ordinary_file_is_not_an_optical_source(), an_ordinary_file_is_read_as_itself() (+3 more)

### Community 184 - ".apply"
Cohesion: 0.14
Nodes (10): a_format_this_build_does_not_read_is_refused_by_name(), a_title_without_markers_says_so_and_still_allows_adding_one(), a_valid_change_is_applied(), accepting_a_parsed_file_replaces_the_list_and_clears_the_candidate(), ChapterImportInput, ChapterSettings, MarkerInput, MarkerSettings (+2 more)

### Community 185 - "RangeMode"
Cohesion: 0.16
Nodes (8): a_selection_carries_the_source_label_the_picker_shows(), chapter_range_without_chapters_is_refused_with_a_reason(), mode_choices(), RangeMode, Chapters, PointToPoint, Whole, RangeModeViewModel

### Community 186 - "begin_scan_with"
Cohesion: 0.31
Nodes (8): begin_scan(), begin_scan_with(), scan_options_build_the_worker_command(), scan_options_default_to_no_previews_and_no_limits(), scan_status(), ScanOptions, ScanStatusViewModel, ScanProgressViewModel

### Community 187 - "JobRun"
Cohesion: 0.33
Nodes (5): a_worker_that_keeps_dying_stops_the_queue_instead_of_looping(), JobRun, ContinueAfter, Continued, StopQueue

### Community 188 - "SubtitleCodec"
Cohesion: 0.18
Nodes (10): SubtitleCodec, Ass, Copy, DvbSub, DvdSub, MovText, Srt, Ttml (+2 more)

### Community 189 - "output_and_chapters.rs"
Cohesion: 0.15
Nodes (9): edited_markers_become_a_second_input_that_is_cleaned_up(), encoder_options_reach_the_plan(), flag(), generate_source(), only_mp4_gets_the_front_loaded_index(), plan_for(), the_front_loaded_index_can_be_turned_off(), tool() (+1 more)

### Community 190 - "roadmap.rs"
Cohesion: 0.44
Nodes (9): a_group_marked_complete_has_nothing_left_unchecked(), Counts, every_group_states_the_counts_its_own_checkboxes_add_up_to(), parse(), roadmap(), row_group(), stated_counts(), the_checker_itself_reads_a_roadmap_it_wrote() (+1 more)

### Community 191 - "system/power.rs"
Cohesion: 0.26
Nodes (16): SystemFacts, a_preference_that_asks_for_no_hold_wins_over_a_running_encode(), a_worker_holding_the_machine_is_the_answer_when_isolation_is_on(), Action, Hold, Release, StayPut, an_encode_in_this_window_takes_the_hold_it_asked_for() (+8 more)

### Community 192 - "AppcastError"
Cohesion: 0.11
Nodes (11): AppcastError, DeclaresEntities, HashMismatch, Malformed, NoVersion, RefusedHash, RefusedUrl, TooLarge (+3 more)

### Community 193 - "windows/mod.rs"
Cohesion: 0.14
Nodes (13): button_id(), every_openable_surface_has_one_toolbar_control_and_the_alert_has_none(), every_surface_carries_the_focus_and_escape_rules_the_page_needs(), focus_returns_to_the_surface_that_was_in_front_before(), spec_of(), SurfaceCatalogViewModel, SurfaceKind, Dialog (+5 more)

### Community 194 - "serialize"
Cohesion: 0.09
Nodes (20): state_label(), is_runnable(), is_running(), state_name(), JobState, Cancelled, Completed, Failed (+12 more)

### Community 195 - "want_one_of"
Cohesion: 0.18
Nodes (12): read_log_level(), a_switch_reads_from_a_switch_and_nothing_else(), describe(), kind_of(), validate(), want_number(), want_one_of(), want_one_of_names() (+4 more)

### Community 196 - "synced"
Cohesion: 0.18
Nodes (15): a_job_moves_up_and_down_and_stops_at_the_ends(), a_new_job_joins_at_the_end_and_a_removed_one_is_forgotten(), a_priority_this_build_does_not_use_is_refused_by_name(), a_running_job_reports_its_progress_and_a_finished_one_does_not(), every_row_control_says_why_it_is_closed_or_confirms_first(), finished_jobs_are_the_ones_a_clear_takes(), job(), nothing_is_offered_to_cancel_when_the_encoder_is_idle() (+7 more)

### Community 197 - "AppSettings"
Cohesion: 0.14
Nodes (7): AppSettings, named_string(), read_settings(), SettingsStore, a_partly_missing_document_keeps_every_member_it_does_have(), a_version_that_is_not_a_whole_number_is_treated_as_unversioned(), one_member_of_the_wrong_shape_costs_that_member_and_not_the_file()

### Community 198 - "TrayAction"
Cohesion: 0.14
Nodes (11): TrayAction, About, .ALL, CancelCurrent, CheckForUpdates, OpenQueue, PauseQueue, PreventSleep (+3 more)

### Community 199 - "ContainerFormat"
Cohesion: 0.16
Nodes (16): container_of(), container_label(), a_request_that_asks_for_nothing_is_supported_everywhere(), AttachmentDecision, matroska_carries_both_cover_art_and_attachments(), mp4_carries_cover_art_but_not_attachments(), resolve(), webm_carries_neither_and_says_so() (+8 more)

### Community 200 - "free_space"
Cohesion: 0.19
Nodes (11): free_space(), Gate, Pause, Proceed, read_free_space(), SpaceError, NoVolume, Refused (+3 more)

### Community 201 - "CurlSource"
Cohesion: 0.17
Nodes (8): no_source_reason(), a_fetch_of_something_that_is_not_an_https_address_does_not_run_at_all(), check(), CurlSource, is_allowed_url(), one_line(), the_fetch_is_an_argument_vector_and_the_url_is_only_one_of_the_arguments(), which()

### Community 202 - "optical_end_to_end.rs"
Cohesion: 0.48
Nodes (5): a_blu_ray_folder_is_read_through_the_bluray_protocol(), a_dvd_folder_is_read_as_a_dvd_all_the_way_to_the_window(), an_ordinary_folder_is_not_a_disc(), ffmpeg(), scratch()

### Community 203 - "IsolationPolicy"
Cohesion: 0.21
Nodes (3): a_threshold_of_one_stops_at_the_first_death(), CrashGuard, IsolationPolicy

### Community 204 - "L"
Cohesion: 0.16
Nodes (16): bt(), Ce(), s(), Ee(), ht(), L(), l(), me() (+8 more)

### Community 205 - "ne"
Cohesion: 0.28
Nodes (9): Ct(), Et(), It(), ne(), Se(), St(), Tt(), Ve() (+1 more)

### Community 206 - "CheckOutcome"
Cohesion: 0.20
Nodes (7): CheckOutcome, Available, Failed, NotConfigured, UpToDate, every_outcome_says_what_happened_in_one_line(), what_a_check_found_is_summarised_in_one_line_without_installing_anything()

### Community 207 - "Extract Flow"
Cohesion: 0.25
Nodes (7): Extract Flow, Step 1: Discover the Design System, Step 2: Identify Patterns, Step 3: Plan Extraction, Step 4: Extract & Enrich, Step 5: Migrate, Step 6: Document

### Community 208 - "ScanPhase"
Cohesion: 0.20
Nodes (6): phase_of(), ScanPhase, Failed, Idle, Ready, Scanning

### Community 209 - "SweepProgress"
Cohesion: 0.20
Nodes (8): SweepRowViewModel, SweepViewModel, SweepProgress, SweepState, Cancelled, Finished, Idle, Running

### Community 210 - "track_selection"
Cohesion: 0.20
Nodes (14): a_foreign_language_audio_track_says_so_on_its_own_row(), a_language_reported_as_its_own_code_is_not_shown_twice(), audio_track_view(), language_label(), subtitle_track_view(), track_heading(), track_projection_shows_language_and_codec(), track_projection_without_a_title_is_empty() (+6 more)

### Community 211 - "ChapterProcessingOptions"
Cohesion: 0.25
Nodes (4): build_chapter_filter(), build_chapter_filter_chain(), build_chapters(), ChapterProcessingOptions

### Community 212 - "partial.rs"
Cohesion: 0.25
Nodes (11): a_destination_with_no_partials_sweeps_clean(), a_folder_that_cannot_be_listed_leaves_nothing_behind(), a_partial_sits_next_to_the_destination_and_names_its_writer(), MARKER, partial_output_path(), partial_output_path_for(), partial_outputs_for(), PartialCleanup (+3 more)

### Community 213 - "PresetCategory"
Cohesion: 0.14
Nodes (10): CATEGORIES, category_counts(), PresetCategory, Devices, General, Hardware, Matroska, Professional (+2 more)

### Community 214 - "media_depth.rs"
Cohesion: 0.19
Nodes (17): a_lossless_job_never_hands_the_encoder_a_rate_control_value(), a_real_lossless_ffv1_flac_mkv_is_produced(), an_mp4_faststart_flag_is_not_written_into_other_containers(), deblock_is_applied_when_the_job_asks_for_it(), dolby_vision_and_hdr10_plus_arguments_follow_the_capable_encoder(), generate_source(), lossless_spec(), mp4_faststart_and_ipod_profile_are_applied() (+9 more)

### Community 215 - "StorageKind"
Cohesion: 0.18
Nodes (8): StorageKind, .ALL, Logs, Presets, Queue, Settings, StorageLocation, StorageRoots

### Community 217 - "PreferenceTabViewModel"
Cohesion: 0.20
Nodes (3): ActionViewModel, FactViewModel, PreferenceTabViewModel

### Community 218 - "TrackKind"
Cohesion: 0.33
Nodes (6): Packet, TrackKind, Audio, Data, Subtitle, Video

### Community 219 - "Diagnostic Scan"
Cohesion: 0.33
Nodes (6): 1. Accessibility (VoiceOver / TalkBack), 2. Performance, 3. Appearance & Theming, 4. Platform Conformance (CRITICAL), 5. Adaptivity, Diagnostic Scan

### Community 220 - "ChapterFormat"
Cohesion: 0.18
Nodes (8): ChapterFormat, Csv, Txt, Xml, ChapterMode, Copy, Edit, None

### Community 221 - "PresetSpec"
Cohesion: 0.18
Nodes (10): apply(), apply_encoder_settings(), apply_preset(), apply_rate_control(), export_preset(), export_preset_file(), import_preset(), import_preset_file() (+2 more)

### Community 222 - "read_settings"
Cohesion: 0.21
Nodes (11): discovers_the_file_next_to_the_executable(), load_for(), locate(), portable_file_can_disable_isolation(), portable_file_can_keep_isolation_on(), PortableError, Io, Malformed (+3 more)

### Community 223 - "AppcastItem"
Cohesion: 0.27
Nodes (8): an_artifact_that_does_not_match_the_hash_is_not_offered(), Appcast, AppcastItem, is_newer(), parse_version(), sha256_hex(), verify(), verify_download()

### Community 224 - "fetch-media-tools.sh"
Cohesion: 0.83
Nodes (3): host_platform(), read_platform(), fetch-media-tools.sh script

### Community 225 - "VideoCodec"
Cohesion: 0.13
Nodes (17): video_codec_id(), video_label(), apply(), load(), parse_audio(), parse_container(), parse_video(), Preset (+9 more)

### Community 227 - "fmt"
Cohesion: 0.21
Nodes (8): access_denied_message(), every_listed_route_parses_and_needs_a_token_except_the_banner(), messages_are_vise_branded(), parses_every_documented_route(), Unknown, TOKEN_HEADER, UNKNOWN_ROUTE_MESSAGE, unknown_routes_are_flagged()

### Community 228 - "Milestones"
Cohesion: 0.25
Nodes (8): Milestone 1 - First usable transcoder, Milestone 2 - CLI and batch workflows, Milestone 3 - Editing and fidelity parity, Milestone 4 - Performance and platform parity, Milestone 5 - Reference-product parity, Milestone 6 - Desktop polish and operations, Milestone 7 - Media depth, Milestones

### Community 230 - "Operate mode depth (and Read notes)"
Cohesion: 0.22
Nodes (9): Color, Components, Layout, Motion, Operate mode depth (and Read notes), Product constraints, Product permissions, The product slop test (+1 more)

### Community 231 - "ChapterHandling"
Cohesion: 0.31
Nodes (5): ChapterHandling, Copy, Edit, None, ChaptersInput

### Community 232 - "ClaimStop"
Cohesion: 0.67
Nodes (3): ClaimStop, Job, Queue

### Community 233 - "TokenRejection"
Cohesion: 0.19
Nodes (5): constant_time_eq(), rejection_messages_describe_the_route_and_header(), TokenRejection, Invalid, Missing

### Community 235 - "Product"
Cohesion: 0.15
Nodes (12): Accessibility & Inclusion, Brand Commitments, Capabilities and Constraints, Evidence on Hand, Operating Context, Platform, Positioning, Product (+4 more)

### Community 237 - ".detect_crop"
Cohesion: 0.24
Nodes (6): CropDetection, detect_crop(), detect_crop_with(), extract_preview(), extract_preview_with(), find_tool()

### Community 238 - "decide"
Cohesion: 0.12
Nodes (19): a_failure_is_worth_a_notification_wherever_the_window_is(), a_pause_always_says_why(), a_queue_that_finishes_where_you_can_see_it_is_not_a_notification(), an_empty_reason_becomes_a_sentence_rather_than_a_blank(), decide(), decide_with_focus(), Event, .ALL (+11 more)

### Community 239 - "Design System"
Cohesion: 0.18
Nodes (10): Behaviour, Character, Colour, Components, Design System, Grid, Layout, Rules that hold (+2 more)

### Community 240 - "build-linux.sh"
Cohesion: 0.50
Nodes (3): MSYS2_ARG_CONV_EXCL, MSYS_NO_PATHCONV, build-linux.sh script

### Community 241 - "check-linux.sh"
Cohesion: 0.50
Nodes (3): CARGO_TARGET_DIR, PATH, check-linux.sh script

### Community 242 - "check-linux-package.sh"
Cohesion: 0.83
Nodes (3): fail(), note(), check-linux-package.sh script

### Community 245 - "SourceError"
Cohesion: 0.22
Nodes (7): SourceError, Empty, InvalidCharacter, NotASource, NotFound, RelativeMarker, TooLong

### Community 246 - "HardwareFamily"
Cohesion: 0.20
Nodes (8): HardwareFamily, .ALL, MediaFoundation, Nvenc, Qsv, Software, Vcn, VideoToolbox

### Community 247 - "subtitle.rs"
Cohesion: 0.22
Nodes (5): SubtitleMode, BurnIn, None, Passthrough, subtitle_codec_for()

### Community 248 - "WorkerDeath"
Cohesion: 0.22
Nodes (4): CrashDecision, Backoff, Restart, WorkerDeath

### Community 249 - "Resolution"
Cohesion: 0.25
Nodes (5): Resolution, JobCancelled, JobCrashed, JobFinished, QueueStopped

### Community 250 - "PortableSettings"
Cohesion: 0.29
Nodes (5): an_installed_build_ignores_a_directory_it_cannot_honoured(), an_installed_build_writes_its_state_in_the_configuration_directory(), default_settings(), isolation_defaults_to_enabled(), PortableSettings

### Community 251 - "Vise Transcoder 0.1.0"
Cohesion: 0.33
Nodes (5): Highlights, Known issues, Platforms, Verification, Vise Transcoder 0.1.0

## Knowledge Gaps
- **566 isolated node(s):** `System`, `Light`, `Dark`, `Light`, `Dark` (+561 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 2050 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **20 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `JobSpec` connect `JobSpec` to `encode.rs`, `SharedState`, `src-tauri/src/main.rs`, `actions.rs`, `window_with`, `queue.rs`, `HardwareApi`, `SubtitleProcessingOptions`, `MetadataProcessingOptions`, `AudioProcessingOptions`, `diskspace.rs`, `TitleFacts`, `windows/diagnostics.rs`, `Session`, `ScanResult`, `CoreError`, `pipeline.rs`, `persistence.rs`, `Job`, `tabs/mod.rs`, `sweep.rs`, `vise-core/src/lib.rs`, `results.rs`, `VideoProcessingOptions`, `AudioCodec`, `serialize`, `ContainerFormat`, `free_space`, `ChapterProcessingOptions`, `validation.rs`, `media_depth.rs`, `hdr.rs`, `PresetSpec`, `VideoCodec`, `contract.rs`, `subtitle.rs`, `DesktopState`?**
  _High betweenness centrality (0.043) - this node is a cross-community bridge._
- **What connects `System`, `Light`, `Dark` to the rest of the system?**
  _566 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `live-browser.js` be split into smaller, more focused modules?**
  _Cohesion score 0.027676548803309366 - nodes in this community are weakly interconnected._
- **Why does `MessageKey` connect `MessageKey` to `DirectionMode`, `i18n.rs`, `appearance.rs`, `Catalog`, `ops.rs`?**
  _High betweenness centrality (0.019) - this node is a cross-community bridge._
- **Should `initPageChat` be split into smaller, more focused modules?**
  _Cohesion score 0.08880666049953746 - nodes in this community are weakly interconnected._
- **Why does `init()` connect `initGlobalBar` to `live-browser.js`, `SKILL.md`, `onAnnotDown`, `handleKeyDown`, `Init flow`, `initPageChat`, `connectSSE`, `handleManualEditActivity`, `new-work.md`, `live.md`?**
  _High betweenness centrality (0.019) - this node is a cross-community bridge._
- **Should `src-tauri/src/main.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.02702702702702703 - nodes in this community are weakly interconnected._