# GOAL: Reimagine Vise Transcoder — Complete Premium Desktop UI/UX Redesign

## Mission

You are continuing development of **Vise Transcoder** after a hiatus.

Repository: https://github.com/edisonmalasan/Vise

The application's previous development phase successfully implemented and verified its media-transcoding capabilities. However, its current UI/UX is not visually or experientially satisfactory.

**Your primary objective is to comprehensively redesign and implement Vise's desktop interface into a modern, premium, professional-grade media transcoding workspace using the installed `modern-web-guidance` skill.**

This is not a minor CSS refresh, a component cleanup, or an exercise in making existing design measurements pass.

You are authorized to challenge and replace the existing interface's layout, styling, navigation, visual hierarchy, and presentation patterns.

The outcome must feel like a carefully designed desktop application that people would genuinely want to use.

## Mandatory skill invocation

Before modifying any frontend implementation:

1. Discover and activate the installed `modern-web-guidance` skill in OpenCode.
2. Read its complete `SKILL.md` instructions.
3. Follow the skill's prescribed search, retrieval, implementation, and verification workflow.
4. Retrieve applicable guidance for modern CSS layouts, dialogs, form controls, tab navigation, accessibility, themes, focus management, and responsive UI.
5. Follow browser compatibility and fallback guidance appropriate to Tauri's platform-specific WebViews.
6. Revisit applicable guidance as each significant UI component is implemented.

Do not merely mention the skill in the final report. Actually use it.

If the skill is unavailable, report the exact issue. Do not pretend that it was invoked successfully.

You may use existing design-oriented skills as supplementary references, but `modern-web-guidance` must govern modern web-platform implementation decisions.

## Mandatory OpenCode superpowers workflow

In addition to `modern-web-guidance`, this work must genuinely use the installed **OpenCode superpowers plugin** (the `superpowers` skills). Do not merely name the skills in the final report — invoke them and follow their prescribed process:

1. **`brainstorming`** — before any design or code: treat this GOAL as the detailed spec it is, write the design doc under `docs/superpowers/specs/`, and record the approaches considered. The user has pre-authorized autonomous execution ("start now, do not stop to ask"), so the approval gates are satisfied by this directive; do not pause to ask for confirmation.
2. **`writing-plans`** — turn the approved design into a concrete implementation plan before touching code.
3. **`using-git-worktrees`** — isolate the redesign on a dedicated branch/worktree; never push directly to `main`.
4. **`test-driven-development`** — for each behaviour change, write the failing check first, then the implementation; the existing Rust tests are the regression net.
5. **`systematic-debugging`** — when any UI binding, layout, or test fails, follow its root-cause process before patching.
6. **`verification-before-completion`** — run and read the real commands; make no success claim without visible command output.
7. **`finishing-a-development-branch`** — verify, merge the reviewed work, and update documentation before declaring done.

You may additionally use `dispatching-parallel-agents` / `subagent-driven-development`, `requesting-code-review`, and `humanizing-writing` where they fit. If a superpowers skill is unavailable, report the exact issue rather than pretending it was used.

## Phase 1 — Repository reconnaissance

Before designing or changing anything, inspect the actual repository.

Read these files:

- `AGENTS.md`
- `docs/CONTEXT.md`
- `docs/ROADMAP.md`
- `PRODUCT.md`
- `DESIGN.md`
- `apps/vise-desktop/README.md`
- `apps/vise-desktop/ui/index.html`
- `apps/vise-desktop/ui/styles.css`
- `apps/vise-desktop/ui/app.js`

Also inspect relevant Rust view models, Tauri commands, translations, tests, and existing design assets.

Understand how data and events move between the frontend, Tauri shell, and background worker.

Identify every existing UI surface and important interaction.

Review the existing application screenshots, and launch the application to inspect its actual rendered appearance whenever the environment supports it.

Do not base design decisions solely on repository documentation, old screenshots, or assumptions.

Produce a concise internal audit covering UX problems, visual weaknesses, unnecessary complexity, layout constraints, accessibility risks, and implementation dependencies.

## Phase 2 — Discard the previous visual direction

The previous design followed a prescribed "true-black flat" aesthetic with a fixed sidebar, a single-column five-step workspace, Inter typography, minimal borders, and a limited accent-color strategy.

**That visual direction is no longer an accepted design requirement.**

The current `DESIGN.md` describes the old implementation. Treat its aesthetic and structural rules as replaceable design history, not immutable requirements.

You may replace:

- The navigation rail and its placement
- The single-column step layout
- Card styles, borders, spacing, and surface treatment
- Typography and font choices
- Color palettes and accent-color behavior
- Header structure and hierarchy
- Toolbars, tabs, and secondary-window presentation
- Status, progress, and output-summary presentation

Preserve functional, architectural, and accessibility commitments unless a verified implementation improvement requires a compatible change.

Do not keep old decisions simply because a previous agent documented or tested them.

Do not optimize to satisfy the previous design-taste measurements if they enforce a design direction that is being replaced.

Update `DESIGN.md` after implementing the new design, so it accurately documents the new system.

## Phase 3 — Establish a premium design direction

Reimagine Vise as a **professional media workstation**.

Aim for a visual quality comparable to carefully crafted modern desktop creative tools, rather than an ordinary website, traditional settings form, or generic AI-generated dashboard.

The interface should communicate:

- Professional craftsmanship
- Fast and intuitive media workflows
- Strong information hierarchy
- Refined typography
- Modern, cohesive visual composition
- Readable technical data
- Thoughtful interaction feedback
- Efficient use of desktop screen space
- A distinct product identity

Create a unified visual system covering surfaces, colors, typography, density, spacing, icons, controls, and motion.

A refined dark appearance is a reasonable starting point, but do not blindly reproduce the existing pure-black aesthetic.

Support a genuinely designed light theme as well.

Use accent colors with purpose, but do not artificially restrict them to the previous rules.

Avoid generic SaaS dashboards, oversized rounded cards, excessive empty space, unnecessary explanatory text, decorative gradients, random visual effects, and features added solely to fill the screen.

Do not use emoji as substitutes for a consistent icon system.

Fonts and essential assets must remain available offline in the packaged desktop application.

## Phase 4 — Redesign the user experience

The user should be able to understand the primary workflow immediately:

**Choose media → Inspect source → Select preset/configure output → Set destination → Queue and encode.**

### Main workspace

Design the main window around the user's current task.

Prioritize source selection and inspection, relevant media information, output configuration, preset selection, destination selection, and the primary encoding action.

Avoid forcing users through an excessively long page of equally weighted panels.

Use available desktop space intelligently. Consider a contextual inspector, clearly separated settings regions, a persistent job summary, or an appropriately sized configuration workspace.

Use progressive disclosure so common settings are easy to reach and advanced settings remain discoverable.

Do not hide important functionality without a clear access path.

### Navigation

Reconsider the current rail containing secondary windows and appearance controls.

Create a coherent application-level navigation model that distinguishes primary destinations, contextual tools, configuration tabs, and preferences.

Queue management, presets, preview, logs, diagnostics, and preferences must remain accessible without cluttering the main workflow.

### Media information

Present source duration, resolution, frame rate, codecs, audio tracks, subtitles, chapters, and relevant metadata clearly.

Use technical typography and appropriate grouping without turning the entire application into a dense table of numbers.

Make source and output information easy to distinguish.

### Output configuration

Preserve the existing configuration capabilities across Summary, Dimensions, Filters, Video, Audio, Subtitles, Chapters, and Metadata.

Improve their organization and visual hierarchy.

Keep all fields, constraints, current values, available options, validation states, and keyboard interactions connected to the actual application logic.

### Presets

Make the existing 138-preset catalogue intuitive to browse and select.

Communicate preset categories, selected values, hardware compatibility, substitutions, and unavailable options.

Do not create fictional capabilities or static preset results.

### Queue and progress

Give encoding progress, queue state, current job, completion, failures, pause, resume, cancellation, and retry controls appropriate visibility.

Make state changes understandable without requiring the user to interpret raw logs.

### Empty and exceptional states

Design all major states, including:

- No media selected
- Source scanning
- Successfully scanned media
- Unsupported or invalid source
- No compatible hardware encoder
- No available titles or tracks
- Valid and invalid output settings
- Empty and populated queue
- Active encode, completion, cancellation, and failure

Each state must communicate the next useful action without unnecessary paragraphs of interface copy.

## Phase 5 — Implement using modern web standards

Keep the existing Tauri/Rust architecture and preserve its worker isolation.

The frontend currently uses vanilla HTML, CSS, and JavaScript without a build step.

Prefer improving that architecture rather than introducing React, Vue, a CSS framework, or a new build pipeline without a concrete, documented justification.

Apply relevant guidance from `modern-web-guidance` to:

- Semantic HTML and accessibility
- CSS Grid, Flexbox, and responsive layouts
- Modern form controls
- Dialogs and overlays
- Keyboard navigation and focus handling
- Light and dark themes
- Accessible status announcements
- Motion and reduced-motion preferences
- Browser performance and compatibility
- Safe DOM updates and event handling

Choose native web-platform primitives when appropriate, but verify support in Windows WebView2, macOS WKWebView, and Linux WebKitGTK.

Do not adopt a newer API solely because it is modern if it breaks a supported desktop platform.

Avoid unnecessary runtime dependencies and remote asset requests.

Do not replace real application functionality with placeholder or mocked functionality.

Preserve the existing Tauri commands, translations, application state, input handling, and worker communication contracts.

Do not rewrite the Rust backend merely to accommodate visual changes.

## Phase 6 — Preserve existing functionality

The redesign must not regress existing capabilities.

Maintain source scanning, title/range/angle selection, audio and subtitle selection, transcoding settings, presets, queues, preview, logs, preferences, diagnostics, and supported hardware capabilities.

Maintain runtime language switching across the six shipped languages.

Preserve right-to-left behavior, theme selection, visible keyboard focus, appropriate semantic accessibility, and reduced-motion support.

The previous localization defect was fixed. Do not reintroduce it by hardcoding new interface strings outside the established translation system.

Review UI-to-Rust bindings carefully when moving, renaming, or rebuilding controls.

A visually impressive static mockup that cannot perform the original actions is not an acceptable result.

## Phase 7 — Real visual verification

**Passing tests alone does not satisfy this goal.**

Run the application and inspect the actual rendered interface.

Verify at representative desktop sizes, including approximately:

- 1024 × 768
- 1280 × 800
- 1440 × 900
- 1920 × 1080

Test dark and light themes, keyboard navigation, appropriate RTL behavior, translated labels, dialog interactions, and important application states.

Use actual desktop screenshots wherever possible.

At minimum, visually inspect:

1. Initial empty workspace
2. Scanned source and media details
3. Preset selection
4. Advanced output configuration
5. Queue and active progress
6. Preferences and secondary surfaces

For each inspection, identify visual defects, fix them, and inspect the result again.

Look for clipping, overlapping elements, horizontal overflow, wasted space, inconsistent spacing, weak contrast, misaligned controls, missing states, and confusing action hierarchy.

If native desktop capture is unavailable, use an honest browser-rendering fallback for visual checks, clearly distinguish it from verified Tauri behavior, and report the native verification as incomplete.

Do not claim visual quality has been verified when screenshots or an actual rendered application have not been inspected.

## Phase 8 — Technical verification

Run the relevant existing validation commands:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

Run applicable frontend and platform-specific checks as well.

Verify primary end-to-end flows against the running application where supported.

Record failures and unavailable checks accurately.

Do not invent test results or rely on the previous development session's passing numbers.

Update affected documentation and screenshots to reflect the new implementation.

Follow the repository's branch-based Git workflow. Do not overwrite unrelated work or push directly to `main`.

Keep redesign changes on a dedicated branch for review before merging.

## Definition of done

The goal is complete only when:

- The installed `modern-web-guidance` skill was genuinely invoked and applied.
- The installed OpenCode superpowers plugin was genuinely used (brainstorming/design doc, writing-plans, git worktree/branch, TDD, systematic debugging, verification, branch finishing), with evidence in the repository and the report.
- The old UI has been substantially redesigned, not merely recolored.
- The new interface demonstrates a cohesive and distinctive visual identity.
- Core workflows are more discoverable and efficient.
- All existing major capabilities remain accessible and functional.
- New and modified controls are connected to real application behavior.
- The interface supports light/dark themes, keyboard accessibility, localization, and RTL.
- Representative UI states have been visually inspected and iteratively improved.
- No known significant layout, contrast, or interaction defects remain.
- Relevant tests and build checks pass, or unresolved failures are explicitly reported.
- `DESIGN.md` and affected documentation accurately describe the new implementation.
- Before/after visual evidence is available for review.

## Completion reporting

At the end, provide:

1. Summary of the previous UI's concrete problems.
2. Explanation of the chosen design direction and its rationale.
3. Major interface changes and affected files.
4. Modern Web Guidance references actually retrieved and applied, following the skill's reporting instructions.
5. OpenCode superpowers skills actually invoked and applied, with evidence (design doc, plan, branch, checks).
6. Before/after screenshots and verified UI states.
7. Functional regression verification results.
8. Technical test results.
9. Remaining issues, limitations, and unverified conditions.

## Critical instruction

**Do not declare this goal achieved merely because the UI renders, the CSS was rewritten, a design inspection reports zero warnings, or the existing Rust tests pass.**

The previous development phase already met those types of measurements, yet the interface remained unsatisfactory.

The objective is a substantially better designed, genuinely functional, professional desktop experience.

Make thoughtful design decisions, implement them, evaluate the rendered result, and iterate.

If the visual redesign or critical functional verification is incomplete, report the goal as incomplete rather than declaring success prematurely.

**Deliver a meaningful transformation of Vise Transcoder, not another superficial restyling.**