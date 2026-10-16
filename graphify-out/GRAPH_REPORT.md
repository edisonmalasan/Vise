# Graph Report - Vise  (2026-10-17)

## Corpus Check
- 130 files · ~208,542 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 10 file(s) not represented in the graph (top: .toml 5, (none) 3, .cmd 1)

## Summary
- 2749 nodes · 6077 edges · 145 communities (127 shown, 18 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 90 edges (avg confidence: 0.85)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `1028d3f9`
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
- syncPageChatFocus
- refreshParamsPanel
- modern-screenshot.umd.js
- hardware.rs
- SubtitleProcessingOptions
- MetadataProcessingOptions
- el
- connectSSE
- handleManualEditActivity
- applyEditing
- AudioProcessingOptions
- media.rs
- validation.rs
- src-tauri/src/main.rs
- new-work.md
- ScanSlot
- vise-core/src/preset.rs
- live.md
- CoreError
- Vise Transcoder — Roadmap
- adapt.md
- pipeline.rs
- onboard.md
- createLiveBrowserDomHelpers
- SKILL.md
- The Toolkit
- onAnnotDown
- JobSpec
- createLiveBrowserSessionState
- server.rs
- FfprobeScanner
- VideoProcessingOptions
- renderDesignVisual
- animate.md
- Handle `generate`
- presets.rs
- AudioCodec
- Generate Report
- showToast
- doctor.md
- New visual work
- optimize.md
- injectSvelteComponentsFromManifest
- ops.rs
- geometry.rs
- Impeccable Asset Producer
- Scan mode (approach C: auto-extract, then confirm descriptive language)
- AppSettings
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
- document.md
- polish.md
- quieter.md
- tick
- subtitles.rs
- Init flow
- supervisor.rs
- transcode_baseline.rs
- path
- Common Cognitive Load Violations
- iOS platform
- Shape
- portable.rs
- command
- platform.rs
- super
- Android platform
- colorize.md
- Persona-Based Design Testing
- Extract Flow
- package.json
- worker_api.rs
- LogService
- Generate Report
- Cognitive Load Assessment
- Impeccable Finish Reviewer
- Impeccable Manual Edit Applier
- live-browser-ignores.js
- Vise Transcoder {VERSION} Release Notes
- Diagnostic Scan
- Visualize: Direction Comps & Asset Production
- BuiltInPreset
- Project Context
- Reproducible Build Process
- contract.rs
- Impeccable Documenter
- Region map
- enableInlineEdit
- Heuristics Scoring Guide
- Operate
- Persuade and Experience
- Read
- WorkerSupervisor
- Changelog
- router.rs
- Route
- Vise Desktop (placeholder)
- vise-cli/src/main.rs
- vise-core
- crate::Transcoder
- http.rs
- api.rs
- WorkerRuntime
- EncodeService
- Local Setup
- Vise Transcoder
- encode.rs
- vise-worker/src/lib.rs
- scan.rs
- actOnAgentTarget
- runtime.rs
- client.rs
- TokenGuard
- vise-worker/src/main.rs
- AudioMixdown
- token.rs
- vise-ipc/src/lib.rs
- route.rs
- Responsive Design
- Operate mode depth (and Read notes)
- media_ingestion.rs
- command_result_omits_empty_error

## God Nodes (most connected - your core abstractions)
1. `CoreError` - 99 edges
2. `JobSpec` - 72 edges
3. `WorkerClient` - 46 edges
4. `WorkerRuntime` - 44 edges
5. `ClientError` - 40 edges
6. `Session` - 36 edges
7. `connectSSE()` - 34 edges
8. `Route` - 34 edges
9. `setLiveState()` - 33 edges
10. `resumeSession()` - 33 edges

## Surprising Connections (you probably didn't know these)
- `Step 2: Act by severity` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/doctor.md → .agents/skills/impeccable/scripts/live-browser.js
- `What this owns, and what it does not` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/doctor.md → .agents/skills/impeccable/scripts/live-browser.js
- `Commands` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/SKILL.md → .agents/skills/impeccable/scripts/live-browser.js
- `Init flow` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/init.md → .agents/skills/impeccable/scripts/live-browser.js
- `Backend` --references--> `WorkerSupervisor`  [EXTRACTED]
  apps/vise-desktop/src-tauri/src/main.rs → crates/vise-ipc/src/supervisor.rs

## Import Cycles
- 2-file cycle: `crates/vise-cli/src/cli.rs -> crates/vise-cli/src/ops.rs -> crates/vise-cli/src/cli.rs`

## Communities (145 total, 18 thin omitted)

### Community 0 - "Apple Design Skill"
Cohesion: 0.21
Nodes (13): Apple Design Skill, Damping Ratio, Direct Manipulation (1:1 tracking), Four Human Needs (safety/predictability, understanding, achievement, joy), Interruptibility, Momentum Projection, Pointer Events / setPointerCapture, Reduced Motion & Accessibility (+5 more)

### Community 1 - "live-browser.js"
Cohesion: 0.03
Nodes (108): applyGlobalBarLabelState(), applyLiveBarPreference(), averageRgb01(), bindEditBadgeProxy(), bufferToBase64(), buildInsertPlaceholderSnapshotFromDom(), buildPickedAnchorSnapshot(), buildSvelteExpressionTextMap() (+100 more)

### Community 10 - "syncPageChatFocus"
Cohesion: 0.09
Nodes (51): armPageChatForTyping(), attachSteerFocusDebug(), attachSteerFocusGuard(), buildSteerProcessingDots(), buildSteerQueueHint(), clearSteerAwaitTimer(), clearSteerFocusRecoverTimer(), collapsePageChat() (+43 more)

### Community 11 - "refreshParamsPanel"
Cohesion: 0.15
Nodes (20): applyParamDefaults(), applyParamValue(), buildParamsPanel(), closedClipPath(), formatRangeValue(), getMountedSvelteComponentAnchor(), getVisibleVariantEl(), hideParamsPanel() (+12 more)

### Community 12 - "modern-screenshot.umd.js"
Cohesion: 0.09
Nodes (55): ae(), be(), bt(), Ce(), s(), Ct(), de(), dt() (+47 more)

### Community 13 - "hardware.rs"
Cohesion: 0.06
Nodes (33): detect(), detect_capabilities(), detect_hardware(), detect_hardware_capabilities(), hardware_capabilities(), HardwareAcceleration, HardwareApi, Amf (+25 more)

### Community 14 - "SubtitleProcessingOptions"
Cohesion: 0.05
Nodes (35): SubtitleMode, BurnIn, None, Passthrough, build_subtitle_filter(), build_subtitle_filter_chain(), build_subtitles(), subtitle_codec_for() (+27 more)

### Community 15 - "MetadataProcessingOptions"
Cohesion: 0.05
Nodes (25): build_chapter_filter(), build_chapter_filter_chain(), build_chapters(), Chapter, ChapterMode, Copy, Edit, None (+17 more)

### Community 16 - "el"
Cohesion: 0.07
Nodes (57): actionLabel(), applyConfigureBarChrome(), bindConfigureCountPillTooltip(), bindConfigureInlineControlHover(), bindConfigureModifierPillHover(), buildConfigureActionControl(), buildConfigureCountControl(), buildConfigureRow() (+49 more)

### Community 17 - "connectSSE"
Cohesion: 0.10
Nodes (71): abortSvelteComponentInjection(), beginNewLiveConfiguration(), cancelEditing(), cancelEditingToPicking(), cancelInsertConfigure(), captureAndEmit(), cleanup(), cleanupAcceptedSession() (+63 more)

### Community 18 - "handleManualEditActivity"
Cohesion: 0.17
Nodes (26): clearStoredManualApplyState(), fetchPendingCount(), handleManualEditActivity(), hidePendingApplyDock(), manualApplyLoadingText(), manualApplyStateKey(), manualEditEventForCurrentPage(), numberOrNull() (+18 more)

### Community 19 - "applyEditing"
Cohesion: 0.08
Nodes (36): addManualContextText(), applyEditing(), buildLocatorForLeaf(), canRestoreManualEditElement(), collectManualContextPieces(), walk(), contextElementForManualEdit(), copyEditContainerContext() (+28 more)

### Community 20 - "AudioProcessingOptions"
Cohesion: 0.23
Nodes (4): AudioProcessingOptions, build_audio_filter(), build_audio_filter_chain(), build_filter_chain()

### Community 21 - "media.rs"
Cohesion: 0.09
Nodes (20): convert_chapter(), convert_format(), convert_stream(), extract_preview(), extract_preview_with(), FfprobeChapter, FfprobeFormat, FfprobeResponse (+12 more)

### Community 22 - "validation.rs"
Cohesion: 0.13
Nodes (35): ChapterInfo, ScanResult, TitleInfo, accepts_a_well_formed_job(), container_audio_support(), container_video_support(), ensure_valid(), extension_container() (+27 more)

### Community 23 - "src-tauri/src/main.rs"
Cohesion: 0.06
Nodes (47): add_job(), Backend, InProcess, Worker, build_backend(), cancel_job(), chapters_to_ui(), clear_logs() (+39 more)

### Community 24 - "new-work.md"
Cohesion: 0.13
Nodes (14): Recommended Actions, Craft (deprecated alias), Apply, Live-mode signature params, Set the spatial thesis, Two isolated assessments, Verify, Visitor mode (+6 more)

### Community 25 - "ScanSlot"
Cohesion: 0.08
Nodes (13): ScanProgress, WorkState, ScanDone, Unknown, WorkDone, Working, expands_directories_and_filters_names(), is_excluded() (+5 more)

### Community 26 - "vise-core/src/preset.rs"
Cohesion: 0.19
Nodes (22): apply(), apply_preset(), create_preset(), create_user_preset(), edit_preset(), edit_user_preset(), export_preset(), export_preset_file() (+14 more)

### Community 27 - "live.md"
Cohesion: 0.07
Nodes (26): Step 1: Parse the request, Step 2: Reuse the page, then start, Step 3: Generate, Step 4: Accept and close, Cleanup, Exit, First-time setup, Handle `accept` (+18 more)

### Community 28 - "CoreError"
Cohesion: 0.12
Nodes (13): CoreError, CommandFailed, Invalid, InvalidMedia, Io, JobNotFound, MissingTool, Parse (+5 more)

### Community 29 - "Vise Transcoder — Roadmap"
Cohesion: 0.05
Nodes (37): Architecture decision snapshot, Deferred / Future Work, Group 0 — Project foundation, Group 10 — Encoding control and job reliability, Group 11 — Hardware acceleration, Group 12 - Platform packaging and release, Group 13 — Worker process and JSON IPC, Group 14 — Desktop parity with reference product (+29 more)

### Community 30 - "adapt.md"
Cohesion: 0.08
Nodes (22): Assess Adaptation Challenge, Content Adaptation, Desktop Adaptation (Mobile → Desktop), Email Adaptation (Web → Email), Implement Adaptations, Layout Adaptation Techniques, Mobile Adaptation (Desktop → Mobile), Adaptation Strategies (+14 more)

### Community 31 - "pipeline.rs"
Cohesion: 0.11
Nodes (15): ContainerFormat, Mkv, Mp4, Webm, audio_encoder(), infer_container(), merge_video_options(), parse_codecs() (+7 more)

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
Cohesion: 0.20
Nodes (17): beginEditPin(), buildAnnotationsForCapture(), buildPinElement(), cancelEditingPin(), clampPlaceholderSize(), finalizeEditingPin(), initAnnotOverlay(), localCoords() (+9 more)

### Community 38 - "JobSpec"
Cohesion: 0.08
Nodes (18): EncodingMode, Abr, AverageBitrate, AverageBitrateMode, ConstantQuality, ConstantQualityMode, Cq, Crf (+10 more)

### Community 39 - "createLiveBrowserSessionState"
Cohesion: 0.22
Nodes (15): createLiveBrowserSessionState(), clearHandled(), clearScrollY(), clearSession(), isHandled(), loadSession(), markHandled(), nextCheckpointRevision() (+7 more)

### Community 40 - "server.rs"
Cohesion: 0.09
Nodes (12): BIND_ADDRESS, bind_listener(), binds_loopback_only_and_reports_its_port(), falls_back_when_the_requested_port_is_busy(), handle_connection(), handshake_is_published_when_configured(), publish_handshake_is_a_no_op_without_a_path(), ServerError (+4 more)

### Community 41 - "FfprobeScanner"
Cohesion: 0.19
Nodes (6): CropDetection, detect_crop(), detect_crop_with(), FfprobeScanner, find_tool(), scan()

### Community 42 - "VideoProcessingOptions"
Cohesion: 0.16
Nodes (6): build_video_filter_chain(), FrameRateMode, Auto, Constant, Variable, VideoProcessingOptions

### Community 43 - "renderDesignVisual"
Cohesion: 0.08
Nodes (34): buildCollapsible(), buildColorModels(), buildListHtml(), buildRadiiModels(), buildTypographyModels(), cssSafe(), designEmptyMessage(), escapeHtml() (+26 more)

### Community 44 - "animate.md"
Cohesion: 0.12
Nodes (14): Accessibility and control, Choose material by meaning, Find the job, Implement to the runtime, Set the motion thesis, Timing and easing, Verify, Visitor mode (+6 more)

### Community 45 - "Handle `generate`"
Cohesion: 0.12
Nodes (16): 1. Read the screenshot (if present), 2. Wrap the element, 3. Load the action's reference, 4. Plan three variants: identity first, then mode, then axes, 5. Apply the freeform prompt (if present), 6. Deliver variants, 7. Parameters (composition-sized, 0-4 per variant), 8. Signal done (+8 more)

### Community 46 - "presets.rs"
Cohesion: 0.13
Nodes (6): import_preset(), import_preset_file(), roundtrip_import_export(), settings_persistence(), tmp_path(), user_create_edit_delete_persists()

### Community 47 - "AudioCodec"
Cohesion: 0.12
Nodes (21): apply(), load(), parse_audio(), parse_container(), parse_video(), Preset, AudioCodec, Aac (+13 more)

### Community 48 - "Generate Report"
Cohesion: 0.13
Nodes (14): 1. Accessibility (A11y), 2. Performance, 3. Theming, 4. Responsive Design, 5. Implementation Integrity (CRITICAL), Audit Health Score, Detailed Findings by Severity, Diagnostic Scan (+6 more)

### Community 49 - "showToast"
Cohesion: 0.08
Nodes (41): abandonForeignSession(), abandonSupersededGo(), applySavedSessionMeta(), clampVariantIndex(), commitAcceptedVariantToDom(), copyToClipboard(), discardOrphanedSession(), findActiveSessionSummary() (+33 more)

### Community 50 - "doctor.md"
Cohesion: 0.25
Nodes (7): Monorepo notes, Opting out of the boot check, Step 1: Run the pass, Step 2: Act by severity, Step 3: Deprecated fields are binding, Step 4: Do not overclaim on truth drift, What this owns, and what it does not

### Community 51 - "New visual work"
Cohesion: 0.14
Nodes (14): 1. Decide what is already true, 2. Ask what will change the work, 3. Choose the right amount of invention, 4. Commit the world, 5. Record the decision, 6. Build with full commitment, 7. Inspect and finish, Both paths (+6 more)

### Community 52 - "optimize.md"
Cohesion: 0.14
Nodes (13): Animation Performance, Assess Performance Issues, Core Web Vitals Optimization, Cumulative Layout Shift (CLS < 0.1), Interaction to Next Paint (INP < 200ms), Largest Contentful Paint (LCP < 2.5s), Loading Performance, Network Optimization (+5 more)

### Community 53 - "injectSvelteComponentsFromManifest"
Cohesion: 0.10
Nodes (33): acceptedDomAlreadyClean(), applyOriginalAttrsToSvelteAnchor(), clearHandledWrapperReloadStamp(), commitAcceptedSvelteComponentToDom(), componentModuleCandidates(), deferredRecoverySuperseded(), describeMountFailure(), detectDevServerBase() (+25 more)

### Community 54 - "ops.rs"
Cohesion: 0.13
Nodes (18): BatchArgs, Cli, Commands, Batch, Presets, Transcode, parse(), PresetCommands (+10 more)

### Community 55 - "geometry.rs"
Cohesion: 0.21
Nodes (5): best_rational(), display_aspect_nonsquare_par(), display_aspect_ratio(), gcd(), reduce_ratio()

### Community 56 - "Impeccable Asset Producer"
Cohesion: 0.15
Nodes (11): Act on the receipt, Assemble and review, Plan and asset review, Plan, capture, serve, Core Rule, Decision Comps, Impeccable Asset Producer, Input Contract (+3 more)

### Community 57 - "Scan mode (approach C: auto-extract, then confirm descriptive language)"
Cohesion: 0.15
Nodes (13): Component translation rules, Narrative mapping, Scan mode (approach C: auto-extract, then confirm descriptive language), Schema, Step 1: Find the design assets, Step 2: Auto-extract what can be auto-extracted, Step 2b: Stage the frontmatter, Step 3: Ask the user for qualitative language (+5 more)

### Community 58 - "AppSettings"
Cohesion: 0.24
Nodes (8): AppSettings, load_app_settings(), load_config(), load_settings(), save_app_settings(), save_config(), save_settings(), SettingsStore

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
Cohesion: 0.14
Nodes (10): ClientError, Decode, Protocol, Rejected, Unreachable, encode_body(), RawReply, WorkerClient (+2 more)

### Community 63 - "timespan.rs"
Cohesion: 0.08
Nodes (18): duration_round_trip(), negative_spans_are_accepted_as_zero(), parse_clock_field(), parses_clock_only_forms(), parses_day_components_and_fractions(), serde_round_trip_and_error_message(), TimeSpan, .ZERO (+10 more)

### Community 64 - "initGlobalBar"
Cohesion: 0.11
Nodes (33): agentHasWorkInFlight(), agentStatusText(), barPaletteForTheme(), brandMarkSvg(), buildDesignHeader(), designPanelCss(), detectPageTheme(), ensureAgentPollTooltip() (+25 more)

### Community 65 - "AGENTS.md"
Cohesion: 0.18
Nodes (9): Architecture Non-Negotiables, Build, Test, and Development Commands, Git Workflow, Graphify, Obsidian / Cross-Project Knowledge, Project, Source of Truth, Tech Stack (+1 more)

### Community 66 - "clarify.md"
Cohesion: 0.18
Nodes (10): Actions and navigation, Audit the language, Errors and permissions, Forms, Help and instructional text, Loading, empty, and success states, Rewrite by function, Set the message hierarchy (+2 more)

### Community 67 - "Nielsen's 10 Heuristics"
Cohesion: 0.18
Nodes (11): 10. Help and Documentation, 1. Visibility of System Status, 2. Match Between System and Real World, 3. User Control and Freedom, 4. Consistency and Standards, 5. Error Prevention, 6. Recognition Rather Than Recall, 7. Flexibility and Efficiency of Use (+3 more)

### Community 68 - "Generate Combined Critique Report"
Cohesion: 0.18
Nodes (11): Design Health Score, Design Specificity Verdict, Generate Combined Critique Report, Minor Observations, Overall Impression, Persona Red Flags, Priority Issues, Questions to Consider (+3 more)

### Community 69 - "document.md"
Cohesion: 0.18
Nodes (10): Pitfalls, Seed mode, Step 1: Route through new-work's workshop, Step 2: Write seed DESIGN.md, Step 3: Confirm, Style guidelines, The frontmatter: token schema, The markdown body: eight sections (canonical order) (+2 more)

### Community 70 - "polish.md"
Cohesion: 0.18
Nodes (10): 1. Establish the system, 2. Gather the evidence, 3. Triage, 4. Polish the whole path, 5. Verify and finish, Color, imagery, and icons, Content and code, Flow and hierarchy (+2 more)

### Community 71 - "quieter.md"
Cohesion: 0.18
Nodes (10): Assess Current State, Color Refinement, Composition Refinement, Motion Reduction, Plan Refinement, Refine the Design, Simplification, Verify Quality (+2 more)

### Community 72 - "tick"
Cohesion: 0.16
Nodes (18): applyPlaceholderDimensions(), applyPlaceholderSizingStyles(), buildPlaceholderResizeHandles(), createInsertPlaceholder(), cursorForPlaceholderEdge(), ensureInsertPlaceholder(), findInsertAnchorInDom(), isInsertGeneratingSession() (+10 more)

### Community 73 - "subtitles.rs"
Cohesion: 0.42
Nodes (6): burn_in_runs(), generate_source(), scanner(), srt_to_mp4_text_conversion(), text_subtitle_passthrough(), tool()

### Community 74 - "Init flow"
Cohesion: 0.20
Nodes (10): Completion gate, Init flow, Step 1: Load current state, Step 2: Explore the project, Step 3: Interview for product truth, Step 4: Write PRODUCT.md, Step 5: Record workflow defaults, Step 6: Wrap up or resume (+2 more)

### Community 75 - "supervisor.rs"
Cohesion: 0.10
Nodes (18): DEFAULT_HANDSHAKE_TIMEOUT, ENV_HANDSHAKE, ENV_PARENT_PID, ENV_PORT, ENV_TOKEN, Handshake, handshake_files_round_trip(), pipe_is_closed() (+10 more)

### Community 76 - "transcode_baseline.rs"
Cohesion: 0.40
Nodes (7): end_to_end_job_spec(), generate_source(), scanner(), tool(), transcode_to_mkv(), transcode_to_mp4(), transcode_to_webm()

### Community 77 - "path"
Cohesion: 0.33
Nodes (5): batch_transcode(), ffmpeg(), preset_transcode(), single_transcode(), source()

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
Cohesion: 0.13
Nodes (24): default_settings(), discovers_the_file_next_to_the_executable(), isolation_defaults_to_enabled(), load_for(), locate(), normalize_key(), parse_bool(), parse_ini() (+16 more)

### Community 82 - "command"
Cohesion: 0.17
Nodes (6): generate_source(), tool(), transcode_with_audio_processing(), generate_source(), tool(), transcode_with_video_processing()

### Community 83 - "platform.rs"
Cohesion: 0.14
Nodes (20): parent_is_alive(), ascii(), CloseHandle(), ControlError, Failed, NotFound, Unsupported, GetModuleHandleW() (+12 more)

### Community 85 - "Android platform"
Cohesion: 0.25
Nodes (8): Android platform, Color & theming, Components & motion, Layout & structure, The Android slop test, Touch targets, Typography, Verifying the build

### Community 86 - "colorize.md"
Cohesion: 0.25
Nodes (7): Apply at system scale, Audit before choosing, Choose a strategy, Contrast and perception, Live-mode signature params, Verify, Visitor mode

### Community 87 - "Persona-Based Design Testing"
Cohesion: 0.25
Nodes (8): 1. Impatient Power User: "Alex", 2. Confused First-Timer: "Jordan", 3. Accessibility-Dependent User: "Sam", 4. Deliberate Stress Tester: "Riley", 5. Distracted Mobile User: "Casey", Persona-Based Design Testing, Project-Specific Personas, Selecting Personas

### Community 88 - "Extract Flow"
Cohesion: 0.25
Nodes (7): Extract Flow, Step 1: Discover the Design System, Step 2: Identify Patterns, Step 3: Plan Extraction, Step 4: Extract & Enrich, Step 5: Migrate, Step 6: Document

### Community 89 - "package.json"
Cohesion: 0.25
Nodes (7): name, private, scripts, build, dev, tauri:build, version

### Community 90 - "worker_api.rs"
Cohesion: 0.13
Nodes (21): a_worker_that_dies_without_being_asked_is_detected_and_replaced(), encode_contract_rejects_an_invalid_job(), encode_contract_runs_pauses_resumes_and_stops(), encode_contract_stops_a_running_job(), generate_source(), handshake_reports_a_generated_token_and_an_ephemeral_port(), init_command(), logging_contract_reads_and_resets() (+13 more)

### Community 91 - "LogService"
Cohesion: 0.11
Nodes (13): LogLevel, Debug, Error, Info, Warning, LogMessage, LogMessagesFromIndex, drops_oldest_entries_and_reports_truncation() (+5 more)

### Community 92 - "Generate Report"
Cohesion: 0.29
Nodes (7): Audit Health Score, Detailed Findings by Severity, Executive Summary, Generate Report, Patterns & Systemic Issues, Platform Conformance Verdict, Positive Findings

### Community 93 - "Cognitive Load Assessment"
Cohesion: 0.29
Nodes (7): Cognitive Load Assessment, Cognitive Load Checklist, Extraneous Load: Bad Design, Germane Load: Learning Effort, Intrinsic Load: The Task Itself, The Working Memory Rule, Three Types of Cognitive Load

### Community 94 - "Impeccable Finish Reviewer"
Cohesion: 0.29
Nodes (6): Checks, in order, Disposition, Impeccable Finish Reviewer, Input Contract, Output Contract, Verdict Pass

### Community 95 - "Impeccable Manual Edit Applier"
Cohesion: 0.29
Nodes (6): Checks, Entry Atomicity, Impeccable Manual Edit Applier, Input Contract, Output Contract, Workflow

### Community 96 - "live-browser-ignores.js"
Cohesion: 0.52
Nodes (6): globToRegex(), matchesScope(), normalizeIgnoreRule(), normalizeIgnoreValue(), pageCandidates(), resolveDetectIgnores()

### Community 98 - "Vise Transcoder {VERSION} Release Notes"
Cohesion: 0.29
Nodes (6): Changes, Highlights, Known Issues, Platforms, Verification, Vise Transcoder {VERSION} Release Notes

### Community 99 - "Diagnostic Scan"
Cohesion: 0.33
Nodes (6): 1. Accessibility (VoiceOver / TalkBack), 2. Performance, 3. Appearance & Theming, 4. Platform Conformance (CRITICAL), 5. Adaptivity, Diagnostic Scan

### Community 100 - "Visualize: Direction Comps & Asset Production"
Cohesion: 0.33
Nodes (5): After approval: the comp becomes a spec, Generate three compositional options, One approval point, Plates and provenance, Visualize: Direction Comps & Asset Production

### Community 102 - "BuiltInPreset"
Cohesion: 0.53
Nodes (4): builtin_preset(), builtin_presets(), BuiltInPreset, builtin_preset_exists()

### Community 103 - "Project Context"
Cohesion: 0.33
Nodes (5): At a Glance, Context Loading Rules, Project Context, Project-Specific Constraints, Where to Look

### Community 104 - "Reproducible Build Process"
Cohesion: 0.33
Nodes (5): Build, Prerequisites, Reproducibility notes, Reproducible Build Process, Verification

### Community 105 - "contract.rs"
Cohesion: 0.09
Nodes (17): CANCELLED, EncodeCommand, FAILED, init_command_reports_unsupported_switches(), init_command_resolves_disk_log_path(), InitCommand, INVALID, LOG_BUFFER_LIMIT (+9 more)

### Community 106 - "Impeccable Documenter"
Cohesion: 0.40
Nodes (4): Impeccable Documenter, Input Contract, Output Contract, Workflow

### Community 107 - "Region map"
Cohesion: 0.40
Nodes (4): Containment, Painted material, Region map, What varies independently

### Community 108 - "enableInlineEdit"
Cohesion: 0.22
Nodes (9): collectEditableTextRows(), visit(), own(), pickable(), stopFocusOutIntoChrome(), enableInlineEdit(), layoutFlowChildren(), onInlineInput() (+1 more)

### Community 109 - "Heuristics Scoring Guide"
Cohesion: 0.50
Nodes (4): Heuristics Scoring Guide, Issue Severity (P0–P3), Reference Material, Score Summary

### Community 110 - "Operate"
Cohesion: 0.50
Nodes (3): Comps, Directions, Operate

### Community 111 - "Persuade and Experience"
Cohesion: 0.50
Nodes (3): Comps, Directions, Persuade and Experience

### Community 112 - "Read"
Cohesion: 0.50
Nodes (3): Comps, Directions, Read

### Community 113 - "WorkerSupervisor"
Cohesion: 0.13
Nodes (10): SupervisorError, Client, HandshakeTimeout, IsolationDisabled, Shutdown, Spawn, WorkerExited, WorkerMissing (+2 more)

### Community 114 - "Changelog"
Cohesion: 0.50
Nodes (3): Added, Changelog, [Unreleased]

### Community 115 - "router.rs"
Cohesion: 0.17
Nodes (25): answers_idle_progress_routes(), answers_title_routes_before_a_scan(), answers_token_routes(), control_routes_report_when_nothing_is_running(), get(), get_with_token(), idle_work_done(), logging_routes_read_and_reset_the_buffer() (+17 more)

### Community 116 - "Route"
Cohesion: 0.08
Nodes (21): Route, .ALL, GetAllLogMessages, GetLogMessagesFromIndex, GetMainTitle, GetPreview, GetTitles, GetWorkerInfo (+13 more)

### Community 119 - "vise-core"
Cohesion: 0.60
Nodes (5): vise-cli, vise-core, vise-desktop, vise-ipc, vise-worker

### Community 122 - "http.rs"
Cohesion: 0.12
Nodes (11): find_head_end(), HttpParseError, InvalidContentLength, MalformedHead, MalformedHeader, MalformedRequestLine, TruncatedBody, MAX_BODY_BYTES (+3 more)

### Community 123 - "api.rs"
Cohesion: 0.14
Nodes (15): DEFAULT_POLL_INTERVAL, DEFAULT_POLL_TIMEOUT, describe(), is_success(), is_terminal(), poll_encode_until(), poll_until_stable(), propagates_worker_death() (+7 more)

### Community 124 - "WorkerRuntime"
Cohesion: 0.17
Nodes (11): HttpRequest, HttpResponse, bad_request(), decode_body(), dispatch(), encode_report(), handle_preview(), handle_start_encode() (+3 more)

### Community 125 - "EncodeService"
Cohesion: 0.19
Nodes (4): EncodeControl, EncodeService, lock(), refuses_a_second_concurrent_encode()

### Community 126 - "Local Setup"
Cohesion: 0.29
Nodes (6): 1. Install Rust, 2. Install FFmpeg, 3. Install Visual Studio C++ Build Tools, 4. Build and test, 5. Manual smoke test, Local Setup

### Community 127 - "Vise Transcoder"
Cohesion: 0.29
Nodes (6): Build and test, Documentation, Notes, Requirements, Run the CLI, Vise Transcoder

### Community 128 - "encode.rs"
Cohesion: 0.20
Nodes (16): EncodeProgress, WorkDone, control_routes_require_a_running_encode(), EncodeSlot, finish(), parse_clock(), parse_progress(), partial_output_path() (+8 more)

### Community 129 - "vise-worker/src/lib.rs"
Cohesion: 0.14
Nodes (11): monitor_leaves_a_live_parent_alone(), parent_monitor_ignores_pid_zero(), parent_monitor_terminates_when_the_parent_is_gone(), run(), spawn_parent_monitor(), stdin_is_pipe(), wake_listener(), StartupError (+3 more)

### Community 130 - "scan.rs"
Cohesion: 0.22
Nodes (14): StreamInfo, ScanCommand, apply_init(), build_titles(), builds_titles_from_a_scan_result(), command(), default_title(), filters_titles_by_duration_bounds() (+6 more)

### Community 131 - "actOnAgentTarget"
Cohesion: 0.29
Nodes (17): actOnAgentTarget(), agentTargetBusyReason(), agentTargetOverlayGone(), agentTargetTaken(), claimAgentTarget(), claimAndActOnAgentTarget(), declineAgentTargetBusy(), declineAgentTargetUnresolvable() (+9 more)

### Community 132 - "runtime.rs"
Cohesion: 0.21
Nodes (6): ServerBanner, banner_reports_the_loopback_contract(), init_command_is_recorded_and_logged(), runtime_reports_facts_about_itself(), shutdown_flag_round_trips(), unsupported_init_switches_are_warned_about()

### Community 133 - "client.rs"
Cohesion: 0.13
Nodes (4): builds_loopback_addresses_only(), DEFAULT_TIMEOUT, rejects_unreachable_workers_without_panicking(), serialises_command_bodies()

### Community 134 - "TokenGuard"
Cohesion: 0.17
Nodes (6): constant_time_eq(), rejection_messages_describe_the_route_and_header(), TokenGuard, TokenRejection, Invalid, Missing

### Community 135 - "vise-worker/src/main.rs"
Cohesion: 0.18
Nodes (8): command_line_overrides_the_environment(), main(), Overrides, parse_arguments(), parses_arguments_in_both_forms(), refuses_to_start_without_a_port_and_token(), resolve_settings(), STANDALONE_NOTICE

### Community 136 - "AudioMixdown"
Cohesion: 0.14
Nodes (12): AudioMixdown, Auto, Dolby, Mono, Stereo, Surround51, Surround71, AudioMode (+4 more)

### Community 137 - "token.rs"
Cohesion: 0.21
Nodes (8): accepts_the_registered_token(), generate_token(), GENERATED_TOKEN_LEN, generated_tokens_are_long_and_unique(), next_counter(), rejects_everything_when_no_token_is_registered(), rejects_missing_and_invalid_tokens(), splitmix64()

### Community 138 - "vise-ipc/src/lib.rs"
Cohesion: 0.14
Nodes (3): LOOPBACK_HOST, PROTOCOL_VERSION, WORKER_NAME

### Community 139 - "route.rs"
Cohesion: 0.21
Nodes (8): access_denied_message(), every_listed_route_parses_and_needs_a_token_except_the_banner(), messages_are_vise_branded(), parses_every_documented_route(), Unknown, TOKEN_HEADER, UNKNOWN_ROUTE_MESSAGE, unknown_routes_are_flagged()

### Community 140 - "Responsive Design"
Cohesion: 0.20
Nodes (10): Breakpoints: Content-Driven, Detect Input Method, Not Just Screen Size, Layout Adaptation Patterns, Mobile-First: Write It Right, Picture Element for Art Direction, Responsive Design, Responsive Images: Get It Right, Safe Areas: Handle the Notch (+2 more)

### Community 141 - "Operate mode depth (and Read notes)"
Cohesion: 0.22
Nodes (9): Color, Components, Layout, Motion, Operate mode depth (and Read notes), Product constraints, Product permissions, The product slop test (+1 more)

### Community 142 - "media_ingestion.rs"
Cohesion: 0.36
Nodes (6): crop_detection_runs_on_generated_media(), generate_basic_media(), generate_chaptered_media(), preview_extraction_writes_image_file(), scan_generated_media_extracts_video_audio_and_chapters(), tool()

## Knowledge Gaps
- **178 isolated node(s):** `InProcess`, `vise-cli`, `Transcode`, `Batch`, `Presets` (+173 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 933 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **18 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `init()` connect `initGlobalBar` to `live-browser.js`, `SKILL.md`, `onAnnotDown`, `syncPageChatFocus`, `Init flow`, `enableInlineEdit`, `connectSSE`, `doctor.md`, `handleManualEditActivity`, `live.md`?**
  _High betweenness centrality (0.078) - this node is a cross-community bridge._
- **What connects `InProcess`, `vise-cli`, `Transcode` to the rest of the system?**
  _178 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `live-browser.js` be split into smaller, more focused modules?**
  _Cohesion score 0.03400093153237075 - nodes in this community are weakly interconnected._
- **Why does `JobSpec` connect `JobSpec` to `encode.rs`, `AudioMixdown`, `contract.rs`, `VideoProcessingOptions`, `hardware.rs`, `SubtitleProcessingOptions`, `MetadataProcessingOptions`, `AudioCodec`, `AudioProcessingOptions`, `validation.rs`, `src-tauri/src/main.rs`, `vise-core/src/preset.rs`, `EncodeService`, `pipeline.rs`?**
  _High betweenness centrality (0.058) - this node is a cross-community bridge._
- **Should `syncPageChatFocus` be split into smaller, more focused modules?**
  _Cohesion score 0.08549019607843138 - nodes in this community are weakly interconnected._
- **Why does `Commands` connect `SKILL.md` to `initGlobalBar`?**
  _High betweenness centrality (0.030) - this node is a cross-community bridge._
- **Should `modern-screenshot.umd.js` be split into smaller, more focused modules?**
  _Cohesion score 0.08897243107769423 - nodes in this community are weakly interconnected._