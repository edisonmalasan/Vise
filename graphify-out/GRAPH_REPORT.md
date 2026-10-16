# Graph Report - Vise  (2026-10-17)

## Corpus Check
- 106 files · ~180,897 words
- Verdict: corpus is large enough that graph structure adds value.
- Unclassified: 10 file(s) not represented in the graph (top: .toml 5, (none) 3, .cmd 1)

## Summary
- 1981 nodes · 4252 edges · 128 communities (111 shown, 17 thin omitted)
- Extraction: 98% EXTRACTED · 2% INFERRED · 0% AMBIGUOUS · INFERRED: 83 edges (avg confidence: 0.86)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `96e31b74`
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
- connectSSE
- modern-screenshot.umd.js
- hardware.rs
- SubtitleProcessingOptions
- MetadataProcessingOptions
- el
- setLiveState
- actOnAgentTarget
- applyEditing
- AudioProcessingOptions
- media.rs
- showToast
- Session
- SKILL.md
- showBar
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
- renderDesignVisual
- animate.md
- Handle `generate`
- presets.rs
- AudioCodec
- Generate Report
- restoreSessionWithoutWrapper
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
- resolveLiveInjectionAnchor
- initGlobalBar
- AGENTS.md
- clarify.md
- Nielsen's 10 Heuristics
- Generate Combined Critique Report
- document.md
- polish.md
- quieter.md
- handleInsertCreate
- subtitles.rs
- Init flow
- SubtitleFormat
- transcode_baseline.rs
- media_ingestion.rs
- Common Cognitive Load Violations
- iOS platform
- Shape
- startVoice
- audio_processing.rs
- init
- deserialize
- Android platform
- colorize.md
- Persona-Based Design Testing
- Extract Flow
- package.json
- syncAgentPollingUi
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
- subtitle.rs
- Vise Desktop (placeholder)
- vise-cli/src/main.rs
- vise-core
- crate::Transcoder
- SubtitleCodec
- steerFocusLog
- unlockSteerChat
- handleMouseMove
- Local Setup
- Vise Transcoder

## God Nodes (most connected - your core abstractions)
1. `CoreError` - 96 edges
2. `JobSpec` - 52 edges
3. `Session` - 36 edges
4. `connectSSE()` - 34 edges
5. `setLiveState()` - 33 edges
6. `resumeSession()` - 33 edges
7. `showToast()` - 31 edges
8. `initGlobalBar()` - 30 edges
9. `Vise Transcoder — Roadmap` - 30 edges
10. `el()` - 29 edges

## Surprising Connections (you probably didn't know these)
- `Step 2: Act by severity` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/doctor.md → .agents/skills/impeccable/scripts/live-browser.js
- `What this owns, and what it does not` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/doctor.md → .agents/skills/impeccable/scripts/live-browser.js
- `Init flow` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/reference/init.md → .agents/skills/impeccable/scripts/live-browser.js
- `Commands` --references--> `init()`  [INFERRED]
  .agents/skills/impeccable/SKILL.md → .agents/skills/impeccable/scripts/live-browser.js
- `scan_source()` --references--> `ScanResult`  [EXTRACTED]
  apps/vise-desktop/src-tauri/src/main.rs → crates/vise-core/src/media.rs

## Import Cycles
- 2-file cycle: `crates/vise-cli/src/cli.rs -> crates/vise-cli/src/ops.rs -> crates/vise-cli/src/cli.rs`

## Communities (128 total, 17 thin omitted)

### Community 0 - "Apple Design Skill"
Cohesion: 0.21
Nodes (13): Apple Design Skill, Damping Ratio, Direct Manipulation (1:1 tracking), Four Human Needs (safety/predictability, understanding, achievement, joy), Interruptibility, Momentum Projection, Pointer Events / setPointerCapture, Reduced Motion & Accessibility (+5 more)

### Community 1 - "live-browser.js"
Cohesion: 0.05
Nodes (69): applyLiveBarPreference(), applyParamValue(), applyPlaceholderDimensions(), applyPlaceholderSizingStyles(), bindEditBadgeProxy(), buildParamsPanel(), buildPlaceholderResizeHandles(), buildSavingRow() (+61 more)

### Community 10 - "initPageChat"
Cohesion: 0.28
Nodes (16): applyGlobalBarLabelState(), armPageChatForTyping(), collapsePageChat(), expandPageChat(), focusPageChatInput(), focusSteerChat(), globalBarModeToggles(), initPageChat() (+8 more)

### Community 11 - "connectSSE"
Cohesion: 0.13
Nodes (43): abortSvelteComponentInjection(), applyParamDefaults(), clearSession(), closedClipPath(), completeParameterGenerationIfReady(), completeParameterPublication(), completeSourceInjection(), connectSSE() (+35 more)

### Community 12 - "modern-screenshot.umd.js"
Cohesion: 0.09
Nodes (55): ae(), be(), bt(), Ce(), s(), Ct(), de(), dt() (+47 more)

### Community 13 - "hardware.rs"
Cohesion: 0.06
Nodes (33): detect(), detect_capabilities(), detect_hardware(), detect_hardware_capabilities(), hardware_capabilities(), HardwareAcceleration, HardwareApi, Amf (+25 more)

### Community 14 - "SubtitleProcessingOptions"
Cohesion: 0.22
Nodes (4): build_subtitle_filter(), build_subtitle_filter_chain(), build_subtitles(), SubtitleProcessingOptions

### Community 15 - "MetadataProcessingOptions"
Cohesion: 0.07
Nodes (19): build_chapter_filter(), build_chapter_filter_chain(), build_chapters(), Chapter, ChapterMode, Copy, Edit, None (+11 more)

### Community 16 - "el"
Cohesion: 0.11
Nodes (37): bindConfigureCountPillTooltip(), bindConfigureInlineControlHover(), bindConfigureModifierPillHover(), buildConfigureActionControl(), buildConfigureCountControl(), buildConfigureRow(), buildConfigureSubmitButton(), buildConfigureTrailingCluster() (+29 more)

### Community 17 - "setLiveState"
Cohesion: 0.16
Nodes (40): beginNewLiveConfiguration(), buildPickedAnchorSnapshot(), cancelEditing(), cancelEditingToPicking(), cancelInsertConfigure(), cleanupAcceptedSession(), clearAnnotations(), clearInsertPicking() (+32 more)

### Community 18 - "actOnAgentTarget"
Cohesion: 0.11
Nodes (41): actOnAgentTarget(), agentTargetBusyReason(), agentTargetOverlayGone(), agentTargetTaken(), claimAgentTarget(), claimAndActOnAgentTarget(), clearStoredManualApplyState(), declineAgentTargetBusy() (+33 more)

### Community 19 - "applyEditing"
Cohesion: 0.10
Nodes (29): addManualContextText(), applyEditing(), buildLocatorForLeaf(), canRestoreManualEditElement(), collectManualContextPieces(), walk(), contextElementForManualEdit(), copyEditContainerContext() (+21 more)

### Community 20 - "AudioProcessingOptions"
Cohesion: 0.10
Nodes (16): AudioMixdown, Auto, Dolby, Mono, Stereo, Surround51, Surround71, AudioMode (+8 more)

### Community 21 - "media.rs"
Cohesion: 0.12
Nodes (22): ChapterInfo, convert_chapter(), convert_format(), convert_stream(), FfprobeChapter, FfprobeFormat, FfprobeResponse, FfprobeStream (+14 more)

### Community 22 - "showToast"
Cohesion: 0.09
Nodes (37): abandonForeignSession(), abandonSupersededGo(), cleanup(), clearHandled(), clearMountErrorCard(), clearScrollY(), copyToClipboard(), discardedWrappers() (+29 more)

### Community 23 - "Session"
Cohesion: 0.07
Nodes (26): add_job(), cancel_job(), clear_logs(), DesktopState, list_jobs(), logs(), pause_job(), push_log() (+18 more)

### Community 24 - "SKILL.md"
Cohesion: 0.11
Nodes (19): Assess Adaptation Challenge, Implement & Verify, Recommended Actions, Craft (deprecated alias), Apply, Live-mode signature params, Set the spatial thesis, Two isolated assessments (+11 more)

### Community 25 - "showBar"
Cohesion: 0.16
Nodes (19): actionLabel(), applyConfigureBarChrome(), buildConfirmedRow(), buildCyclingRow(), buildDots(), buildGeneratingRow(), cycleVariant(), cyclingCounterText() (+11 more)

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
Cohesion: 0.05
Nodes (37): Architecture decision snapshot, Deferred / Future Work, Group 0 — Project foundation, Group 10 — Encoding control and job reliability, Group 11 — Hardware acceleration, Group 12 - Platform packaging and release, Group 13 — Worker process and JSON IPC, Group 14 — Desktop parity with reference product (+29 more)

### Community 30 - "Responsive Design"
Cohesion: 0.08
Nodes (25): Assess Adaptation Challenge, Breakpoints: Content-Driven, Content Adaptation, Desktop Adaptation (Mobile → Desktop), Detect Input Method, Not Just Screen Size, Email Adaptation (Web → Email), Implement Adaptations, Layout Adaptation Patterns (+17 more)

### Community 31 - "pipeline.rs"
Cohesion: 0.13
Nodes (12): ContainerFormat, Mkv, Mp4, Webm, audio_encoder(), infer_container(), merge_video_options(), parse_codecs() (+4 more)

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
Cohesion: 0.08
Nodes (36): averageRgb01(), beginEditPin(), bufferToBase64(), buildAnnotationsForCapture(), buildPinElement(), cancelEditingPin(), captureChromeNodes(), captureElementFromRenderedAncestor() (+28 more)

### Community 38 - "EncodingMode"
Cohesion: 0.09
Nodes (17): EncodingMode, Abr, AverageBitrate, AverageBitrateMode, ConstantQuality, ConstantQualityMode, Cq, Crf (+9 more)

### Community 39 - "createLiveBrowserSessionState"
Cohesion: 0.22
Nodes (15): createLiveBrowserSessionState(), clearHandled(), clearScrollY(), clearSession(), isHandled(), loadSession(), markHandled(), nextCheckpointRevision() (+7 more)

### Community 41 - "FfprobeScanner"
Cohesion: 0.19
Nodes (7): CropDetection, detect_crop(), detect_crop_with(), extract_preview(), extract_preview_with(), FfprobeScanner, find_tool()

### Community 42 - "VideoProcessingOptions"
Cohesion: 0.16
Nodes (6): build_video_filter_chain(), FrameRateMode, Auto, Constant, Variable, VideoProcessingOptions

### Community 43 - "renderDesignVisual"
Cohesion: 0.08
Nodes (35): buildCollapsible(), buildColorModels(), buildListHtml(), buildRadiiModels(), buildTypographyModels(), cssSafe(), designEmptyMessage(), escapeHtml() (+27 more)

### Community 44 - "animate.md"
Cohesion: 0.12
Nodes (14): Accessibility and control, Choose material by meaning, Find the job, Implement to the runtime, Set the motion thesis, Timing and easing, Verify, Visitor mode (+6 more)

### Community 45 - "Handle `generate`"
Cohesion: 0.12
Nodes (16): 1. Read the screenshot (if present), 2. Wrap the element, 3. Load the action's reference, 4. Plan three variants: identity first, then mode, then axes, 5. Apply the freeform prompt (if present), 6. Deliver variants, 7. Parameters (composition-sized, 0-4 per variant), 8. Signal done (+8 more)

### Community 46 - "presets.rs"
Cohesion: 0.15
Nodes (4): roundtrip_import_export(), settings_persistence(), tmp_path(), user_create_edit_delete_persists()

### Community 47 - "AudioCodec"
Cohesion: 0.13
Nodes (19): apply(), load(), parse_audio(), parse_container(), parse_video(), Preset, AudioCodec, Aac (+11 more)

### Community 48 - "Generate Report"
Cohesion: 0.13
Nodes (14): 1. Accessibility (A11y), 2. Performance, 3. Theming, 4. Responsive Design, 5. Implementation Integrity (CRITICAL), Audit Health Score, Detailed Findings by Severity, Diagnostic Scan (+6 more)

### Community 49 - "restoreSessionWithoutWrapper"
Cohesion: 0.15
Nodes (20): applySavedSessionMeta(), clampVariantIndex(), findActiveSessionSummary(), findAdoptableServerSession(), findAnyVariantsWrapper(), isFrameworkComponentPreviewMode(), isSessionHandled(), isSvelteComponentManifestPath() (+12 more)

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
Cohesion: 0.12
Nodes (27): acceptedDomAlreadyClean(), applyOriginalAttrsToSvelteAnchor(), clearHandledWrapperReloadStamp(), commitAcceptedSvelteComponentToDom(), componentModuleCandidates(), deferredRecoverySuperseded(), describeMountFailure(), detectDevServerBase() (+19 more)

### Community 54 - "ops.rs"
Cohesion: 0.26
Nodes (9): BatchArgs, TranscodeArgs, batch_command(), BatchFile, BatchJob, CANCEL_PATH, setup_cancel_handler(), transcode_command() (+1 more)

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

### Community 62 - "src/cli.rs"
Cohesion: 0.23
Nodes (9): Cli, Commands, Batch, Presets, Transcode, parse(), PresetCommands, Inspect (+1 more)

### Community 63 - "resolveLiveInjectionAnchor"
Cohesion: 0.16
Nodes (19): buildSvelteExpressionTextMap(), buildSveltePropValuesFromLiveElement(), buildSveltePropValuesV2(), cloneWithoutElements(), collectTextNodes(), collectVisibleTexts(), cssEscapeIdent(), elementMatchesOriginalMarkup() (+11 more)

### Community 64 - "initGlobalBar"
Cohesion: 0.20
Nodes (16): buildDesignHeader(), clearSteerFocusRecoverTimer(), fetchDesignSystem(), hideAgentPollTooltip(), initGlobalBar(), makeIconBtn(), loadDetectScript(), onDetectMessage() (+8 more)

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

### Community 72 - "handleInsertCreate"
Cohesion: 0.15
Nodes (15): buildInsertPlaceholderSnapshotFromDom(), captureAndEmit(), checkpointPayload(), compileShader(), enterRecoveryWaitingForAnchor(), handleInsertCreate(), queueCheckpoint(), resolveScrollLockAnchorTop() (+7 more)

### Community 73 - "subtitles.rs"
Cohesion: 0.31
Nodes (6): burn_in_runs(), generate_source(), scanner(), srt_to_mp4_text_conversion(), text_subtitle_passthrough(), tool()

### Community 74 - "Init flow"
Cohesion: 0.20
Nodes (10): Completion gate, Init flow, Step 1: Load current state, Step 2: Explore the project, Step 3: Interview for product truth, Step 4: Write PRODUCT.md, Step 5: Record workflow defaults, Step 6: Wrap up or resume (+2 more)

### Community 75 - "SubtitleFormat"
Cohesion: 0.12
Nodes (15): SubtitleFormat, Ass, Auto, ClosedCaption, Dvb, Dvd, MovText, Pgs (+7 more)

### Community 76 - "transcode_baseline.rs"
Cohesion: 0.40
Nodes (7): end_to_end_job_spec(), generate_source(), scanner(), tool(), transcode_to_mkv(), transcode_to_mp4(), transcode_to_webm()

### Community 77 - "media_ingestion.rs"
Cohesion: 0.18
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

### Community 81 - "startVoice"
Cohesion: 0.27
Nodes (12): finishVoiceSession(), isEmbeddedPreviewBrowser(), releaseVoiceEngine(), startVoice(), steerSpeechRecognitionCtor(), steerVoiceContext(), steerVoiceErrorMessage(), steerVoiceUnavailableMessage() (+4 more)

### Community 82 - "audio_processing.rs"
Cohesion: 0.28
Nodes (3): generate_source(), tool(), transcode_with_audio_processing()

### Community 83 - "init"
Cohesion: 0.21
Nodes (14): barPaletteForTheme(), buildSteerProcessingDots(), designPanelCss(), detectPageTheme(), init(), initActionPicker(), initBar(), initDesignPanel() (+6 more)

### Community 84 - "deserialize"
Cohesion: 0.21
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

### Community 90 - "syncAgentPollingUi"
Cohesion: 0.21
Nodes (12): agentHasWorkInFlight(), agentStatusText(), brandMarkSvg(), buildSteerQueueHint(), ensureAgentPollTooltip(), fetchAgentPollingStatus(), showAgentPollTooltip(), startAgentStatusPoll() (+4 more)

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

### Community 116 - "subtitle.rs"
Cohesion: 0.22
Nodes (6): SubtitleMode, BurnIn, None, Passthrough, subtitle_codec_for(), SubtitleTrackConfig

### Community 119 - "vise-core"
Cohesion: 0.67
Nodes (3): vise-cli, vise-core, vise-desktop

### Community 122 - "SubtitleCodec"
Cohesion: 0.18
Nodes (10): SubtitleCodec, Ass, Copy, DvbSub, DvdSub, MovText, Srt, Ttml (+2 more)

### Community 123 - "steerFocusLog"
Cohesion: 0.33
Nodes (10): attachSteerFocusDebug(), attachSteerFocusGuard(), focusConfigureInput(), notePagePointerDown(), pageHasHostTextSelection(), scheduleSteerFocusRecover(), shouldFocusSteerChat(), steerFocusDebugEnabled() (+2 more)

### Community 124 - "unlockSteerChat"
Cohesion: 0.31
Nodes (9): clearSteerAwaitTimer(), pageChatCollapsedWidthPx(), pageChatExpandedWidth(), scheduleSteerAwaitTimeout(), sendSteerCheckpoint(), steerTimeoutMessage(), submitSteerMessage(), syncPageChatExpandedWidth() (+1 more)

### Community 125 - "handleMouseMove"
Cohesion: 0.33
Nodes (7): cursorForInsertAxis(), ensureInsertLine(), handleMouseMove(), hideInsertLine(), setPageInteractionCursor(), showInsertLine(), syncPageInteractionCursor()

### Community 126 - "Local Setup"
Cohesion: 0.29
Nodes (6): 1. Install Rust, 2. Install FFmpeg, 3. Install Visual Studio C++ Build Tools, 4. Build and test, 5. Manual smoke test, Local Setup

### Community 127 - "Vise Transcoder"
Cohesion: 0.29
Nodes (6): Build and test, Documentation, Notes, Requirements, Run the CLI, Vise Transcoder

## Knowledge Gaps
- **130 isolated node(s):** `vise-desktop`, `vise-cli`, `Transcode`, `Batch`, `Presets` (+125 more)
  These have ≤1 connection - possible missing edges or undocumented components. (Counts symbols only; 742 node(s) total have ≤1 connection when file, concept and rationale nodes are included.)
- **17 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `init()` connect `init` to `initGlobalBar`, `live-browser.js`, `createLiveBrowserDomHelpers`, `captureElementToBlob`, `live.md`, `Init flow`, `connectSSE`, `setLiveState`, `doctor.md`, `actOnAgentTarget`, `steerFocusLog`, `handleMouseMove`?**
  _High betweenness centrality (0.181) - this node is a cross-community bridge._
- **What connects `vise-desktop`, `vise-cli`, `Transcode` to the rest of the system?**
  _130 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `live-browser.js` be split into smaller, more focused modules?**
  _Cohesion score 0.04701962236208811 - nodes in this community are weakly interconnected._
- **Why does `Commands` connect `init` to `SKILL.md`?**
  _High betweenness centrality (0.072) - this node is a cross-community bridge._
- **Should `connectSSE` be split into smaller, more focused modules?**
  _Cohesion score 0.13178294573643412 - nodes in this community are weakly interconnected._
- **Why does `JobSpec` connect `JobSpec` to `EncodingMode`, `VideoProcessingOptions`, `hardware.rs`, `SubtitleProcessingOptions`, `MetadataProcessingOptions`, `AudioCodec`, `AudioProcessingOptions`, `subtitle.rs`, `Session`, `vise-core/src/preset.rs`, `pipeline.rs`?**
  _High betweenness centrality (0.034) - this node is a cross-community bridge._
- **Should `modern-screenshot.umd.js` be split into smaller, more focused modules?**
  _Cohesion score 0.08897243107769423 - nodes in this community are weakly interconnected._