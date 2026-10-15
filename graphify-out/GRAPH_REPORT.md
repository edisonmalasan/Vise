# Graph Report - Vise  (2026-10-16)

## Corpus Check
- 104 files · ~179,459 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 10 file(s) not represented in the graph (top: .toml 5, (none) 3, .cmd 1)

## Summary
- 1956 nodes · 4229 edges · 122 communities (104 shown, 18 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 83 edges (avg confidence: 0.86)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `e624a034`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Response and Direct Manipulation
- live-browser.js
- Frame Smoothness and Libraries
- Design Process
- Design Principles
- Initial Response
- Materials and Depth
- Multimodal Feedback
- Rubber Banded Boundaries
- Spatial Consistency
- initGlobalBar
- resumeSession
- modern-screenshot.umd.js
- hardware.rs
- SubtitleProcessingOptions
- MetadataProcessingOptions
- el
- connectSSE
- actOnAgentTarget
- applyEditing
- AudioProcessingOptions
- media.rs
- cleanup
- Session
- SKILL.md
- showToast
- vise-core/src/preset.rs
- live.md
- CoreError
- Vise Transcoder — Roadmap
- Responsive Design
- pipeline.rs
- onboard.md
- createLiveBrowserDomHelpers
- Operate mode depth (and Read notes)
- The Toolkit
- captureElementToBlob
- EncodingMode
- createLiveBrowserSessionState
- JobSpec
- FfprobeScanner
- VideoProcessingOptions
- src-tauri/src/main.rs
- animate.md
- Handle `generate`
- presets.rs
- VideoCodec
- Generate Report
- onAnnotDown
- doctor.md
- New visual work
- optimize.md
- mountSvelteComponentVariant
- ops.rs
- geometry.rs
- Impeccable Asset Producer
- Scan mode (approach C: auto-extract, then confirm descriptive language)
- AppSettings
- critique.md
- Simplify the Design
- Hardening Dimensions
- src/cli.rs
- session.rs
- JobState
- AGENTS.md
- clarify.md
- Nielsen's 10 Heuristics
- Generate Combined Critique Report
- document.md
- polish.md
- quieter.md
- scheduleAcceptCleanup
- subtitles.rs
- Init flow
- syncEditBadgeHitProxies
- transcode_baseline.rs
- media_ingestion.rs
- Common Cognitive Load Violations
- iOS platform
- Shape
- startVoice
- audio_processing.rs
- AudioCodec
- release.rs
- Android platform
- colorize.md
- Persona-Based Design Testing
- Extract Flow
- package.json
- tests/cli.rs
- pathbuf
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
- Adaptation Strategies
- Impeccable Documenter
- Region map
- enableInlineEdit
- Heuristics Scoring Guide
- Operate
- Persuade and Experience
- Read
- Command guidance
- Changelog
- MediaBuffer
- FrameRateMode
- Vise Desktop (placeholder)
- vise-cli/src/main.rs
- vise-core
- crate::Transcoder

## God Nodes (most connected - your core abstractions)
1. `CoreError` - 96 edges
2. `JobSpec` - 52 edges
3. `Session` - 36 edges
4. `connectSSE()` - 34 edges
5. `setLiveState()` - 33 edges
6. `resumeSession()` - 33 edges
7. `showToast()` - 31 edges
8. `initGlobalBar()` - 30 edges
9. `el()` - 29 edges
10. `buildInsertConfigureRow()` - 27 edges

## Surprising Connections (you probably didn't know these)
- `Step 2: Act by severity` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/doctor.md → .agents/skills/impeccable/scripts/live-browser.js
- `What this owns, and what it does not` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/doctor.md → .agents/skills/impeccable/scripts/live-browser.js
- `Commands` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/SKILL.md → .agents/skills/impeccable/scripts/live-browser.js
- `Init flow` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/init.md → .agents/skills/impeccable/scripts/live-browser.js
- `DesktopState` --references--> `Session`  [EXTRACTED]
  apps/vise-desktop/src-tauri/src/main.rs → crates/vise-core/src/session.rs

## Import Cycles
- 2-file cycle: `crates/vise-cli/src/cli.rs -> crates/vise-cli/src/ops.rs -> crates/vise-cli/src/cli.rs`

## Communities (122 total, 18 thin omitted)

### Community 0 - "Response and Direct Manipulation"
Cohesion: 0.21
Nodes (13): Apple Design Skill, Damping Ratio, Direct Manipulation (1:1 tracking), Four Human Needs (safety/predictability, understanding, achievement, joy), Interruptibility, Momentum Projection, Pointer Events / setPointerCapture, Reduced Motion & Accessibility (+5 more)

### Community 1 - "live-browser.js"
Cohesion: 0.03
Nodes (112): applyGlobalBarLabelState(), applyLiveBarPreference(), applyPlaceholderSizingStyles(), bufferToBase64(), buildCollapsible(), buildColorModels(), buildDesignHeader(), buildListHtml() (+104 more)

### Community 10 - "initGlobalBar"
Cohesion: 0.06
Nodes (70): agentHasWorkInFlight(), agentStatusText(), armPageChatForTyping(), attachSteerFocusDebug(), attachSteerFocusGuard(), barPaletteForTheme(), brandMarkSvg(), buildSteerProcessingDots() (+62 more)

### Community 11 - "resumeSession"
Cohesion: 0.08
Nodes (64): applyParamDefaults(), applyParamValue(), applyPlaceholderDimensions(), applySavedSessionMeta(), buildParamsPanel(), closedClipPath(), completeParameterGenerationIfReady(), completeParameterPublication() (+56 more)

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
Cohesion: 0.06
Nodes (19): build_chapter_filter(), build_chapter_filter_chain(), build_chapters(), Chapter, ChapterMode, Copy, Edit, None (+11 more)

### Community 16 - "el"
Cohesion: 0.08
Nodes (48): actionLabel(), applyConfigureBarChrome(), bindConfigureCountPillTooltip(), bindConfigureInlineControlHover(), bindConfigureModifierPillHover(), buildConfigureActionControl(), buildConfigureCountControl(), buildConfigureRow() (+40 more)

### Community 17 - "connectSSE"
Cohesion: 0.13
Nodes (47): beginNewLiveConfiguration(), cancelEditing(), cancelEditingToPicking(), cancelInsertConfigure(), cleanupAcceptedSession(), clearAnnotations(), clearInsertPicking(), closeTunePopover() (+39 more)

### Community 18 - "actOnAgentTarget"
Cohesion: 0.10
Nodes (46): actOnAgentTarget(), agentTargetBusyReason(), agentTargetOverlayGone(), agentTargetTaken(), claimAgentTarget(), claimAndActOnAgentTarget(), clearStoredManualApplyState(), declineAgentTargetBusy() (+38 more)

### Community 19 - "applyEditing"
Cohesion: 0.08
Nodes (36): addManualContextText(), applyEditing(), buildLocatorForLeaf(), canRestoreManualEditElement(), collectManualContextPieces(), walk(), contextElementForManualEdit(), copyEditContainerContext() (+28 more)

### Community 20 - "AudioProcessingOptions"
Cohesion: 0.10
Nodes (16): AudioMixdown, Auto, Dolby, Mono, Stereo, Surround51, Surround71, AudioMode (+8 more)

### Community 21 - "media.rs"
Cohesion: 0.11
Nodes (23): scan_source(), ChapterInfo, convert_chapter(), convert_format(), convert_stream(), FfprobeChapter, FfprobeFormat, FfprobeResponse (+15 more)

### Community 22 - "cleanup"
Cohesion: 0.11
Nodes (31): abortSvelteComponentInjection(), buildInsertPlaceholderSnapshotFromDom(), buildPickedAnchorSnapshot(), captureAndEmit(), cleanup(), clearHandled(), clearMountErrorCard(), clearScrollY() (+23 more)

### Community 23 - "Session"
Cohesion: 0.14
Nodes (3): DesktopState, Job, Session

### Community 24 - "SKILL.md"
Cohesion: 0.10
Nodes (20): Assess Adaptation Challenge, Implement & Verify, Recommended Actions, Craft (deprecated alias), Apply, Live-mode signature params, Set the spatial thesis, Two isolated assessments (+12 more)

### Community 25 - "showToast"
Cohesion: 0.11
Nodes (29): abandonForeignSession(), abandonSupersededGo(), buildCyclingRow(), cycleVariant(), cyclingCounterText(), cyclingShownVariant(), discardOrphanedSession(), dismissToast() (+21 more)

### Community 26 - "vise-core/src/preset.rs"
Cohesion: 0.18
Nodes (24): apply(), apply_preset(), create_preset(), create_user_preset(), edit_preset(), edit_user_preset(), export_preset(), export_preset_file() (+16 more)

### Community 27 - "live.md"
Cohesion: 0.07
Nodes (26): Step 1: Parse the request, Step 2: Reuse the page, then start, Step 3: Generate, Step 4: Accept and close, Cleanup, Exit, First-time setup, Handle `accept` (+18 more)

### Community 28 - "CoreError"
Cohesion: 0.12
Nodes (13): CoreError, CommandFailed, Invalid, InvalidMedia, Io, JobNotFound, MissingTool, Parse (+5 more)

### Community 29 - "Vise Transcoder — Roadmap"
Cohesion: 0.07
Nodes (26): Architecture decision snapshot, Deferred / Future Work, Group 0 — Project foundation, Group 10 — Encoding control and job reliability, Group 11 — Hardware acceleration, Group 12 - Platform packaging and release, Group 1 — Media ingestion, Group 2 — Core transcode baseline (+18 more)

### Community 30 - "Responsive Design"
Cohesion: 0.08
Nodes (25): Assess Adaptation Challenge, Breakpoints: Content-Driven, Content Adaptation, Desktop Adaptation (Mobile → Desktop), Detect Input Method, Not Just Screen Size, Email Adaptation (Web → Email), Implement Adaptations, Layout Adaptation Patterns (+17 more)

### Community 31 - "pipeline.rs"
Cohesion: 0.16
Nodes (7): infer_container(), merge_video_options(), parse_codecs(), transcode(), transcode_file(), Transcoder, TranscodeResult

### Community 32 - "onboard.md"
Cohesion: 0.09
Nodes (22): Assess Onboarding Needs, Context Over Ceremony, Contextual Help, Design Onboarding Experiences, Documentation & Help, Empty State Design, Feature Discovery & Adoption, Guided Tours & Walkthroughs (+14 more)

### Community 33 - "createLiveBrowserDomHelpers"
Cohesion: 0.13
Nodes (17): createLiveBrowserDomHelpers(), cloneWithoutChrome(), cssId(), liveUiRoot(), makeFrozenAnchor(), own(), pickable(), raiseHitModal() (+9 more)

### Community 34 - "Operate mode depth (and Read notes)"
Cohesion: 0.10
Nodes (18): Craft floor, Refuse, Verify, Constraints, Failure modes, Flow, $impeccable hooks, Routing (+10 more)

### Community 35 - "The Toolkit"
Cohesion: 0.10
Nodes (20): Animate complex properties, Assess What "Extraordinary" Means Here, For data-heavy interfaces, For functional UI, For performance-critical UI, For visual/marketing surfaces, Implement with Discipline, Interact with the device (+12 more)

### Community 36 - "captureElementToBlob"
Cohesion: 0.12
Nodes (20): averageRgb01(), captureChromeNodes(), captureElementFromRenderedAncestor(), captureElementToBlob(), compileShader(), cssColorToRgb01(), dominantRgb01(), findBackdropAncestor() (+12 more)

### Community 38 - "EncodingMode"
Cohesion: 0.10
Nodes (17): EncodingMode, Abr, AverageBitrate, AverageBitrateMode, ConstantQuality, ConstantQualityMode, Cq, Crf (+9 more)

### Community 39 - "createLiveBrowserSessionState"
Cohesion: 0.22
Nodes (15): createLiveBrowserSessionState(), clearHandled(), clearScrollY(), clearSession(), isHandled(), loadSession(), markHandled(), nextCheckpointRevision() (+7 more)

### Community 41 - "FfprobeScanner"
Cohesion: 0.19
Nodes (7): CropDetection, detect_crop(), detect_crop_with(), extract_preview(), extract_preview_with(), FfprobeScanner, find_tool()

### Community 43 - "src-tauri/src/main.rs"
Cohesion: 0.30
Nodes (10): add_job(), cancel_job(), clear_logs(), list_jobs(), logs(), pause_job(), push_log(), remove_job() (+2 more)

### Community 44 - "animate.md"
Cohesion: 0.12
Nodes (14): Accessibility and control, Choose material by meaning, Find the job, Implement to the runtime, Set the motion thesis, Timing and easing, Verify, Visitor mode (+6 more)

### Community 45 - "Handle `generate`"
Cohesion: 0.12
Nodes (16): 1. Read the screenshot (if present), 2. Wrap the element, 3. Load the action's reference, 4. Plan three variants: identity first, then mode, then axes, 5. Apply the freeform prompt (if present), 6. Deliver variants, 7. Parameters (composition-sized, 0-4 per variant), 8. Signal done (+8 more)

### Community 46 - "presets.rs"
Cohesion: 0.15
Nodes (4): roundtrip_import_export(), settings_persistence(), tmp_path(), user_create_edit_delete_persists()

### Community 47 - "VideoCodec"
Cohesion: 0.18
Nodes (15): apply(), load(), parse_audio(), parse_container(), parse_video(), Preset, ContainerFormat, Mkv (+7 more)

### Community 48 - "Generate Report"
Cohesion: 0.13
Nodes (14): 1. Accessibility (A11y), 2. Performance, 3. Theming, 4. Responsive Design, 5. Implementation Integrity (CRITICAL), Audit Health Score, Detailed Findings by Severity, Diagnostic Scan (+6 more)

### Community 49 - "onAnnotDown"
Cohesion: 0.24
Nodes (15): beginEditPin(), buildAnnotationsForCapture(), buildPinElement(), cancelEditingPin(), finalizeEditingPin(), initAnnotOverlay(), localCoords(), onAnnotDown() (+7 more)

### Community 50 - "doctor.md"
Cohesion: 0.13
Nodes (12): Before you finish, Scope is sovereign, The amplification, The skeleton test, Why it reads flat, Monorepo notes, Opting out of the boot check, Step 1: Run the pass (+4 more)

### Community 51 - "New visual work"
Cohesion: 0.14
Nodes (14): 1. Decide what is already true, 2. Ask what will change the work, 3. Choose the right amount of invention, 4. Commit the world, 5. Record the decision, 6. Build with full commitment, 7. Inspect and finish, Both paths (+6 more)

### Community 52 - "optimize.md"
Cohesion: 0.14
Nodes (13): Animation Performance, Assess Performance Issues, Core Web Vitals Optimization, Cumulative Layout Shift (CLS < 0.1), Interaction to Next Paint (INP < 200ms), Largest Contentful Paint (LCP < 2.5s), Loading Performance, Network Optimization (+5 more)

### Community 53 - "mountSvelteComponentVariant"
Cohesion: 0.22
Nodes (14): applyOriginalAttrsToSvelteAnchor(), commitAcceptedSvelteComponentToDom(), componentModuleCandidates(), describeMountFailure(), detectDevServerBase(), findLiveElementForSvelteManifest(), getMountedSvelteComponentAnchor(), importFirstReachable() (+6 more)

### Community 54 - "ops.rs"
Cohesion: 0.23
Nodes (9): BatchArgs, TranscodeArgs, batch_command(), BatchFile, BatchJob, CANCEL_PATH, setup_cancel_handler(), transcode_command() (+1 more)

### Community 55 - "geometry.rs"
Cohesion: 0.19
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

### Community 62 - "src/cli.rs"
Cohesion: 0.23
Nodes (9): Cli, Commands, Batch, Presets, Transcode, parse(), PresetCommands, Inspect (+1 more)

### Community 63 - "session.rs"
Cohesion: 0.27
Nodes (3): session_add_and_read(), session_remove_missing_errors(), SessionSnapshot

### Community 64 - "JobState"
Cohesion: 0.20
Nodes (9): JobState, Cancelled, Completed, Failed, Paused, Queued, Running, JobStatus (+1 more)

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

### Community 72 - "scheduleAcceptCleanup"
Cohesion: 0.31
Nodes (11): acceptedDomAlreadyClean(), clearHandledWrapperReloadStamp(), deferredRecoverySuperseded(), ensureAcceptedDomClean(), findAcceptedRuntimeWrappers(), handledWrapperReloadKey(), reloadAfterMissingAcceptedDom(), restoreAcceptedDomFromSnapshot() (+3 more)

### Community 73 - "subtitles.rs"
Cohesion: 0.31
Nodes (6): burn_in_runs(), generate_source(), scanner(), srt_to_mp4_text_conversion(), text_subtitle_passthrough(), tool()

### Community 74 - "Init flow"
Cohesion: 0.20
Nodes (10): Completion gate, Init flow, Step 1: Load current state, Step 2: Explore the project, Step 3: Interview for product truth, Step 4: Write PRODUCT.md, Step 5: Record workflow defaults, Step 6: Wrap up or resume (+2 more)

### Community 75 - "syncEditBadgeHitProxies"
Cohesion: 0.27
Nodes (10): bindEditBadgeProxy(), editBadgeProxyTargets(), initEditBadge(), initEditBadgeHitProxies(), positionEditBadge(), proxyMouseEvent(), setImportantStyle(), styleEditBadgeProxy() (+2 more)

### Community 76 - "transcode_baseline.rs"
Cohesion: 0.40
Nodes (7): end_to_end_job_spec(), generate_source(), scanner(), tool(), transcode_to_mkv(), transcode_to_mp4(), transcode_to_webm()

### Community 77 - "media_ingestion.rs"
Cohesion: 0.31
Nodes (6): crop_detection_runs_on_generated_media(), generate_basic_media(), generate_chaptered_media(), preview_extraction_writes_image_file(), scan_generated_media_extracts_video_audio_and_chapters(), tool()

### Community 78 - "Common Cognitive Load Violations"
Cohesion: 0.22
Nodes (9): 1. The Wall of Options, 2. The Memory Bridge, 3. The Hidden Navigation, 4. The Jargon Barrier, 5. The Visual Noise Floor, 6. The Inconsistent Pattern, 7. The Multi-Task Demand, 8. The Context Switch (+1 more)

### Community 79 - "iOS platform"
Cohesion: 0.22
Nodes (9): Color & materials, Components & controls, iOS platform, Layout & structure, Motion, The iOS slop test, Touch targets, Typography (+1 more)

### Community 80 - "Shape"
Cohesion: 0.22
Nodes (8): Cadence, Confirm and stop, Phase 1: Discovery interview, Phase 2: Resolve the design direction, Phase 3: Write the brief, Round 1: purpose, people, and outcome, Round 2: material, behavior, and boundaries, Shape

### Community 81 - "startVoice"
Cohesion: 0.36
Nodes (9): finishVoiceSession(), isEmbeddedPreviewBrowser(), releaseVoiceEngine(), startVoice(), steerSpeechRecognitionCtor(), steerVoiceErrorMessage(), steerVoiceUnavailableMessage(), stopVoice() (+1 more)

### Community 82 - "audio_processing.rs"
Cohesion: 0.28
Nodes (3): generate_source(), tool(), transcode_with_audio_processing()

### Community 83 - "AudioCodec"
Cohesion: 0.22
Nodes (9): AudioCodec, Aac, Ac3, Eac3, Flac, Mp3, Opus, Vorbis (+1 more)

### Community 84 - "release.rs"
Cohesion: 0.31
Nodes (6): no_update_when_same(), ReleaseInfo, update_available_when_latest_newer(), UpdateManifest, version_less_than(), VISE_VERSION

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

### Community 90 - "tests/cli.rs"
Cohesion: 0.39
Nodes (5): batch_transcode(), ffmpeg(), preset_transcode(), single_transcode(), source()

### Community 91 - "pathbuf"
Cohesion: 0.32
Nodes (3): generate_source(), tool(), transcode_with_video_processing()

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

### Community 105 - "Adaptation Strategies"
Cohesion: 0.40
Nodes (5): Adaptation Strategies, Orientation & foldables, Phone → Tablet (iPad / large screens), Platform → platform (iOS ↔ Android), Web → native (porting a website or web app)

### Community 106 - "Impeccable Documenter"
Cohesion: 0.40
Nodes (4): Impeccable Documenter, Input Contract, Output Contract, Workflow

### Community 107 - "Region map"
Cohesion: 0.40
Nodes (4): Containment, Painted material, Region map, What varies independently

### Community 108 - "enableInlineEdit"
Cohesion: 0.40
Nodes (5): collectEditableTextRows(), visit(), enableInlineEdit(), onInlineInput(), wrapMixedContentTextNodes()

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

### Community 113 - "Command guidance"
Cohesion: 0.50
Nodes (3): Command guidance, No-argument routing: the context-aware menu, Workflow questions

### Community 114 - "Changelog"
Cohesion: 0.50
Nodes (3): Added, Changelog, [Unreleased]

### Community 116 - "FrameRateMode"
Cohesion: 0.50
Nodes (4): FrameRateMode, Auto, Constant, Variable

### Community 119 - "vise-core"
Cohesion: 0.67
Nodes (3): vise-cli, vise-core, vise-desktop

## Knowledge Gaps
- **130 isolated node(s):** `vise-desktop`, `vise-cli`, `Transcode`, `Batch`, `Presets` (+125 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 719 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **18 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `init()` connect `connectSSE` to `live-browser.js`, `createLiveBrowserDomHelpers`, `initGlobalBar`, `Init flow`, `syncEditBadgeHitProxies`, `resumeSession`, `el`, `onAnnotDown`, `doctor.md`, `actOnAgentTarget`, `SKILL.md`, `live.md`?**
  _High betweenness centrality (0.186) - this node is a cross-community bridge._
- **What connects `vise-desktop`, `vise-cli`, `Transcode` to the rest of the system?**
  _130 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `live-browser.js` be split into smaller, more focused modules?**
  _Cohesion score 0.03404067197170645 - nodes in this community are weakly interconnected._
- **Why does `Commands` connect `SKILL.md` to `connectSSE`?**
  _High betweenness centrality (0.074) - this node is a cross-community bridge._
- **Should `initGlobalBar` be split into smaller, more focused modules?**
  _Cohesion score 0.056338028169014086 - nodes in this community are weakly interconnected._
- **Why does `JobSpec` connect `JobSpec` to `EncodingMode`, `VideoProcessingOptions`, `src-tauri/src/main.rs`, `hardware.rs`, `SubtitleProcessingOptions`, `MetadataProcessingOptions`, `VideoCodec`, `AudioCodec`, `AudioProcessingOptions`, `Session`, `vise-core/src/preset.rs`, `pipeline.rs`, `session.rs`?**
  _High betweenness centrality (0.035) - this node is a cross-community bridge._
- **Should `resumeSession` be split into smaller, more focused modules?**
  _Cohesion score 0.08234126984126984 - nodes in this community are weakly interconnected._