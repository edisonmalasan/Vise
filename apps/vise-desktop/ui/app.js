/* Vise Transcoder desktop page.
 *
 * The page is a renderer. Every decision about a source, a scan, a title, a
 * range, a track, or an output setting is made in the window (Rust) and
 * arrives here as a finished object, so this file only wires controls to
 * commands and paints the result. No media logic, no formatting rules: what the
 * user reads is built and tested in Rust.
 */
(function () {
  "use strict";

  var bridge = window.__TAURI__ || null;
  var invoke = bridge && bridge.core ? bridge.core.invoke : null;

  /* Everything the page is holding.
   *
   * The view models belong to the window: each one is the finished answer to a
   * question, and the page keeps it only so it can repaint the same surface
   * between two commands. Nothing here is a decision.
   */
  var state = {
    selection: null,
    recents: [],
    pending: 0,
    queue: null,
    tabs: null,
    importedSubtitle: "",
    /* The catalog of secondary surfaces, and the description of each one. */
    catalog: [],
    specs: {},
    /* The surfaces that are open, front last. */
    open: [],
    front: "",
    restoreFocus: "",
    /* The view model behind each surface. */
    queueSelection: null,
    activity: null,
    preview: null,
    errors: null,
    about: null,
    presets: null,
    alert: null,
    /* The preset form, which the page holds so the fields it clears on submit
       are the ones it is about to send. */
    presetForm: {
      action: "create",
      id: "",
      name: "",
      description: "",
      category: "general",
      default: false,
      path: ""
    },
    /* What the window says this window looks like: the theme it resolved and
       the direction it resolved. Every surface in this document reads these two
       values, so a dialog cannot end up in a different theme or a different
       direction from the page behind it.

       Seeded from the document rather than from a pair of constants, because the
       inline script in the head has already put the window's last answer there
       and the tab strip answers the arrow keys before the bridge has spoken. */
    appearance: {
      theme: document.documentElement.dataset.theme === "dark" ? "dark" : "light",
      direction: document.documentElement.dir === "rtl" ? "rtl" : "ltr"
    },
    /* The words the window is written in, by name. The page composes nothing: an
       element says which message it wants and the window's language decides what
       that message is. Empty until the window answers, which is what leaves the
       markup's own text in place on the first paint. */
    strings: {},
    /* What the countdown alert is holding on the user's behalf. */
    confirm: null,
    alertTimer: null,
    /* The preferences surface, held whole because one change can move a control
       on another tab: the collision switch settles the collision choice, and a
       record this build cannot write closes every tab at once. */
    preferences: null,
    /* What to run for an alert the window opened itself. Written down here
       rather than sent by the window, because a command name is not data the
       window can send. */
    confirmRunners: {}
  };

  function el(id) {
    return document.getElementById(id);
  }

  function clear(node) {
    while (node.firstChild) {
      node.removeChild(node.firstChild);
    }
  }

  function setText(node, value) {
    node.textContent = value == null ? "" : String(value);
  }

  function show(node, visible) {
    node.hidden = !visible;
  }

  /* A button that is not on offer stays out of the layout and out of the tab
     order; both halves have to move together or a hidden button still eats
     clicks. */
  function offer(node, available) {
    show(node, available);
    if ("disabled" in node) {
      node.disabled = !available;
    }
  }

  /* ------------------------------------------------------------ small parts */

  function node(tag, className, text) {
    var made = document.createElement(tag);
    if (className) {
      made.className = className;
    }
    if (text != null) {
      made.textContent = String(text);
    }
    return made;
  }

  /* A line of text. Nothing here formats a value: the window has already built
     the sentence, and this only puts it in a paragraph. */
  function para(text, className) {
    return node("p", className || "hint", text == null ? "" : text);
  }

  function emptyNote(text) {
    return node("div", "empty", text);
  }

  /* A control that is not on offer is disabled and carries the reason as its
     tooltip, so nothing in the window can be pressed without an explanation of
     why it did nothing. */
  function control(node, control, onUse) {
    node.disabled = !control.available;
    if (control.available) {
      node.removeAttribute("title");
    } else {
      node.setAttribute("title", control.reason);
      node.setAttribute("aria-describedby", "");
    }
    if (onUse) {
      node.onclick = control.available
        ? function () {
            onUse();
          }
        : null;
    }
    return node;
  }

  function actionButton(action, onUse) {
    var button = node("button", action.confirms ? "warn" : "", action.label);
    button.type = "button";
    button.setAttribute("data-action", action.id);
    if (!action.available) {
      button.setAttribute("title", action.reason);
    }
    button.disabled = !action.available;
    button.onclick = action.available
      ? function () {
          onUse(action);
        }
      : null;
    return button;
  }

  function labelledField(labelText, input, hintText, tone) {
    var field = node("div", "field");
    var label = node("label", null, labelText);
    label.htmlFor = input.id;
    field.appendChild(label);
    field.appendChild(input);
    var hint = node("span", "hint", hintText || "");
    hint.id = input.id + "Hint";
    if (tone) {
      hint.dataset.tone = tone;
    }
    input.setAttribute("aria-describedby", hint.id);
    field.appendChild(hint);
    return field;
  }

  /* Put text into a field without moving the caret of whoever is typing in it. */
  function fill(input, value) {
    if (document.activeElement !== input && input.value !== (value || "")) {
      input.value = value || "";
    }
  }

  /* ---------------------------------------------------------------- bridge */

  /* Where the last answer the window gave is kept.
   *
   * The page cannot be told what theme it is in before its first paint, because
   * the answer arrives over the bridge. The inline script at the top of the
   * document replays what is stored here so the first frame is already right;
   * this is a record of what the window said, not a preference of its own.
   */
  var APPEARANCE_CACHE_KEY = "vise.appearance";

  function unavailable(message) {
    var notice = el("scanNotice");
    setText(notice, message);
    notice.dataset.tone = "error";
    show(notice, true);
  }

  function call(command, args) {
    if (!invoke) {
      return Promise.reject(new Error("the desktop bridge is unavailable"));
    }
    try {
      return Promise.resolve(invoke(command, args || {}));
    } catch (error) {
      // A bridge that throws instead of rejecting would otherwise leave the
      // control looking broken with no explanation.
      return Promise.reject(error);
    }
  }

  /* ---------------------------------------------------------------- notices */

  function notice(tone, message) {
    var node = el("scanNotice");
    if (!message) {
      show(node, false);
      return;
    }
    setText(node, message);
    node.dataset.tone = tone;
    show(node, true);
  }

  /* ----------------------------------------------------------------- render */

  function renderSource(selection) {
    var current = el("sourceCurrent");
    var source = selection.source;
    show(current, Boolean(source));
    offer(el("clearSourceBtn"), Boolean(source));
    offer(el("scanBtn"), Boolean(source));
    if (source) {
      setText(el("sourceKind"), source.kind === "folder" ? "Folder" : "File");
      setText(el("sourceName"), source.name);
      setText(el("sourceLocation"), source.location);
      if (!el("pathInput").value) {
        el("pathInput").value = source.path;
      }
    } else {
      el("pathInput").value = "";
    }
  }

  function renderScan(selection) {
    var progress = selection.progress || {};
    var line = el("scanStatus");
    var bar = el("scanProgress");
    var fill = el("scanProgressFill");

    if (selection.phase === "scanning") {
      setText(line, progress.statusLine || "Scanning…");
      show(bar, Boolean(progress.visible));
      fill.style.width = (progress.percent || 0) + "%";
      bar.setAttribute("aria-valuenow", String(progress.percent || 0));
      bar.setAttribute("role", "progressbar");
      bar.setAttribute("aria-valuemin", "0");
      bar.setAttribute("aria-valuemax", "100");
    } else if (selection.phase === "ready") {
      setText(line, "Scan finished.");
      show(bar, false);
    } else {
      setText(line, "No source scanned yet.");
      show(bar, false);
    }

    if (selection.phase === "failed") {
      notice("error", selection.problem || "The scan did not finish.");
    } else if (selection.phase === "ready" && selection.problem) {
      notice("muted", selection.problem);
    } else {
      notice(null, "");
    }
  }

  /* A single-choice row.
 *
 * Native radios rather than a button wearing a radio role: the group then gets
 * arrow-key navigation, one tab stop, and the platform's own announcement for
 * free, which a hand-rolled role cannot match.
 */
function choiceRow(item, group, checked, unavailable, onSelect) {
    var wrapper = document.createElement("label");
    wrapper.className = "choice";
    var input = document.createElement("input");
    input.type = "radio";
    input.name = group;
    input.value = item.id || "";
    input.checked = Boolean(checked);
    input.disabled = Boolean(unavailable);
    input.addEventListener("change", function () {
      if (input.checked) {
        onSelect();
      }
    });
    var text = document.createElement("span");
    text.className = "text";
    var label = document.createElement("span");
    label.textContent = item.label;
    text.appendChild(label);
    if (item.facts) {
      var facts = document.createElement("span");
      facts.className = "facts";
      facts.textContent = item.facts;
      text.appendChild(facts);
    }
    wrapper.appendChild(input);
    wrapper.appendChild(text);
    return wrapper;
  }

  function renderTitles(selection) {
    var list = el("titleList");
    clear(list);
    var titles = selection.titles || [];
    setText(el("titleCount"), titles.length ? titles.length + " found" : "");
    show(el("titleEmpty"), titles.length === 0);

    titles.forEach(function (title) {
      var item = document.createElement("li");
      item.appendChild(
        choiceRow(
          {
            id: String(title.index),
            label: title.label,
            facts: title.facts
          },
          "titles",
          title.index === selection.selectedTitleIndex,
          false,
          function () {
            run("choose_title", { index: title.index }, "Could not choose that title.");
          }
        )
      );
      list.appendChild(item);
    });
  }

  function renderRange(selection) {
    var range = selection.range;
    var modeList = el("rangeModeList");
    clear(modeList);
    var pointRow = el("pointRow");
    var angleRow = el("angleRow");
    var chapterList = el("chapterList");
    var chapterEmpty = el("chapterEmpty");
    clear(chapterList);

    show(pointRow, false);
    show(angleRow, false);
    show(chapterEmpty, true);
    setText(el("rangeSummary"), "");

    var rangeNotice = el("rangeNotice");
    show(rangeNotice, false);

    if (!range) {
      setText(el("rangeSummary"), "Choose a title to set a range.");
      return;
    }

    setText(
      el("rangeSummary"),
      range.startLabel + " – " + range.endLabel + " (" + range.durationLabel + ")"
    );

    range.modes.forEach(function (mode) {
      var item = document.createElement("li");
      item.appendChild(
        choiceRow(
          {
            id: mode.id,
            label: mode.label,
            facts: mode.available ? "" : mode.reason
          },
          "rangeMode",
          mode.id === range.mode,
          !mode.available,
          function () {
            // The points already resolved travel with the mode change, so a
            // mode that needs them opens on a usable range instead of an error.
            run(
              "choose_range",
              {
                choice: {
                  mode: mode.id,
                  startSeconds: range.startSeconds,
                  endSeconds: range.endSeconds,
                  angle: range.angle
                }
              },
              "Could not use that range mode."
            );
          }
        )
      );
      modeList.appendChild(item);
    });

    if (range.mode === "chapters" || range.mode === "point_to_point") {
      show(pointRow, true);
      fillPoint(el("startPoint"), range, range.startSeconds);
      fillPoint(el("endPoint"), range, range.endSeconds);
    }

    if (range.hasAngles) {
      show(angleRow, true);
      var angles = el("angleSelect");
      clear(angles);
      range.angles.forEach(function (angle) {
        var option = document.createElement("option");
        option.value = String(angle.index);
        option.textContent = angle.label;
        option.selected = angle.index === range.angle;
        angles.appendChild(option);
      });
    }

    if (range.chapters.length) {
      show(chapterEmpty, false);
      range.chapters.forEach(function (chapter) {
        var item = document.createElement("li");
        setText(item, chapter.label);
        chapterList.appendChild(item);
      });
    }

    if (!range.valid && range.problem) {
      setText(rangeNotice, range.problem);
      show(rangeNotice, true);
    }
  }

  function fillPoint(select, range, seconds) {
    clear(select);
    range.points.forEach(function (point) {
      var option = document.createElement("option");
      option.value = String(point.seconds);
      option.textContent = point.label;
      option.selected = Math.abs(point.seconds - seconds) < 0.001;
      select.appendChild(option);
    });
  }

  function trackCheckbox(track, group, onChange) {
    var wrapper = document.createElement("label");
    wrapper.className = "check";
    // The whole track reads as one phrase for a screen reader; the two visible
    // lines are the short form of that phrase and its detail.
    wrapper.setAttribute("aria-label", track.label);
    var input = document.createElement("input");
    input.type = "checkbox";
    input.checked = Boolean(track.selected);
    input.value = String(track.number);
    input.addEventListener("change", function () {
      var chosen = Array.prototype.slice
        .call(document.querySelectorAll('#' + group + ' input:checked'))
        .map(function (box) {
          return Number(box.value);
        });
      onChange(chosen);
    });
    var text = document.createElement("span");
    text.className = "text";
    var label = document.createElement("span");
    label.textContent = track.summary;
    var detail = document.createElement("span");
    detail.className = "facts";
    detail.textContent = track.detail;
    text.appendChild(label);
    text.appendChild(detail);
    wrapper.appendChild(input);
    wrapper.appendChild(text);
    return wrapper;
  }

  function renderTracks(selection) {
    var tracks = selection.tracks || { audio: [], subtitle: [] };
    var audio = el("audioList");
    var subtitles = el("subtitleList");
    clear(audio);
    clear(subtitles);
    show(el("audioEmpty"), tracks.audio.length === 0);
    show(el("subtitleEmpty"), tracks.subtitle.length === 0);
    setText(
      el("trackSummary"),
      tracks.audio.length
        ? tracks.audio.length + " audio · " + tracks.subtitle.length + " subtitle"
        : ""
    );

    function push(list, node) {
      var wrapper = document.createElement("div");
      wrapper.appendChild(node);
      list.appendChild(wrapper);
    }

    tracks.audio.forEach(function (track) {
      push(audio, trackCheckbox(track, "audioList", function (chosen) {
        run(
          "choose_tracks",
          { input: { audio: chosen, subtitle: selection.tracks.selectedSubtitle } },
          "Could not choose those audio tracks."
        );
      }));
    });

    tracks.subtitle.forEach(function (track) {
      push(subtitles, trackCheckbox(track, "subtitleList", function (chosen) {
        run(
          "choose_tracks",
          { input: { audio: selection.tracks.selectedAudio, subtitle: chosen } },
          "Could not choose those subtitle tracks."
        );
      }));
    });
  }

  function render(selection) {
    state.selection = selection;
    renderSource(selection);
    renderScan(selection);
    renderTitles(selection);
    renderRange(selection);
    renderTracks(selection);

    // The tabs describe the chosen title, so they are re-read whenever it or its
    // tracks change.
    refreshTabs();

    var usable = selection.phase === "ready" && selection.selectedTitleIndex > 0;
    if (!usable) {
      el("queueBtn").disabled = true;
      setText(el("queueHint"), "Scan a source and choose a title first.");
    } else if (state.tabs) {
      // renderTabs has already decided the button against the tab settings.
      setText(
        el("queueHint"),
        state.tabs.canQueue
          ? "Queues the chosen title with the settings on the tabs below."
          : "Fix the problems on the tabs below before queueing."
      );
    }
  }

  /* ------------------------------------------------------------------ calls */

  function run(command, args, failure) {
    state.pending += 1;
    return call(command, args)
      .then(function (result) {
        if (result && result.phase !== undefined) {
          render(result);
        } else if (result && result.tabs !== undefined) {
          renderTabs(result);
        }
        return result;
      })
      .catch(function (error) {
        var message = String(error && error.message ? error.message : error);
        notice("error", failure ? failure + " " + message : message);
        return null;
      })
      .then(function (result) {
        state.pending -= 1;
        return result;
      });
  }

  function refreshSelection() {
    return run("source_selection", {});
  }

  function loadRecents() {
    return call("recent_sources")
      .then(function (recents) {
        state.recents = recents || [];
        renderRecents();
      })
      .catch(function () {
        state.recents = [];
        renderRecents();
      });
  }

  function renderRecents() {
    var list = el("recentList");
    clear(list);
    if (!state.recents.length) {
      var empty = document.createElement("li");
      empty.className = "empty";
      setText(empty, "Sources you open will be listed here.");
      list.appendChild(empty);
      show(el("clearRecentsBtn"), false);
      return;
    }
    show(el("clearRecentsBtn"), true);
    state.recents.forEach(function (source) {
      var item = document.createElement("li");
      item.className = "source-current";
      var name = document.createElement("span");
      name.className = "name";
      setText(name, source.name);
      item.appendChild(name);
      var location = document.createElement("span");
      location.className = "location";
      setText(location, source.location);
      item.appendChild(location);

      var open = document.createElement("button");
      open.type = "button";
      open.textContent = "Open";
      open.addEventListener("click", function () {
        openSource(source.path);
      });
      item.appendChild(open);

      var forget = document.createElement("button");
      forget.type = "button";
      forget.className = "quiet";
      forget.textContent = "Forget";
      forget.setAttribute("aria-label", "Forget " + source.name);
      forget.addEventListener("click", function () {
        call("forget_recent_source", { path: source.path }).then(loadRecents);
      });
      item.appendChild(forget);
      list.appendChild(item);
    });
  }

  function openSource(path) {
    state.pending += 1;
    return call("select_source", { path: path })
      .then(function (selection) {
        render(selection);
        loadRecents();
      })
      .catch(function (error) {
        unavailable(
          "That source could not be opened. " +
            String(error && error.message ? error.message : error)
        );
      })
      .then(function () {
        state.pending -= 1;
      });
  }

  /* -------------------------------------------------------------- scan poll */

  var pollTimer = null;

  function startPolling() {
    if (pollTimer) {
      return;
    }
    pollTimer = window.setInterval(function () {
      if (state.pending > 0) {
        return;
      }
      call("scan_status")
        .then(function (status) {
          if (!status) {
            return;
          }
          var selection = state.selection || {};
          selection.phase = status.phase;
          selection.phaseName = status.phaseName;
          selection.problem = status.problem;
          if (status.progress && status.progress.statusLine) {
            selection.progress = status.progress;
          }
          if (status.phase === "scanning") {
            renderScan(selection);
            return;
          }
          stopPolling();
          refreshSelection();
        })
        .catch(function (error) {
          stopPolling();
          notice("error", "The scan could not be followed: " + error);
        });
    }, 250);
  }

  function stopPolling() {
    if (pollTimer) {
      window.clearInterval(pollTimer);
      pollTimer = null;
    }
  }

  function startScan() {
    setText(el("scanStatus"), "Starting scan…");
    state.pending += 1;
    /* No preview count is sent: the preview window owns how many stills a scan
       extracts, so the two cannot disagree. */
    return call("begin_scan", { options: {} })
      .then(function (status) {
        var selection = state.selection || {};
        selection.phase = status.phase;
        selection.phaseName = status.phaseName;
        selection.problem = status.problem;
        renderScan(selection);
        if (status.running) {
          startPolling();
        } else {
          refreshSelection();
        }
      })
      .catch(function (error) {
        notice(
          "error",
          "The scan could not be started. " +
            String(error && error.message ? error.message : error)
        );
      })
      .then(function () {
        state.pending -= 1;
      });
  }

  /* -------------------------------------------------------- secondary windows */

  /* Which renderer belongs to which surface.
   *
   * A command name is not data the window can send, so this routing table is
   * written down once, here, and nowhere else. Each entry asks the window for
   * the surface's view model and paints it.
   */
  var PAINTERS = {
    queue: function (shell) {
      return call("queue_window").then(function (view) {
        state.queue = view;
        paintQueue(shell, view);
      });
    },
    "queue-selection": function (shell) {
      return call("queue_selection_window").then(function (view) {
        state.queueSelection = view;
        paintSelection(shell, view);
      });
    },
    activity: function (shell) {
      return call("activity_window").then(function (view) {
        state.activity = view;
        paintActivity(shell, view);
      });
    },
    preview: function (shell) {
      return call("preview_window").then(function (view) {
        state.preview = view;
        paintPreview(shell, view);
        // The first time a surface opens with a source loaded and nothing on
        // screen, the frame is fetched once; after that it is left alone.
        if (view.hasSource && view.stillCount > 0 && view.empty) {
          return fetchStill().then(function (again) {
            paintPreview(shell, again);
          });
        }
        return null;
      });
    },
    error: function (shell) {
      return call("error_view").then(function (view) {
        state.errors = view;
        paintErrors(shell, view);
      });
    },
    about: function (shell) {
      return call("about_view").then(function (view) {
        state.about = view;
        paintAbout(shell, view);
      });
    },
    presets: function (shell) {
      return call("preset_window").then(function (view) {
        state.presets = view;
        paintPresets(shell, view);
      });
    },
    preferences: function (shell) {
      return call("preferences_view").then(function (view) {
        state.preferences = view;
        paintPreferences(shell, view);
      });
    }
  };

  /* Which surfaces keep themselves up to date while they are open. */
  var POLLED = { queue: 1500, error: 1500, activity: 1500 };

  function shellOf(id) {
    return el("surface-" + id);
  }

  function bodyOf(shell) {
    return el(shell.id + "-body");
  }

  function isOpen(id) {
    return state.open.indexOf(id) !== -1;
  }

  /* Build one surface shell from its description.
   *
   * The role, the modal flag, the heading, the close control, and the control
   * that takes focus all come from the window, so a surface cannot be presented
   * with accessibility semantics the window did not ask for.
   */
  function buildSurface(spec) {
    var shell = node("section", "surface" + (spec.kind === "window" ? " surface-wide" : ""));
    shell.id = "surface-" + spec.id;
    shell.setAttribute("role", spec.role);
    shell.setAttribute("aria-modal", spec.modal ? "true" : "false");
    shell.tabIndex = -1;
    shell.hidden = true;

    var head = node("header", "surface-head");
    var heading = node("h2", null, spec.label);
    heading.id = spec.id + "-label";
    shell.setAttribute("aria-labelledby", heading.id);
    head.appendChild(heading);
    head.appendChild(para(spec.hint));
    if (spec.escapeCloses) {
      var close = node("button", "quiet", "Close");
      close.type = "button";
      close.id = spec.id + "close";
      close.addEventListener("click", function () {
        closeSurface(spec.id);
      });
      head.appendChild(close);
    }
    shell.appendChild(head);

    var body = node("div", "surface-body");
    body.id = shell.id + "-body";
    shell.appendChild(body);
    return shell;
  }

  /* The toolbar and the surface shells, built from the catalog. */
  function loadCatalog() {
    return call("window_catalog").then(function (catalog) {
      state.catalog = catalog.surfaces || [];
      var bar = el("toolbar");
      var host = el("surfaces");
      clear(bar);
      clear(host);
      state.specs = {};
      /* One backdrop for every surface, so the window behind is dimmed and
         cannot be clicked while something is up. It is a real element rather
         than a pseudo-element because it has to take pointer events away from
         the whole window, not just tint it. */
      var backdrop = node("div", "backdrop");
      backdrop.id = "backdrop";
      backdrop.hidden = true;
      host.appendChild(backdrop);
      state.catalog.forEach(function (spec) {
        state.specs[spec.id] = spec;
        host.appendChild(buildSurface(spec));
        if (!spec.openable) {
          return;
        }
        var button = node("button", null, spec.label);
        button.type = "button";
        button.id = spec.buttonId;
        button.setAttribute("aria-haspopup", "dialog");
        button.title = spec.hint;
        button.addEventListener("click", function () {
          openSurface(spec.id);
        });
        bar.appendChild(button);
        if (spec.badge) {
          var badge = node("span", "badge", "");
          button.appendChild(badge);
        }
      });
    });
  }

  /* Put a count on the toolbar button of a surface that keeps one. */
  function setBadge(count) {
    state.catalog.forEach(function (spec) {
      if (!spec.badge) {
        return;
      }
      var button = el(spec.buttonId);
      var badge = button && button.querySelector(".badge");
      if (badge) {
        badge.textContent = count ? String(count) : "";
        badge.hidden = !count;
      }
    });
  }

  function openSurface(id) {
    var painter = PAINTERS[id];
    return call("open_window", { id: id })
      .then(function (spec) {
        if (state.open.indexOf(id) === -1) {
          state.open.push(id);
        }
        state.front = id;
        syncBackdrop();
        showSurface(spec);
        if (painter) {
          state.pending += 1;
          return painter(shellOf(id)).catch(failed).then(function () {
            state.pending -= 1;
          });
        }
        return null;
      })
      .catch(failed);
  }

  /* The dimmed backdrop exists only while something is open, so it can never
     cover the window when it has nothing to dim. */
  function syncBackdrop() {
    var backdrop = el("backdrop");
    if (backdrop) {
      backdrop.hidden = !state.open.length;
    }
  }

  function showSurface(spec) {
    var shell = shellOf(spec.id);
    if (!shell) {
      return;
    }
    shell.hidden = false;
    // The surface that was opened last is the one in front.
    el("surfaces").appendChild(shell);
    var focus = el(spec.initialFocus) || shell;
    if (focus) {
      focus.focus();
    }
  }

  function closeSurface(id) {
    return call("close_window", { id: id })
      .then(function (view) {
        hideSurface(id);
        state.open = (view.open || []).map(function (spec) {
          return spec.id;
        });
        state.front = view.front || "";
        state.restoreFocus = view.restoreFocus || "";
        syncBackdrop();
        if (id === "alert") {
          stopAlertCountdown();
          state.confirm = null;
        }
        /* The window decides where the caret goes when a surface closes: back
           to the surface that was in front before, or to the toolbar control
           that opens the queue. */
        var target = el(state.restoreFocus);
        if (target) {
          target.focus();
        }
        return view;
      })
      .catch(failed);
  }

  function hideSurface(id) {
    var shell = shellOf(id);
    if (shell) {
      shell.hidden = true;
      clear(bodyOf(shell));
    }
  }

  /* Escape closes the surface in front, and only if the window says that
     surface may be closed that way; Tab stays inside it while it is up. */
  function wireSurfaceKeys() {
    document.addEventListener("keydown", function (event) {
      var id = state.front;
      var spec = id ? state.specs[id] : null;
      if (!spec) {
        return;
      }
      if (event.key === "Escape" && spec.escapeCloses) {
        event.preventDefault();
        if (id === "alert") {
          call("dismiss_alert").then(function () {
            closeSurface("alert");
          });
          return;
        }
        closeSurface(id);
        return;
      }
      if (event.key === "Tab") {
        keepFocusInside(id, event);
      }
    });
  }

  /* Focus belongs to the surface in front while it is up: tabbing past its
     last control comes back to its first rather than into the window behind. */
  function keepFocusInside(id, event) {
    var shell = shellOf(id);
    if (!shell) {
      return;
    }
    var focusable = shell.querySelectorAll(
      'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])'
    );
    if (!focusable.length) {
      event.preventDefault();
      shell.focus();
      return;
    }
    var first = focusable[0];
    var last = focusable[focusable.length - 1];
    var here = document.activeElement;
    if (event.shiftKey && (here === first || !shell.contains(here))) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && here === last) {
      event.preventDefault();
      first.focus();
    }
  }

  /* Repaint whichever surfaces are open. The queue and the problem list are
     cheap to read and keep their toolbar counts honest while they are closed;
     the activity window is only read while somebody is looking at it, because
     every read costs a round trip to the worker. */
  function pollWindows() {
    if (state.pending || !state.catalog.length) {
      return;
    }
    var painter = PAINTERS.queue;
    if (painter && (isOpen("queue") || !state.open.length)) {
      painter(shellOf("queue")).catch(function () {
        return null;
      });
    }
    if (isOpen("activity")) {
      PAINTERS.activity(shellOf("activity")).catch(function () {
        return null;
      });
    }
    if (isOpen("error")) {
      PAINTERS.error(shellOf("error")).catch(function () {
        return null;
      });
    } else {
      call("error_view")
        .then(function (view) {
          state.errors = view;
          setBadge(view.total);
        })
        .catch(function () {
          return null;
        });
    }
  }

  function failed(error) {
    notice("error", String(error && error.message ? error.message : error));
    return null;
  }

  /* ------------------------------------------------------------------ queue */

  /* The queue, read again from the window. */
  function refreshQueue() {
    return call("queue_window")
      .then(function (view) {
        state.queue = view;
        var shell = shellOf("queue");
        if (shell && !shell.hidden) {
          paintQueue(shell, view);
        }
        return view;
      })
      .catch(function () {
        return null;
      });
  }

  /* Each queue action is one command. Nothing here decides whether it is on
     offer: the row already said, with the reason. */
  var QUEUE_ACTIONS = {
    run: function (item) {
      return call("run_job", { id: item.id });
    },
    pause: function (item) {
      return call("pause_job", { id: item.id });
    },
    resume: function (item) {
      return call("resume_job", { id: item.id });
    },
    cancel: function (item) {
      return call("cancel_job_confirmed", { id: item.id });
    },
    remove: function (item) {
      return call("remove_job", { id: item.id });
    },
    move_up: function (item) {
      return call("move_job", { id: item.id, delta: -1 });
    },
    move_down: function (item) {
      return call("move_job", { id: item.id, delta: 1 });
    },
    priority_high: function (item) {
      return call("set_job_priority", { id: item.id, priority: "high" });
    },
    priority_low: function (item) {
      return call("set_job_priority", { id: item.id, priority: "low" });
    }
  };

  /* The confirmation an action asks for, when it asks for one. */
  var QUEUE_ALERTS = { cancel: "cancel_job", remove: "remove_job" };

  var QUEUE_BULK = {
    start_queue: { command: "start_queue", alert: "start_queue" },
    cancel_all: { command: "cancel_all", alert: "cancel_queue" },
    clear_finished: { command: "clear_finished_jobs", alert: "clear_finished" },
    empty_queue: { command: "empty_queue", alert: "clear_queue" }
  };

  function useQueueAction(id, item) {
    var run = QUEUE_ACTIONS[id];
    if (!run) {
      return Promise.resolve(null);
    }
    return run(item).then(refreshQueue);
  }

  function paintQueue(shell, view) {
    var host = bodyOf(shell);
    clear(host);

    var head = node("div", "surface-summary");
    head.appendChild(node("span", "title", view.summary));
    head.appendChild(node("span", "hint", view.progressSummary));
    if (view.priorities && view.priorities.length) {
      head.appendChild(
        node(
          "span",
          "hint",
          view.priorities
            .map(function (priority) {
              return priority.label + " " + priority.count;
            })
            .join(", ")
        )
      );
    }
    host.appendChild(head);

    var bulk = node("div", "controls");
    (view.bulk || []).forEach(function (action) {
      bulk.appendChild(
        actionButton(action, function () {
          var route = QUEUE_BULK[action.id];
          if (!route) {
            return;
          }
          askForAlert(route.alert, action.label + "?", {}, function () {
            return call(route.command, {});
          }).then(refreshQueue);
        })
      );
    });
    host.appendChild(bulk);

    if (view.empty) {
      host.appendChild(emptyNote("Nothing is queued."));
      return;
    }

    var table = node("table", "queue-table");
    var headRow = node("tr");
    ["Order", "Job", "State", "Progress", "Message", "Controls"].forEach(function (label) {
      var cell = node("th", null, label);
      cell.setAttribute("scope", "col");
      headRow.appendChild(cell);
    });
    var headGroup = node("thead");
    headGroup.appendChild(headRow);
    table.appendChild(headGroup);

    var body = node("tbody");
    view.items.forEach(function (item) {
      body.appendChild(queueRow(item));
    });
    table.appendChild(body);
    host.appendChild(table);
  }

  function queueRow(item) {
    var row = node("tr");
    row.dataset.tone = item.tone;
    row.dataset.state = item.state;

    var order = node("td", "order", item.runPosition ? String(item.runPosition) : "");
    row.appendChild(order);

    var job = node("td", "name");
    job.appendChild(node("span", "name", item.inputLabel));
    job.appendChild(node("span", "location", item.outputLabel));
    if (item.nextInLine) {
      job.appendChild(node("span", "chip", "Next"));
    }
    row.appendChild(job);

    var stateCell = node("td", "state");
    var label = node("span", null, item.stateLabel);
    label.dataset.tone = item.tone;
    stateCell.appendChild(label);
    stateCell.appendChild(node("span", "hint", item.priorityLabel + " priority"));
    row.appendChild(stateCell);

    var progress = node("td", "progress-cell");
    if (item.progressVisible) {
      var bar = node("div", "progress");
      var barFill = node("div");
      barFill.style.width = item.percent + "%";
      bar.appendChild(barFill);
      bar.setAttribute("role", "progressbar");
      bar.setAttribute("aria-valuemin", "0");
      bar.setAttribute("aria-valuemax", "100");
      bar.setAttribute("aria-valuenow", String(item.percent));
      progress.appendChild(bar);
    }
    progress.appendChild(
      node("span", "hint", item.progressVisible ? item.percent + "%" : item.etaLabel)
    );
    row.appendChild(progress);

    row.appendChild(node("td", "message", item.message));

    var controls = node("td", "controls");
    item.actions.forEach(function (action) {
      controls.appendChild(
        actionButton(action, function () {
          var alert = QUEUE_ALERTS[action.id];
          if (alert) {
            askForAlert(
              alert,
              action.label + " " + item.inputLabel + "?",
              { id: item.id },
              function () {
                return call(QUEUE_ACTIONS[action.id](item));
              }
            ).then(refreshQueue);
            return;
          }
          useQueueAction(action.id, item);
        })
      );
    });
    row.appendChild(controls);
    return row;
  }

  /* -------------------------------------------------------- queue selection */

  function paintSelection(shell, view) {
    var host = bodyOf(shell);
    clear(host);
    state.queueSelection = view;

    var directory = node("input", null, null);
    directory.type = "text";
    directory.id = "selectionDirectory";
    directory.spellcheck = false;
    directory.value = view.outputDirectory;
    var directoryField = labelledField(
      "Folder to write into",
      directory,
      view.directoryValid ? "Every chosen title is written here." : view.directoryProblem,
      view.directoryValid ? "" : "blocked"
    );
    directory.onchange = function () {
      sendSelection({ outputDirectory: directory.value });
    };
    host.appendChild(directoryField);

    var pattern = node("input", null, null);
    pattern.type = "text";
    pattern.id = "selectionPattern";
    pattern.spellcheck = false;
    pattern.value = view.namePattern;
    var patternField = labelledField(
      "Name pattern",
      pattern,
      view.patternValid ? "{title} is the title's name and {n} its number." : view.patternProblem,
      view.patternValid ? "" : "blocked"
    );
    pattern.onchange = function () {
      sendSelection({ namePattern: pattern.value });
    };
    host.appendChild(patternField);

    var list = node("ul", "list selection-list");
    (view.titles || []).forEach(function (title) {
      list.appendChild(selectionRow(title));
    });
    if (!(view.titles || []).length) {
      list.appendChild(emptyNote("Scan a source to see the titles it contains."));
    }
    host.appendChild(list);

    if ((view.planned || []).length) {
      var planned = node("div", "planned");
      planned.appendChild(node("h3", null, "Files that will be written"));
      var plannedList = node("ul", "list");
      view.planned.forEach(function (path) {
        plannedList.appendChild(node("li", "path", path));
      });
      planned.appendChild(plannedList);
      host.appendChild(planned);
    }

    if ((view.problems || []).length) {
      var problems = para(view.problems.join(" "), "notice");
      problems.dataset.tone = "error";
      problems.setAttribute("role", "status");
      host.appendChild(problems);
    }

    var actions = node("div", "controls");
    var queue = node("button", "primary", "Queue the chosen titles");
    queue.type = "button";
    queue.id = "selectionQueueBtn";
    queue.disabled = !view.canQueue;
    if (!view.canQueue) {
      queue.setAttribute("title", view.problems.join(" "));
    }
    queue.onclick = function () {
      call("queue_titles")
        .then(function (ids) {
          closeSurface("queue-selection");
          refreshQueue();
          return ids;
        })
        .catch(failed);
    };
    actions.appendChild(queue);
    actions.appendChild(para(view.summary));
    host.appendChild(actions);
  }

  function selectionRow(title) {
    var item = node("li", "selection-item");
    item.dataset.collides = title.outputCollides ? "true" : "false";

    var pick = node("input");
    pick.type = "checkbox";
    pick.id = "selectionTitle" + title.index;
    pick.checked = title.chosen;
    pick.setAttribute("aria-label", title.label);
    pick.onchange = function () {
      sendSelection({ titles: chosenTitles() });
    };
    item.appendChild(pick);

    var body = node("div", "selection-body");
    var head = node("div", "selection-head");
    head.appendChild(node("span", "title", title.label));
    head.appendChild(node("span", "facts", title.facts));
    body.appendChild(head);

    var name = node("input", null, null);
    name.type = "text";
    name.id = "selectionName" + title.index;
    name.spellcheck = false;
    name.disabled = title.outputCollides || !title.outputValid;
    fill(name, title.outputName);
    var nameHint = title.outputCollides
      ? "Another chosen title would be written to the same file."
      : title.outputValid
      ? title.outputPath
      : title.outputProblem;
    body.appendChild(
      labelledField(
        "File name",
        name,
        nameHint,
        title.outputValid && !title.outputCollides ? "" : "error"
      )
    );
    name.onchange = function () {
      var names = {};
      names[String(title.index)] = name.value;
      sendSelection({ names: names });
    };
    item.appendChild(body);

    // The job model has no start and end field, so the sheet says so here
    // instead of offering a range control that could not be honoured.
    var range = node("span", "chip", title.rangeLabel);
    if (!title.rangeAvailable) {
      range.setAttribute("title", title.rangeReason);
      range.dataset.tone = "blocked";
    }
    body.appendChild(range);
    return item;
  }

  function chosenTitles() {
    return (state.queueSelection.titles || [])
      .filter(function (title) {
        return title.chosen;
      })
      .map(function (title) {
        return title.index;
      });
  }

  /* Send one change to the sheet with everything it has to keep. */
  function sendSelection(change) {
    var view = state.queueSelection || {};
    var input = {
      outputDirectory: view.outputDirectory || "",
      namePattern: view.namePattern || "",
      titles: (view.titles || [])
        .filter(function (title) {
          return title.chosen;
        })
        .map(function (title) {
          return title.index;
        }),
      names: {}
    };
    Object.keys(change).forEach(function (key) {
      input[key] = change[key];
    });
    return call("set_queue_selection", { change: input }).then(function (answer) {
      state.queueSelection = answer;
      var shell = shellOf("queue-selection");
      if (shell && !shell.hidden) {
        paintSelection(shell, answer);
      }
      return answer;
    });
  }

  /* ---------------------------------------------------------------- activity */

  function paintActivity(shell, view) {
    var host = bodyOf(shell);
    clear(host);

    var controls = node("div", "controls");

    var level = node("select");
    level.id = "activityLevel";
    (view.levelOptions || []).forEach(function (option) {
      var entry = document.createElement("option");
      entry.value = option.id;
      entry.textContent = option.label + " (" + option.count + ")";
      entry.selected = option.selected;
      level.appendChild(entry);
    });
    level.onchange = function () {
      call("set_log_level", { level: level.value }).then(function (answer) {
        state.activity = answer;
        paintActivity(shell, answer);
      });
    };
    var levelField = labelledField("Show", level, view.summary);
    controls.appendChild(levelField);

    var follow = node("input");
    follow.type = "checkbox";
    follow.id = "activityFollow";
    follow.checked = view.follow;
    follow.disabled = !view.followAvailable;
    var followLabel = node("label", "switch");
    followLabel.htmlFor = follow.id;
    var followText = node("span", "text");
    followText.appendChild(node("span", null, "Follow the newest line"));
    followText.appendChild(
      node("span", "facts", view.followAvailable ? "Keeps the last line in view." : view.followReason)
    );
    followLabel.appendChild(follow);
    followLabel.appendChild(followText);
    controls.appendChild(followLabel);
    follow.onchange = function () {
      call("set_follow_logs", { follow: follow.checked }).then(function (answer) {
        state.activity = answer;
        paintActivity(shell, answer);
      });
    };

    var copy = actionButton(view.copy, function () {
      copyText(view.copyText, "The activity log is on the clipboard.");
    });
    copy.id = view.copy.id;
    controls.appendChild(copy);

    var clearButton = actionButton(view.clear, function () {
      call("clear_logs").then(function (answer) {
        state.activity = answer;
        paintActivity(shell, answer);
      });
    });
    clearButton.id = view.clear.id;
    controls.appendChild(clearButton);
    host.appendChild(controls);

    if (view.truncated) {
      var dropped = para(
        "The worker dropped older lines before these, so this is not the whole history.",
        "notice"
      );
      dropped.dataset.tone = "warn";
      host.appendChild(dropped);
    }
    if ((view.repeated || []).length) {
      host.appendChild(
        para(
          view.repeated
            .slice(0, 5)
            .map(function (line) {
              return line.message + " × " + line.count;
            })
            .join(" · ")
        )
      );
    }

    var list = node("div", "logs");
    list.id = "activityLines";
    list.setAttribute("role", "log");
    if (view.empty) {
      list.appendChild(emptyNote("Nothing has been recorded yet."));
    }
    (view.lines || []).forEach(function (line) {
      var entry = node("div", "log-line");
      entry.dataset.tone = line.tone;
      entry.dataset.source = line.source;
      entry.appendChild(node("span", "log-level", line.level));
      entry.appendChild(node("span", "log-source", line.source));
      entry.appendChild(node("span", "log-message", line.message));
      list.appendChild(entry);
    });
    host.appendChild(list);
    if (view.follow) {
      list.scrollTop = list.scrollHeight;
    }
  }

  /* ----------------------------------------------------------------- preview */

  function paintPreview(shell, view) {
    var host = bodyOf(shell);
    clear(host);
    state.preview = view;

    host.appendChild(para(view.summary, "surface-summary"));
    var where = node("div", "controls");
    where.appendChild(node("span", "hint", view.sourceLabel));
    where.appendChild(node("span", "hint", view.titleLabel));
    host.appendChild(where);

    var controls = node("div", "controls");

    var stills = node("input");
    stills.type = "number";
    stills.id = view.stillCountControl.id;
    stills.min = "0";
    stills.value = String(view.stillCount);
    stills.onchange = function () {
      call("set_preview_still_count", { count: Number(stills.value) }).then(function (answer) {
        state.preview = answer;
        paintPreview(shell, answer);
      });
    };
    controls.appendChild(
      labelledField(
        view.stillCountControl.label,
        stills,
        view.stillCountControl.available
          ? "The next scan extracts this many frames per title."
          : view.stillCountControl.reason,
        view.stillCountControl.available ? "" : "blocked"
      )
    );

    var previous = actionButton(view.previous, function () {
      stepStill(-1, shell);
    });
    previous.id = view.previous.id;
    controls.appendChild(previous);

    var next = actionButton(view.next, function () {
      stepStill(1, shell);
    });
    next.id = view.next.id;
    controls.appendChild(next);

    controls.appendChild(
      actionButton(view.rescan, function () {
        rescan(shell);
      })
    );
    host.appendChild(controls);

    var scrub = node("input", "scrub");
    scrub.type = "range";
    scrub.id = "previewScrub";
    scrub.min = "0";
    scrub.max = String(Math.max(view.stillCount - 1, 0));
    scrub.value = String(view.stillNumber);
    scrub.step = "1";
    scrub.disabled = !view.scrubAvailable;
    scrub.setAttribute("aria-label", "Still frame");
    if (!view.scrubAvailable) {
      scrub.setAttribute("title", view.scrubReason);
    }
    scrub.onchange = function () {
      call("select_preview_still", { number: Number(scrub.value) }).then(function (answer) {
        return fetchStill().then(function () {
          return answer;
        });
      });
    };
    host.appendChild(scrub);

    var stops = node("ul", "list stops");
    (view.stills || []).forEach(function (still) {
      var item = node("li", "stop");
      var button = node("button", still.current ? "current" : "quiet", still.label);
      button.type = "button";
      button.disabled = !view.scrubAvailable;
      button.addEventListener("click", function () {
        call("select_preview_still", { number: still.number }).then(function () {
          return fetchStill();
        });
      });
      item.appendChild(button);
      stops.appendChild(item);
    });
    host.appendChild(stops);

    var stage = node("div", "preview-stage");
    if (view.image) {
      var frame = node("img", "still");
      frame.id = "previewStill";
      frame.alt = view.titleLabel ? "Still from " + view.titleLabel : "Still frame";
      frame.src = "data:" + view.image.mediaType + ";base64," + view.image.dataBase64;
      stage.appendChild(frame);
      /* The rectangle the crop is taken from.
       *
       * These four are physical on purpose. They place a rectangle drawn over a
       * picture, and a picture does not mirror when the window does: the left
       * edge of the frame is the left edge of the frame whichever way the layout
       * reads, and the drag below reads these same values back to work out what
       * was drawn. Turning them into logical insets would mirror the rectangle
       * away from the pixels it names.
       */
      var overlay = node("div", "crop-overlay");
      overlay.id = "previewCrop";
      overlay.style.left = view.overlay.left * 100 + "%";
      overlay.style.top = view.overlay.top * 100 + "%";
      overlay.style.width = view.overlay.width * 100 + "%";
      overlay.style.height = view.overlay.height * 100 + "%";
      overlay.dataset.valid = view.crop.valid ? "true" : "false";
      stage.appendChild(overlay);
      wireCropDrag(stage, overlay);
    } else {
      stage.appendChild(emptyNote(view.emptyLabel));
    }
    host.appendChild(stage);

    var crop = node("div", "controls");
    var cropNote = para(
      view.crop.summary,
      view.crop.valid ? "" : "notice"
    );
    if (!view.crop.valid) {
      cropNote.dataset.tone = "error";
    }
    crop.appendChild(cropNote);
    var apply = actionButton(view.applyCrop, function () {
      call("apply_preview_crop").then(function (tabs) {
        if (tabs) {
          renderTabs(tabs);
        }
        return call("preview_window").then(function (answer) {
          state.preview = answer;
          paintPreview(shell, answer);
        });
      });
    });
    apply.id = view.applyCrop.id;
    crop.appendChild(apply);

    var reset = node("button", "quiet", "Use the whole frame");
    reset.type = "button";
    reset.id = "previewResetCrop";
    reset.addEventListener("click", function () {
      call("reset_preview_crop").then(function (answer) {
        state.preview = answer;
        paintPreview(shell, answer);
      });
    });
    crop.appendChild(reset);
    host.appendChild(crop);
  }

  function stepStill(delta, shell) {
    call("select_preview_still", { delta: delta }).then(function () {
      return fetchStill();
    });
  }

  function fetchStill() {
    return call("fetch_preview_still").then(function (answer) {
      state.preview = answer;
      var shell = shellOf("preview");
      if (shell && !shell.hidden) {
        paintPreview(shell, answer);
      }
      return answer;
    });
  }

  /* A drag across the frame draws a rectangle.
   *
   * The rectangle is reported as fractions of the frame, which is what the
   * window converts into source pixels; the page never counts pixels itself and
   * never decides whether the rectangle is usable. */
  function wireCropDrag(stage, overlay) {
    var start = null;
    stage.onpointerdown = function (event) {
      if (event.button !== 0) {
        return;
      }
      var bounds = stage.getBoundingClientRect();
      if (!bounds.width || !bounds.height) {
        return;
      }
      start = {
        x: (event.clientX - bounds.left) / bounds.width,
        y: (event.clientY - bounds.top) / bounds.height
      };
      stage.setPointerCapture(event.pointerId);
      event.preventDefault();
    };
    stage.onpointermove = function (event) {
      if (!start) {
        return;
      }
      var bounds = stage.getBoundingClientRect();
      var now = {
        x: (event.clientX - bounds.left) / bounds.width,
        y: (event.clientY - bounds.top) / bounds.height
      };
      overlay.style.left = Math.min(start.x, now.x) * 100 + "%";
      overlay.style.top = Math.min(start.y, now.y) * 100 + "%";
      overlay.style.width = Math.abs(now.x - start.x) * 100 + "%";
      overlay.style.height = Math.abs(now.y - start.y) * 100 + "%";
    };
    stage.onpointerup = function () {
      if (!start) {
        return;
      }
      var left = parseFloat(overlay.style.left) / 100;
      var top = parseFloat(overlay.style.top) / 100;
      var width = parseFloat(overlay.style.width) / 100;
      var height = parseFloat(overlay.style.height) / 100;
      start = null;
      if (width < 0.005 || height < 0.005) {
        return;
      }
      call("set_preview_crop", { crop: { left: left, top: top, width: width, height: height } })
        .then(function (answer) {
          state.preview = answer;
          var shell = shellOf("preview");
          if (shell && !shell.hidden) {
            paintPreview(shell, answer);
          }
        })
        .catch(failed);
    };
  }

  /* Scanning again is how stills are extracted, and the window knows how many
     to ask for; the page sends no count of its own. */
  function rescan(shell) {
    call("begin_scan", { options: {} }).then(function (status) {
      var selection = state.selection || {};
      selection.phase = status.phase;
      selection.phaseName = status.phaseName;
      selection.problem = status.problem;
      renderScan(selection);
      if (status.running) {
        startPolling();
      }
      return status;
    });
  }

  /* ---------------------------------------------------------------- problems */

  function paintErrors(shell, view) {
    var host = bodyOf(shell);
    clear(host);
    state.errors = view;
    setBadge(view.total);

    var controls = node("div", "controls");
    var copy = actionButton(
      { id: "errorCopy", label: "Copy the report", available: view.hasEntries, reason: "" },
      function () {
        copyText(view.copyText, "The problem report is on the clipboard.");
      }
    );
    controls.appendChild(copy);
    var clearButton = node("button", "quiet", "Forget every problem");
    clearButton.type = "button";
    clearButton.id = "errorClear";
    clearButton.disabled = !view.canClear;
    if (!view.canClear) {
      clearButton.setAttribute("title", "There is nothing to forget.");
    }
    clearButton.onclick = function () {
      call("clear_errors").then(function (answer) {
        state.errors = answer;
        setBadge(answer.total);
        paintErrors(shell, answer);
      });
    };
    controls.appendChild(clearButton);
    host.appendChild(controls);

    if (!view.hasEntries) {
      host.appendChild(emptyNote("Nothing has gone wrong."));
      return;
    }
    var list = node("ul", "list error-list");
    view.entries.forEach(function (entry) {
      var item = node("li", "error-item");
      item.dataset.tone = entry.tone;
      item.appendChild(node("span", "severity", entry.severityLabel));
      item.appendChild(node("span", "context", entry.context));
      item.appendChild(node("span", "detail", entry.detail));
      item.appendChild(node("span", "hint", entry.source));
      list.appendChild(item);
    });
    host.appendChild(list);
  }

  /* ------------------------------------------------------------------ about */

  function paintAbout(shell, view) {
    var host = bodyOf(shell);
    clear(host);
    state.about = view;

    host.appendChild(node("h3", null, view.heading));

    var facts = node("dl", "facts-list");
    [
      ["Version", view.version],
      ["Build", view.profile],
      ["Platform", view.platform + " (" + view.architecture + ")"],
      ["Engine", view.engine]
    ].forEach(function (pair) {
      facts.appendChild(node("dt", null, pair[0]));
      facts.appendChild(node("dd", null, pair[1] || "not reported"));
    });
    host.appendChild(facts);
    host.appendChild(para(view.license));

    var links = node("div", "links");
    (view.links || []).forEach(function (link, index) {
      var row = node("div", "link-row");
      row.appendChild(node("span", "title", link.label));
      row.appendChild(node("span", "hint", link.url));
      var copy = node("button", "quiet", "Copy the address");
      copy.type = "button";
      copy.id = "aboutCopyLink" + index;
      /* The shell has no browser to hand a link to, so the address is copied
         rather than followed: a link that opens nothing would be a dead
         control. */
      copy.addEventListener("click", function () {
        copyText(link.url, "The address is on the clipboard.");
      });
      row.appendChild(copy);
      links.appendChild(row);
    });
    host.appendChild(links);

    var copyAll = node("button", "quiet", "Copy these details");
    copyAll.type = "button";
    copyAll.id = "aboutCopy";
    copyAll.addEventListener("click", function () {
      copyText(view.copyText, "The build details are on the clipboard.");
    });
    host.appendChild(copyAll);
  }

  /* ---------------------------------------------------------------- presets */

  /* What each preset control does. The ones that prepare the form rather than
     act on it are the ones that need something typed first. */
  var PRESET_ACTIONS = {
    apply: function (row, form) {
      return sendPreset({ action: "apply", id: row.id });
    },
    clone: function (row, form) {
      return prepareForm({ action: "clone", id: row.id }, "presetName");
    },
    edit: function (row, form) {
      return prepareForm({ action: "edit", id: row.id }, "presetName");
    },
    "set_default": function (row, form) {
      return sendPreset({ action: "set_default", id: row.id });
    },
    clear_default: function (row, form) {
      return sendPreset({ action: "clear_default", id: row.id });
    },
    export: function (row, form) {
      return prepareForm({ action: "export", id: row.id }, "presetPath");
    },
    delete: function (row, form) {
      return askForAlert(
        "delete_preset",
        "",
        { presetId: row.id },
        function () {
          return sendPreset({ action: "delete", id: row.id });
        }
      );
    }
  };

  var PRESET_LIBRARY = {
    create: function () {
      return prepareForm({ action: "create" }, "presetName");
    },
    import: function () {
      return prepareForm({ action: "import" }, "presetPath");
    },
    export: function () {
      return prepareForm({ action: "export" }, "presetPath");
    }
  };

  function paintPresets(shell, view) {
    var host = bodyOf(shell);
    clear(host);
    state.presets = view;

    host.appendChild(para(view.summary + " Stored in " + view.storePath + "."));

    host.appendChild(catalogBrowser(view.catalog));

    var library = node("div", "controls");
    (view.controls || []).forEach(function (control) {
      library.appendChild(
        actionButton(control, function () {
          var run = PRESET_LIBRARY[control.id];
          if (run) {
            run();
          }
        })
      );
    });
    host.appendChild(library);

    host.appendChild(presetForm(view.form));

    if (view.empty) {
      host.appendChild(emptyNote("No presets are available."));
    }

    host.appendChild(presetGroup("Your presets", view.user));
  }

  /* The catalog browser.
   *
   * The six categories are a tab strip and the entries in the chosen category
   * are a listbox, because that is what they are: two levels of choosing one
   * thing out of many. The window has already decided which entry is open, what
   * each one will actually run, and why anything is closed, so the page only
   * wires the keys and the presses to the command that asks again.
   */
  function catalogBrowser(catalog) {
    var box = node("fieldset", "preset-catalog");
    box.appendChild(node("legend", null, "Presets that ship with this build"));
    box.appendChild(para(catalog.summary));

    var wrap = node("div", "catalog");
    var strip = node("div", "tablist catalog-categories");
    strip.setAttribute("role", "tablist");
    strip.setAttribute("aria-label", "Preset categories");
    wrap.appendChild(strip);

    var panels = node("div", "catalog-panels");
    (catalog.categories || []).forEach(function (category, index) {
      strip.appendChild(catalogCategoryTab(category, index, catalog.categories.length));
      panels.appendChild(catalogPanel(category, catalog));
    });
    wrap.appendChild(panels);
    box.appendChild(wrap);

    var note = para(catalog.note, "hint catalog-note");
    note.id = "catalogNote";
    box.appendChild(note);
    box.appendChild(para("Catalog schema version " + catalog.version + ".", "hint"));
    return box;
  }

  /* One category on the strip.
   *
   * One tab stop for the whole strip, arrow keys along it, Home and End for the
   * ends, and the arrows swap with the layout because they are the platform's
   * own. Same contract as the output tabs and the preferences strip.
   */
  function catalogCategoryTab(category, index, total) {
    var button = document.createElement("button");
    button.type = "button";
    button.id = "catalogTab-" + category.id;
    button.setAttribute("role", "tab");
    button.setAttribute("aria-selected", category.selected ? "true" : "false");
    button.setAttribute("aria-controls", "catalogPanel-" + category.id);
    button.title = category.description;
    button.tabIndex = category.selected ? 0 : -1;
    setText(button, category.label + " (" + category.count + ")");
    button.addEventListener("click", function () {
      browseCatalog({ category: category.id });
    });
    button.addEventListener("keydown", function (event) {
      handleCatalogTabKeys(event, index, total);
    });
    return button;
  }

  function catalogPanel(category, catalog) {
    var panel = node("div", "tabpanel catalog-panel");
    panel.id = "catalogPanel-" + category.id;
    panel.setAttribute("role", "tabpanel");
    panel.setAttribute("aria-labelledby", "catalogTab-" + category.id);
    panel.tabIndex = 0;
    panel.hidden = !category.selected;
    panel.appendChild(para(category.description, "hint catalog-category-note"));

    var entries = category.selected ? catalog.entries || [] : [];
    if (!entries.length) {
      panel.appendChild(emptyNote("There are no presets in this category."));
      return panel;
    }

    var list = node("div", "catalog-list");
    list.setAttribute("role", "listbox");
    list.id = "catalogList-" + category.id;
    list.setAttribute("aria-label", category.label + " presets");
    list.setAttribute("aria-describedby", "catalogNote");
    entries.forEach(function (entry, index) {
      list.appendChild(catalogOption(entry, index, entries.length));
    });
    panel.appendChild(list);
    panel.appendChild(catalogDetail(catalog.detail));
    return panel;
  }

  /* One entry in the list.
   *
   * A listbox option rather than a button, because the arrows walk the list
   * rather than tabbing through it. An option this machine cannot run is left in
   * the list with its reason beside it: taking it out of the list would hide the
   * presets a user may well want to read about on a machine that is not this one.
   */
  function catalogOption(entry, index, total) {
    var option = document.createElement("div");
    option.className = "catalog-option";
    option.id = "catalogOption-" + entry.id;
    option.setAttribute("role", "option");
    option.setAttribute("aria-selected", entry.selected ? "true" : "false");
    option.tabIndex = entry.selected ? 0 : -1;
    if (!entry.available) {
      option.setAttribute("aria-disabled", "true");
      option.title = entry.availableReason;
    }

    var head = node("div", "preset-head");
    head.appendChild(node("span", "title", entry.name));
    head.appendChild(node("span", "chip", entry.encoderLabel));
    if (entry.needsHardware) {
      head.appendChild(node("span", "chip", entry.family || "Hardware"));
    }
    if (entry.fallback) {
      head.appendChild(node("span", "chip fallback", "Software fallback"));
    }
    if (!entry.available) {
      head.appendChild(node("span", "chip closed", "Not available here"));
    }
    if (entry.isDefault) {
      head.appendChild(node("span", "chip default", "Default"));
    }
    option.appendChild(head);
    option.appendChild(para(entry.summary));

    if (!entry.available && entry.availableReason) {
      option.appendChild(para(entry.availableReason, "hint catalog-closed"));
    }
    option.addEventListener("click", function () {
      if (entry.id !== (state.presets.catalog.detail || {}).id) {
        browseCatalog({ category: entry.categoryId, entry: entry.id });
      }
    });
    option.addEventListener("keydown", function (event) {
      handleCatalogOptionKeys(event, entry.id, index, total);
    });
    return option;
  }

  /* The open entry, read out in full.
   *
   * Everything here is text the window wrote: what the preset is for, which
   * players it is meant for, which encoder will run and whether that is the one
   * it was written for. The controls are the window's actions, so a preset that
   * cannot run here arrives with its apply closed and its reason in place.
   */
  function catalogDetail(detail) {
    if (!detail) {
      return emptyNote("Choose a preset to read what it does.");
    }
    var box = node("div", "catalog-detail");

    var head = node("div", "preset-head");
    head.appendChild(node("span", "title", detail.name));
    head.appendChild(node("span", "chip", detail.encoder));
    if (detail.hardware) {
      head.appendChild(node("span", "chip", "On the chip"));
    }
    if (detail.fallback) {
      head.appendChild(node("span", "chip fallback", "Software fallback"));
    }
    box.appendChild(head);
    box.appendChild(para(detail.description));
    if (detail.compatibility) {
      var guidance = node("dl", "facts-list");
      guidance.appendChild(node("dt", null, "Plays on"));
      guidance.appendChild(node("dd", null, detail.compatibility));
      box.appendChild(guidance);
    }
    var facts = node("dl", "facts-list");
    facts.appendChild(node("dt", null, "Sets"));
    facts.appendChild(node("dd", null, detail.summary));
    facts.appendChild(node("dt", null, "Encoder"));
    facts.appendChild(node("dd", null, detail.encoderLabel + " (" + detail.encoder + ")"));
    box.appendChild(facts);

    if (detail.availableNote) {
      box.appendChild(para(detail.availableNote, "hint catalog-note"));
    }
    if (!detail.available && detail.availableReason) {
      var closed = para(detail.availableReason, "notice catalog-closed-note");
      closed.setAttribute("data-tone", "error");
      box.appendChild(closed);
    }

    var controls = node("div", "controls");
    (detail.actions || []).forEach(function (action) {
      controls.appendChild(
        actionButton(action, function () {
          var run = PRESET_ACTIONS[action.id];
          if (run) {
            run({ id: detail.id }, state.presetForm);
          }
        })
      );
    });
    box.appendChild(controls);
    return box;
  }

  function browseCatalog(change) {
    return call("browse_catalog", { change: change })
      .then(function (view) {
        state.presets = view;
        var shell = shellOf("presets");
        if (shell && !shell.hidden) {
          paintPresets(shell, view);
          /* The list is rebuilt from the answer, so the option that is open now
             is the one that exists now: focus follows the selection rather than
             the element that was pressed. */
          var open = view.catalog && view.catalog.detail;
          if (open) {
            var option = el("catalogOption-" + open.id);
            if (option) {
              option.focus();
            }
          }
        }
        return view;
      })
      .catch(failed);
  }

  /* Home and End do not move with the layout: they go to the first and the last
     either way, which is where they sit on screen. */
  function handleCatalogTabKeys(event, index, total) {
    var moved = stepBy(event, index, total);
    if (moved === null) {
      return;
    }
    var categories = (state.presets && state.presets.catalog.categories) || [];
    var target = categories[moved];
    if (!target) {
      return;
    }
    event.preventDefault();
    browseCatalog({ category: target.id }).then(function () {
      var button = el("catalogTab-" + target.id);
      if (button) {
        button.focus();
      }
    });
  }

  /* The list walks up and down, and the arrows follow the reading direction:
     the arrow that means "further along" is the one that points that way. */
  function handleCatalogOptionKeys(event, id, index, total) {
    var arrows = tabArrows();
    var next = null;
    if (event.key === "ArrowDown" || event.key === arrows.forwards) {
      next = index === total - 1 ? null : index + 1;
    } else if (event.key === "ArrowUp" || event.key === arrows.backwards) {
      next = index === 0 ? null : index - 1;
    } else if (event.key === "Home") {
      next = 0;
    } else if (event.key === "End") {
      next = total - 1;
    }
    if (next === null || next === index) {
      return;
    }
    var entries = (state.presets && state.presets.catalog.entries) || [];
    var target = entries[next];
    if (!target) {
      return;
    }
    event.preventDefault();
    browseCatalog({ category: target.categoryId, entry: target.id });
  }

  /* Which way along a strip a key press goes, or null when it is not one of the
     keys a strip answers. Shared by the catalog strip so all three strips agree
     on what wraps and what does not. */
  function stepBy(event, index, total) {
    if (!total) {
      return null;
    }
    var last = total - 1;
    var arrows = tabArrows();
    if (event.key === arrows.forwards) {
      return index === last ? 0 : index + 1;
    }
    if (event.key === arrows.backwards) {
      return index === 0 ? last : index - 1;
    }
    if (event.key === "Home") {
      return 0;
    }
    if (event.key === "End") {
      return last;
    }
    return null;
  }

  function presetGroup(title, presets) {
    var group = node("fieldset", "presets");
    group.appendChild(node("legend", null, title));
    if (!(presets || []).length) {
      group.appendChild(emptyNote("Nothing here yet."));
      return group;
    }
    (presets || []).forEach(function (preset) {
      group.appendChild(presetRow(preset));
    });
    return group;
  }

  function presetRow(preset) {
    var row = node("div", "preset-row");
    row.dataset.builtIn = preset.builtIn ? "true" : "false";

    var head = node("div", "preset-head");
    head.appendChild(node("span", "title", preset.name));
    head.appendChild(node("span", "chip", preset.category));
    if (preset.fallback) {
      head.appendChild(node("span", "chip fallback", "Software fallback"));
    }
    if (!preset.available) {
      head.appendChild(node("span", "chip closed", "Not available here"));
    }
    if (preset.isDefault) {
      head.appendChild(node("span", "chip default", "Default"));
    }
    row.appendChild(head);
    row.appendChild(para(preset.summary));
    if (preset.description) {
      row.appendChild(para(preset.description));
    }
    if (!preset.editable && preset.editReason) {
      row.appendChild(para(preset.editReason));
    }
    /* A stored preset is resolved against this machine as well as one from the
       catalog: a preset copied off another machine may name an encoder this one
       does not have. */
    if (!preset.available && preset.availableReason) {
      row.appendChild(para(preset.availableReason, "hint catalog-closed"));
    } else if (preset.availableNote) {
      row.appendChild(para(preset.availableNote, "hint catalog-note"));
    }

    var controls = node("div", "controls");
    preset.actions.forEach(function (action) {
      controls.appendChild(
        actionButton(action, function () {
          var run = PRESET_ACTIONS[action.id];
          if (run) {
            run(preset, state.presetForm);
          }
        })
      );
    });
    row.appendChild(controls);
    return row;
  }

  function presetForm(form) {
    var box = node("fieldset", "preset-form");
    box.appendChild(node("legend", null, "Preset"));

    var name = node("input");
    name.id = "presetName";
    name.type = "text";
    name.disabled = !form.nameAvailable;
    fill(name, form.name);
    var nameField = labelledField(
      "Name",
      name,
      form.nameAvailable ? "A preset needs a name so it can be picked from the list." : form.nameReason,
      form.nameAvailable ? "" : "blocked"
    );
    name.onchange = function () {
      state.presetForm.name = name.value;
      refreshPresetForm();
    };
    box.appendChild(nameField);

    var description = node("input");
    description.id = "presetDescription";
    description.type = "text";
    description.disabled = !form.descriptionAvailable;
    fill(description, form.description);
    description.onchange = function () {
      state.presetForm.description = description.value;
      refreshPresetForm();
    };
    box.appendChild(
      labelledField(
        "What it is for",
        description,
        form.descriptionAvailable ? "Shown in the list beside the name." : form.descriptionReason,
        form.descriptionAvailable ? "" : "blocked"
      )
    );

    var category = node("select");
    category.id = "presetCategory";
    (form.categories || []).forEach(function (option) {
      var entry = document.createElement("option");
      entry.value = option.id;
      entry.textContent = option.label;
      entry.selected = option.selected;
      category.appendChild(entry);
    });
    category.onchange = function () {
      state.presetForm.category = category.value;
      refreshPresetForm();
    };
    box.appendChild(labelledField("Category", category, "Where the preset is filed."));

    var useAsDefault = node("input");
    useAsDefault.id = "presetDefault";
    useAsDefault.type = "checkbox";
    useAsDefault.checked = form.default;
    useAsDefault.disabled = !form.defaultAvailable;
    useAsDefault.onchange = function () {
      state.presetForm.default = useAsDefault.checked;
      refreshPresetForm();
    };
    var defaultLabel = node("label", "switch");
    defaultLabel.htmlFor = useAsDefault.id;
    var defaultText = node("span", "text");
    defaultText.appendChild(node("span", null, "Use for every new job"));
    defaultText.appendChild(
      node(
        "span",
        "facts",
        form.defaultAvailable
          ? "One preset can start every new job."
          : form.defaultReason
      )
    );
    defaultLabel.appendChild(useAsDefault);
    defaultLabel.appendChild(defaultText);
    box.appendChild(defaultLabel);

    var path = node("input");
    path.id = "presetPath";
    path.type = "text";
    path.spellcheck = false;
    path.disabled = !form.pathAvailable;
    fill(path, form.path);
    path.onchange = function () {
      state.presetForm.path = path.value;
      refreshPresetForm();
    };
    box.appendChild(
      labelledField(
        "File",
        path,
        form.pathAvailable
          ? "A preset file is the JSON the core writes."
          : form.pathReason,
        form.pathAvailable ? "" : "blocked"
      )
    );

    if ((form.problems || []).length) {
      var problems = para(form.problems.join(" "), "notice");
      problems.dataset.tone = "error";
      problems.setAttribute("role", "status");
      box.appendChild(problems);
    }

    var submit = node("button", "primary", form.submitLabel);
    submit.type = "button";
    submit.id = "presetSubmit";
    submit.disabled = !form.canSubmit;
    if (!form.canSubmit) {
      submit.setAttribute("title", form.problems.join(" "));
    }
    submit.onclick = function () {
      sendPreset(state.presetForm);
    };
    box.appendChild(submit);
    return box;
  }

  /* Point the form at an action and read it back from the window, so the
     wording, the closed fields, and the problems all come from there. */
  function prepareForm(change, focusId) {
    var next = {};
    Object.keys(state.presetForm).forEach(function (key) {
      next[key] = state.presetForm[key];
    });
    Object.keys(change).forEach(function (key) {
      next[key] = change[key];
    });
    state.presetForm = next;
    return refreshPresetForm(focusId);
  }

  /* Read the form back from the window.
   *
   * Whether the form may be submitted is the window's answer, not the page's, so
   * every field change asks again: a control that stays closed while the reason
   * for it has been fixed is a control the user cannot get out of. */
  function refreshPresetForm(focusId) {
    var active = document.activeElement ? document.activeElement.id : "";
    return call("preset_window", { change: state.presetForm })
      .then(function (view) {
        state.presets = view;
        var shell = shellOf("presets");
        if (shell && !shell.hidden) {
          paintPresets(shell, view);
          /* The form is painted again from scratch, so the field the user was
             in is focused again rather than dropped. */
          var focus = el(focusId || active);
          if (focus) {
            focus.focus();
          }
        }
        return view;
      })
      .catch(failed);
  }

  /* Send one preset change with everything the form is holding. */
  function sendPreset(change) {
    var input = {};
    Object.keys(state.presetForm).forEach(function (key) {
      input[key] = state.presetForm[key];
    });
    Object.keys(change || {}).forEach(function (key) {
      input[key] = change[key];
    });
    state.presetForm = input;
    return call("apply_preset_action", { change: input }).then(function (view) {
      state.presets = view;
      var shell = shellOf("presets");
      if (shell && !shell.hidden) {
        paintPresets(shell, view);
      }
      if (tabsNeedRepaint()) {
        return call("tab_set").then(function (tabs) {
          if (tabs) {
            renderTabs(tabs);
          }
          return view;
        });
      }
      return view;
    });
  }

  /* A preset writes the container, codecs, size, and quality, so the tabs that
     show those have to be read again after one is applied. */
  function tabsNeedRepaint() {
    return state.presetForm.action === "apply" || state.presetForm.action === "create";
  }

  /* ------------------------------------------------------------------ alert */

  /* Ask the window whether `action` may go ahead.
   *
   * The countdown itself is counted in Rust, so the number the user reads is
   * the number that arms the control. What runs once it is confirmed is the
   * function the caller passed in, which is the only place an action exists.
   */
  function askForAlert(action, headline, args, run) {
    var request = {
      action: action,
      headline: headline || "",
      jobId: args && args.id ? args.id : null,
      presetId: args && args.presetId ? args.presetId : null
    };
    return call("ask_for_alert", request)
      .then(function (view) {
        state.alert = view;
        state.confirm = run;
        paintAlert(view);
        if (view.armed) {
          /* Nothing destructive waits, so the question is asked and answered in
             one go rather than opened for a countdown nobody has to sit
             through. */
          return confirmAlert();
        }
        if (state.open.indexOf("alert") === -1) {
          state.open.push("alert");
        }
        state.front = "alert";
        syncBackdrop();
        showSurface(state.specs.alert);
        startAlertCountdown();
        return null;
      })
      .catch(function (error) {
        failed(error);
        return null;
      });
  }

  function confirmAlert() {
    return call("confirm_alert")
      .then(function (alert) {
        stopAlertCountdown();
        var run = state.confirm;
        state.confirm = null;
        if (!alert) {
          closeSurface("alert");
          return null;
        }
        closeSurface("alert");
        /* An alert the window opened for itself — the queue finishing and asking
           whether to close — has no page-side runner, so the confirmed action is
           routed by name instead. Only the window names an action it asked for
           itself, and only the command that owns the action can carry it out. */
        var carry = state.confirmRunners[alert.action] || run;
        return carry ? carry() : null;
      })
      .catch(failed);
  }

  function dismissAlert() {
    stopAlertCountdown();
    state.confirm = null;
    return call("dismiss_alert").then(function () {
        closeSurface("alert");
      });
  }

  /* Open the countdown for a question the window asked by itself.
   *
   * The alert is already pending in the window, so it is not asked for again;
   * the page only has to put it on screen and start the count, which is the same
   * two steps the page takes for a question it asked itself.
   */
  function showAskedAlert(view) {
    if (!view) {
      return null;
    }
    state.alert = view;
    state.confirm = null;
    paintAlert(view);
    if (state.open.indexOf("alert") === -1) {
      state.open.push("alert");
    }
    state.front = "alert";
    syncBackdrop();
    showSurface(state.specs.alert);
    if (view.armed) {
      stopAlertCountdown();
    } else {
      startAlertCountdown();
    }
    return view;
  }

  function startAlertCountdown() {
    stopAlertCountdown();
    state.alertTimer = window.setInterval(function () {
      call("tick_alert")
        .then(function (view) {
          state.alert = view;
          paintAlert(view);
          if (view.armed) {
            stopAlertCountdown();
          }
        })
        .catch(function () {
          stopAlertCountdown();
        });
    }, 1000);
  }

  function stopAlertCountdown() {
    if (state.alertTimer) {
      window.clearInterval(state.alertTimer);
      state.alertTimer = null;
    }
  }

  function paintAlert(view) {
    var shell = shellOf("alert");
    if (!shell) {
      return;
    }
    var host = bodyOf(shell);
    clear(host);
    host.appendChild(node("h3", null, view.headline));
    host.appendChild(para(view.detail));
    var live = node("p", "hint", view.liveLabel);
    live.id = "alertCountdown";
    live.setAttribute("role", "status");
    live.setAttribute("aria-live", "polite");
    host.appendChild(live);

    var controls = node("div", "controls");
    var confirm = node("button", view.destructive ? "warn" : "primary", view.verb);
    confirm.type = "button";
    confirm.id = view.confirmId;
    confirm.disabled = !view.armed;
    if (!view.armed) {
      confirm.setAttribute("title", view.liveLabel);
    }
    confirm.addEventListener("click", function () {
      confirmAlert();
    });
    controls.appendChild(confirm);

    var cancel = node("button", "quiet", "Back out");
    cancel.type = "button";
    cancel.id = view.cancelId;
    cancel.addEventListener("click", function () {
      dismissAlert();
    });
    controls.appendChild(cancel);
    host.appendChild(controls);
  }

  /* -------------------------------------------------------------- clipboard */

  /* The webview's clipboard, with a selection-based fallback for the builds
     where it is not available. */
  function copyText(text, done) {
    if (!text) {
      return;
    }
    var copied = function () {
      notice("muted", "Copied to the clipboard.");
      if (done) {
        done();
      }
    };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(text).then(copied, function () {
        legacyCopy(text);
        copied();
      });
      return;
    }
    legacyCopy(text);
    copied();
  }

  function legacyCopy(text) {
    var area = document.createElement("textarea");
    area.value = text;
    area.setAttribute("readonly", "readonly");
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    try {
      document.execCommand("copy");
    } catch (error) {
      notice("error", "That could not be copied: " + error);
    }
    document.body.removeChild(area);
  }

  function queueSelection() {
    // Only the destination travels from the page: the window builds the job from
    // the tab settings it already validated.
    var outputPath = el("outPath").value.trim();
    if (!outputPath) {
      notice("error", "Choose where the finished file should be written first.");
      return Promise.resolve();
    }
    state.pending += 1;
    return call("queue_selection", { outputPath: outputPath })
      .then(refreshQueue)
      .catch(function (error) {
        notice("error", "Could not queue the selection: " + error);
      })
      .then(function () {
        state.pending -= 1;
      });
  }

  function importSubtitles() {
    var path = el("externalSubtitle").value.trim();
    state.pending += 1;
    return call("import_subtitles", { path: path })
      .then(function (tabs) {
        if (tabs) {
          renderTabs(tabs);
        }
      })
      .catch(function (error) {
        notice("error", "That subtitle file could not be imported: " + error);
      })
      .then(function () {
        state.pending -= 1;
      });
  }

  /* ------------------------------------------------------------ drag and drop */

  function acceptPaths(paths) {
    if (!paths || !paths.length) {
      return;
    }
    return call("inspect_sources", { paths: paths })
      .then(function (report) {
        var rejected = (report.rejected || [])
          .map(function (entry) {
            return entry.reason;
          })
          .join(" ");
        if (!report.accepted || !report.accepted.length) {
          notice("error", "Nothing in that drop could be scanned. " + rejected);
          return null;
        }
        notice("muted", rejected);
        return openSource(report.accepted[0].path);
      })
      .catch(function (error) {
        notice("error", "That drop could not be read: " + error);
        return null;
      });
  }

  function wireDropzone() {
    var zone = el("dropzone");
    if (!zone) {
      return;
    }
    zone.addEventListener("dragover", function (event) {
      event.preventDefault();
      zone.dataset.dragging = "true";
    });
    zone.addEventListener("dragleave", function () {
      zone.dataset.dragging = "false";
    });
    zone.addEventListener("drop", function (event) {
      event.preventDefault();
      zone.dataset.dragging = "false";
      // A browser only exposes file names, never full paths; the window
      // supplies real paths through its own drag event below.
      notice("muted", "Drop files inside the application window to read their full paths.");
    });

    if (!bridge || !bridge.webview) {
      setText(
        el("dropHint"),
        "Drag and drop needs the application window; typing a path works everywhere."
      );
      return;
    }
    try {
      bridge.webview.getCurrentWebview().onDragDropEvent(function (event) {
        var payload = event.payload || {};
        if (payload.type === "enter" || payload.type === "over") {
          zone.dataset.dragging = "true";
          return;
        }
        zone.dataset.dragging = "false";
        if (payload.type === "drop") {
          acceptPaths(payload.paths || []);
        }
      });
    } catch (error) {
      setText(el("dropHint"), "Drag and drop is unavailable here; type a path instead.");
    }
  }

/* ------------------------------------------------------------- preferences */

  /* The preferences surface.
   *
   * Nothing here decides anything. The window sends six tabs, each with the
   * controls on it, the value each one holds, whether it is on offer, and the
   * reason when it is not; the page puts those on screen, sends back the one
   * field that was touched, and paints whatever comes back. That is why a tab
   * can show a control closed with a reason the page never invented, and why a
   * change that moves another tab's control — the collision switch settling the
   * collision choice — needs no special case here.
   */
  /* Where each tab's view model lives on the answer.
   *
   * Written down here rather than sent by the window, for the same reason the
   * output tabs' fields are read by name: a property name is not data the window
   * can send, and the window's catalog of six tabs is checked against this list
   * by a test in the window so the two cannot drift apart.
   */
  var PREFERENCE_TAB_FIELDS = [
    "general",
    "output",
    "advanced",
    "hardware",
    "updates",
    "whenDone"
  ];

  /* The whole view model for the tab named `id`, or nothing when the window has
     no such tab. */
  function preferenceTabModel(view, id) {
    for (var i = 0; i < PREFERENCE_TAB_FIELDS.length; i += 1) {
      var tab = view[PREFERENCE_TAB_FIELDS[i]];
      if (tab && tab.id === id) {
        return tab;
      }
    }
    return null;
  }

  function paintPreferences(shell, view) {
    var host = bodyOf(shell);
    clear(host);
    state.preferences = view;

    var wrap = node("div", "preferences");
    var strip = node("div", "tablist");
    strip.setAttribute("role", "tablist");
    strip.setAttribute("aria-label", "Preferences");
    wrap.appendChild(strip);

    var panels = node("div", "tabpanels");
    wrap.appendChild(panels);

    (view.tabs || []).forEach(function (tab, index) {
      strip.appendChild(preferenceTab(tab, view.active, index, view.tabs.length));
      panels.appendChild(preferencePanel(preferenceTabModel(view, tab.id), view.active));
    });
    host.appendChild(wrap);

    var storage = node("p", "hint preferences-storage");
    storage.appendChild(node("strong", null, view.storage.stateLabel + ". "));
    storage.appendChild(document.createTextNode(view.storage.note));
    var where = node("span", "hint preferences-location");
    where.textContent = view.storage.location;
    storage.appendChild(where);
    host.appendChild(storage);

    if ((view.problems || []).length) {
      var problems = node("div", "notice preferences-problems");
      problems.setAttribute("data-tone", "error");
      problems.setAttribute("role", "status");
      problems.setAttribute("aria-live", "polite");
      problems.textContent = view.problems.join(" ");
      host.appendChild(problems);
    }

    host.appendChild(preferenceReset(view.reset));
    var announcement = node("p", "hint preferences-announcement");
    announcement.setAttribute("role", "status");
    announcement.setAttribute("aria-live", "polite");
    announcement.textContent = view.reset.announcement || "";
    host.appendChild(announcement);
  }

  /* One tab on the strip.
   *
   * The same semantics as the output tabs: one tab stop for the whole strip,
   * arrow keys along it, Home and End for the ends, and the arrows swap with
   * the layout because they are the platform's own. The window says which tab is
   * showing and which needs attention; the page only wires the presses.
   */
  function preferenceTab(tab, active, index, total) {
    var button = document.createElement("button");
    button.type = "button";
    button.id = "prefTab-" + tab.id;
    button.setAttribute("role", "tab");
    button.setAttribute("aria-selected", tab.id === active ? "true" : "false");
    button.setAttribute("aria-controls", "prefPanel-" + tab.id);
    button.dataset.attention = tab.needsAttention ? "true" : "false";
    button.title = tab.hint;
    button.tabIndex = tab.id === active ? 0 : -1;
    button.textContent = tab.label;
    button.addEventListener("click", function () {
      activatePreferenceTab(tab.id);
    });
    button.addEventListener("keydown", function (event) {
      handlePreferenceTabKeys(event, index, total);
    });
    return button;
  }

  function preferencePanel(tab, active) {
    if (!tab) {
      /* The window listed a tab it did not send the contents of. An empty panel
         with the name on it is better than a strip that leads nowhere. */
      var missing = node("div", "tabpanel");
      missing.hidden = true;
      return missing;
    }
    var panel = node("div", "tabpanel");
    panel.id = "prefPanel-" + tab.id;
    panel.setAttribute("role", "tabpanel");
    panel.setAttribute("aria-labelledby", "prefTab-" + tab.id);
    panel.tabIndex = 0;
    panel.hidden = tab.id !== active;
    panel.appendChild(para(tab.hint));

    var body = node("div", "preference-body");
    (tab.choices || []).forEach(function (choice) {
      body.appendChild(preferenceChoice(choice));
    });
    (tab.facts || []).forEach(function (fact) {
      body.appendChild(preferenceFact(fact));
    });
    if ((tab.toggles || []).length) {
      var flags = node("fieldset", "flags");
      flags.appendChild(node("legend", null, "Switches"));
      (tab.toggles || []).forEach(function (toggle) {
        flags.appendChild(preferenceToggle(toggle));
      });
      body.appendChild(flags);
    }
    (tab.numbers || []).forEach(function (number) {
      body.appendChild(preferenceNumber(number));
    });
    (tab.texts || []).forEach(function (text) {
      body.appendChild(preferenceText(text));
    });
    if ((tab.actions || []).length) {
      var row = node("div", "controls");
      (tab.actions || []).forEach(function (action) {
        row.appendChild(preferenceAction(action));
      });
      body.appendChild(row);
    }
    panel.appendChild(body);

    if ((tab.problems || []).length) {
      var problems = node("div", "notice");
      problems.setAttribute("data-tone", "error");
      problems.textContent = tab.problems.join(" ");
      panel.appendChild(problems);
    }
    return panel;
  }

  /* A single-choice control.
   *
   * Built as a labelled select so it is announced with its label, and so an
   * option that cannot be picked is left in the list with its reason beside it
   * rather than silently missing.
   */
  function preferenceChoice(choice) {
    var select = document.createElement("select");
    select.id = choice.id;
    (choice.options || []).forEach(function (option) {
      var entry = document.createElement("option");
      entry.value = option.id;
      entry.textContent = option.available
        ? option.label
        : option.label + " — " + option.reason;
      entry.disabled = !option.available;
      entry.selected = option.id === choice.selected;
      select.appendChild(entry);
    });
    var field = labelledField(
      choice.label,
      select,
      choice.available ? choice.hint : choice.reason,
      choice.available ? "" : "blocked"
    );
    select.disabled = !choice.available;
    select.onchange = function () {
      sendPreference(choice.id, select.value);
    };
    return field;
  }

  /* A switch. The label wraps the input, so the control and its label are one
     thing to a screen reader and one thing to click. The line under the label is
     the reason the control is closed when it is closed, and it is named and
     linked rather than merely present: a reason only somebody can see is a
     reason that does not exist for anyone who cannot see it. */
  function preferenceToggle(toggle) {
    var input = document.createElement("input");
    input.type = "checkbox";
    input.id = toggle.id;
    input.checked = Boolean(toggle.checked);
    input.disabled = !toggle.available;

    var facts = node(
      "span",
      "facts",
      toggle.available ? toggle.hint : toggle.reason
    );
    facts.id = toggle.id + "Hint";

    var text = node("span", "text");
    text.appendChild(node("span", null, toggle.label));
    text.appendChild(facts);

    var label = node("label", "switch");
    label.htmlFor = input.id;
    label.appendChild(input);
    label.appendChild(text);
    if (!toggle.available) {
      label.dataset.blocked = "true";
    }
    describe(input, facts);
    input.onchange = function () {
      sendPreference(toggle.id, input.checked);
    };
    return label;
  }

  /* A number the user types into. What counts as a number is the window's
     rule; the page only turns what was typed into a number when it is one, and
     otherwise sends the text so the refusal names the field and says why. */
  function preferenceNumber(number) {
    var input = document.createElement("input");
    input.type = "text";
    input.id = number.id;
    input.inputMode = "numeric";
    input.value = number.text || "";
    input.disabled = !number.available;
    var field = labelledField(
      number.label,
      input,
      number.available ? number.hint : number.reason,
      number.available ? "" : "blocked"
    );
    if (number.available) {
      input.onchange = function () {
        var typed = input.value.trim();
        var parsed = Number(typed);
        sendPreference(number.id, typed !== "" && isFinite(parsed) ? parsed : typed);
      };
    }
    return field;
  }

  /* A line of text. A folder is a path, so the field reads left to right even
     in a mirrored layout; that is the one place on this surface where a
     physical direction is right, and it is the field's own attribute rather
     than a stylesheet override. */
  function preferenceText(text) {
    var input = document.createElement("input");
    input.type = "text";
    input.id = text.id;
    input.spellcheck = false;
    input.autocomplete = "off";
    input.dir = "ltr";
    input.placeholder = text.placeholder || "";
    input.value = text.text || "";
    var hint = text.available ? text.hint : text.reason;
    if (!text.valid && text.problem) {
      hint = text.problem;
    }
    var field = labelledField(
      text.label,
      input,
      hint,
      text.valid ? "" : text.available ? "error" : "blocked"
    );
    input.setAttribute("aria-invalid", text.valid ? "false" : "true");
    input.disabled = !text.available;
    if (text.available) {
      input.onchange = function () {
        sendPreference(text.id, input.value);
      };
    }
    return field;
  }

  /* A fact: something the window observed or worked out, not something to
     change. Marked up as a description list because a row with a name and a
     value is a pair, and the pair is what a screen reader should announce. */
  function preferenceFact(fact) {
    var list = node("dl", "facts-list");
    list.id = fact.id;
    var term = node("dt", null, fact.label);
    var detail = node("dd", null, fact.value);
    if (!fact.available && fact.reason) {
      detail.appendChild(document.createTextNode(" "));
      detail.appendChild(node("span", "hint", fact.reason));
    }
    list.appendChild(term);
    list.appendChild(detail);
    return list;
  }

  /* A button. Which command a press means is decided by the page's table, so a
     button that is not on offer cannot be pressed to find out. */
  function preferenceAction(action) {
    var button = node("button", null, action.label);
    button.type = "button";
    button.id = action.id;
    button.title = action.available ? action.hint : action.reason;
    button.disabled = !action.available;
    if (!action.available) {
      button.setAttribute("aria-describedby", "preferences-problem-" + action.id);
    }
    button.onclick = action.available
      ? function () {
          runPreferenceAction(action);
        }
      : null;
    return button;
  }

  /* Restore the defaults, and take a restore back.
   *
   * Both ask through the countdown alert rather than acting on a click: a reset
   * throws away every choice on every tab, and the take-back is only offered
   * when there is something to take back.
   */
  function preferenceReset(reset) {
    var box = node("fieldset", "rule-set preferences-reset");
    box.appendChild(node("legend", null, "Starting over"));

    var row = node("div", "controls");
    var restore = node("button", reset.confirms ? "warn" : null, reset.label);
    restore.type = "button";
    restore.id = "preferencesReset";
    restore.title = reset.available ? reset.effect : reset.reason;
    restore.disabled = !reset.available;
    restore.onclick = reset.available
      ? function () {
          askForAlert(
            "reset_preferences",
            "Put every preference on every tab back to its default?",
            null,
            function () {
              return call("reset_preferences").then(function (view) {
                repaintPreferences(view);
              });
            }
          );
        }
      : null;
    row.appendChild(restore);

    var undo = node("button", null, reset.undoLabel);
    undo.type = "button";
    undo.id = "preferencesUndo";
    undo.title = reset.undoEffect;
    undo.disabled = !reset.canUndo;
    undo.onclick = reset.canUndo
      ? function () {
          call("undo_preference_reset")
            .then(function (view) {
              repaintPreferences(view);
            })
            .catch(failed);
        }
      : null;
    row.appendChild(undo);
    box.appendChild(row);

    if (!reset.available && reset.reason) {
      var blocked = node("p", "hint");
      blocked.setAttribute("data-tone", "blocked");
      blocked.id = "preferences-problem-preferencesReset";
      blocked.textContent = reset.reason;
      box.appendChild(blocked);
    }
    box.appendChild(para(reset.summary));
    return box;
  }

  /* Send one change and paint the whole surface from the answer.
   *
   * One command returns everything, so a change that reaches several tabs shows
   * up everywhere at once, and a refusal puts every control back where the
   * window says it is rather than leaving the page showing what was asked for.
   */
  function sendPreference(field, value) {
    state.pending += 1;
    return call("set_preference", { change: { field: field, value: value } })
      .then(function (view) {
        repaintPreferences(view);
        return view;
      })
      .catch(function (error) {
        notice(
          "error",
          "That preference was not kept: " +
            String(error && error.message ? error.message : error)
        );
        return refreshPreferences();
      })
      .then(function (view) {
        state.pending -= 1;
        return view;
      });
  }

  /* What a press on one of the surface's buttons means.
   *
   * Written down once. A button whose identifier is missing here would do
   * nothing when pressed, which is why the window lists the identifiers it
   * offers and the page's table and that list are checked against each other by
   * a test in the window.
   */
  var PREFERENCE_PRESSES = {
    preferencesRecheckHardware: function () {
      return call("recheck_hardware")
        .then(repaintPreferences)
        .catch(failed);
    },
    preferencesCheckNow: function () {
      /* This build contacts no release server, so the window closes this control
         and it cannot be pressed. The press is still routed, because a name on
         offer that nothing answers is worse than one that says why: if a build
         ever opens the control, the reason the window gives is shown rather than
         the press doing nothing. */
      return call("preferences_view")
        .then(function (view) {
          repaintPreferences(view);
          var found = (view.updates.actions || []).filter(function (action) {
            return action.id === "preferencesCheckNow";
          })[0];
          notice("error", (found && found.reason) || "There is nothing to check.");
          return view;
        })
        .catch(failed);
    }
  };

  function runPreferenceAction(action) {
    var run = PREFERENCE_PRESSES[action.id];
    if (!run) {
      return null;
    }
    return run();
  }

  function refreshPreferences() {
    return call("preferences_view")
      .then(function (view) {
        repaintPreferences(view);
        return view;
      })
      .catch(function () {
        return null;
      });
  }

  /* Repaint from an answer, whether or not the surface is on screen.
   *
   * The strip is rebuilt every time, so focus is restored by hand to whichever
   * control the reader was on. Losing the caret to the top of the dialog after
   * every change would make a screen of switches unusable with a keyboard.
   */
  function repaintPreferences(view) {
    state.preferences = view;
    var shell = shellOf("preferences");
    if (!shell || shell.hidden) {
      return view;
    }
    var focused = document.activeElement;
    var focusedId = focused && shell.contains(focused) ? focused.id : "";
    paintPreferences(shell, view);
    if (focusedId) {
      var again = el(focusedId);
      if (again && !again.disabled) {
        again.focus();
      }
    }
    return view;
  }

  function activatePreferenceTab(id) {
    if (!state.preferences) {
      return null;
    }
    var known = (state.preferences.tabs || []).some(function (tab) {
      return tab.id === id;
    });
    if (!known || state.preferences.active === id) {
      return null;
    }
    return call("set_preference_tab", { tab: id })
      .then(repaintPreferences)
      .catch(failed);
  }

  /* Home and End do not move with the layout: they go to the first and the last
     tab either way, which is where they sit on screen. */
  function handlePreferenceTabKeys(event, index, total) {
    if (!total) {
      return;
    }
    var last = total - 1;
    var arrows = tabArrows();
    var next = null;
    if (event.key === arrows.forwards) {
      next = index === last ? 0 : index + 1;
    } else if (event.key === arrows.backwards) {
      next = index === 0 ? last : index - 1;
    } else if (event.key === "Home") {
      next = 0;
    } else if (event.key === "End") {
      next = last;
    }
    if (next === null) {
      return;
    }
    event.preventDefault();
    var tabs = (state.preferences && state.preferences.tabs) || [];
    var target = tabs[next];
    if (!target) {
      return;
    }
    /* The caret is moved after the answer comes back, because the strip is
       rebuilt from that answer: focusing the button that is on screen now would
       focus nothing, since it is the one the rebuild throws away. */
    var moved = activatePreferenceTab(target.id);
    var follow = function () {
      var button = el("prefTab-" + target.id);
      if (button) {
        button.focus();
      }
    };
    if (moved) {
      moved.then(follow);
    } else {
      follow();
    }
  }

/* -------------------------------------------------------------- appearance */

  /* Paint from what the window decided.
   *
   * The page holds the answer and nothing else: the preference, the observation
   * and the resolution all live in the window, so there is no rule here that
   * could disagree with what the window would say next. Two attributes go on the
   * document and the whole page follows them, including every dialog, because
   * they are all one document reading one pair of values.
   */
  function applyAppearance(view) {
    if (!view) {
      return null;
    }
    state.appearance = view;
    var root = document.documentElement;
    root.dataset.theme = view.theme === "dark" ? "dark" : "light";
    root.dir = view.direction === "rtl" ? "rtl" : "ltr";
    cacheAppearance(view);
    renderAppearance(view);
    return view;
  }

  /* Put the window's words into the page.

   *
   *
   * One rule, the same one the theme follows: the page holds no text of its own.
   * An element says which message it wants with data-i18n, and the window's
   * language says what that message is. A name the window does not have leaves
   * the element exactly as it was, so a gap in a locale costs a sentence in the
   * base language rather than a blank heading — and never an identifier, because
   * the window never sends one.
   */
  function applyStrings(strings) {
    if (!strings || typeof strings !== "object") {
      return state.strings;
    }
    state.strings = strings;
    var marked = document.querySelectorAll("[data-i18n]");
    Array.prototype.forEach.call(marked, function (element) {
      var wanted = strings[element.getAttribute("data-i18n")];
      if (typeof wanted === "string" && wanted !== "") {
        setText(element, wanted);
      }
    });
    return state.strings;
  }

  /* The language answer, applied.

   *
   *
   * The window sends what moved rather than telling the page to reload, so the
   * configuration tabs are left alone: what the user has typed into them is not
   * something a language change is entitled to throw away. The three answers
   * that did move - the words, the appearance, and the preferences - are put back
   * in the order the document is built: the language on the document first, so a
   * sentence in Arabic is laid out as Arabic while it arrives.
   */
  function applyLanguage(change) {
    if (!change) {
      return null;
    }
    /* Whatever has the caret keeps what was typed into it. The preferences panel
       is rebuilt by the answer below, and a rebuilt panel is an empty field: the
       window is changing the words, not the answers to anything being typed, so
       the one field the reader is in is put back with its text. */
    var focused = document.activeElement;
    var focusedId =
      focused && focused.tagName === "INPUT" && focused.id ? focused.id : "";
    var typed = focusedId ? focused.value : "";
    var root = document.documentElement;
    if (change.language && change.language.locale) {
      root.lang = change.language.locale;
    }
    if (change.appearance) {
      applyAppearance(change.appearance);
    }
    if (change.language) {
      applyStrings(change.language.strings);
    }
    if (change.preferences) {
      repaintPreferences(change.preferences);
    }
    if (focusedId) {
      var again = el(focusedId);
      if (again && again.value !== typed) {
        again.value = typed;
        again.focus();
      }
    }
    if (change.language && change.language.gaps && change.language.gaps.length) {
      /* A gap is not a failure and not a notice the user has to dismiss; it is
         reported once, where the answer is being shown, because a
         half-translated interface that says nothing about it reads as finished. */
      setText(el("appearanceObservation"), change.language.coverage);
    }
    return change;
  }

  function refreshLanguage() {
    return call("language_view")
      .then(applyLanguage)
      .catch(function () {
        return null;
      });
  }

  function cacheAppearance(view) {
    try {
      window.localStorage.setItem(
        APPEARANCE_CACHE_KEY,
        JSON.stringify({ theme: view.theme, direction: view.direction })
      );
    } catch (error) {
      // A window with nowhere to keep the answer still paints the theme the
      // window gave it; the cache only saves the first frame from being wrong.
    }
  }

  function refreshAppearance() {
    return call("appearance_view")
      .then(applyAppearance)
      .catch(function () {
        return null;
      });
  }

  function renderAppearance(view) {
    setText(el("appearanceObservation"), view.observationNote);
    setText(el("appearanceSummaryText"), view.summary);
    setText(el("appearanceFlow"), view.flowGlyph);
    appearanceOptions("appearanceThemeList", "appearanceTheme", view.themeOptions, "set_theme_mode");
    appearanceOptions(
      "appearanceDirectionList",
      "appearanceDirection",
      view.directionOptions,
      "set_direction_mode"
    );
  }

  /* One renderer for both appearance questions.
   *
   * Native radios, so the group gets arrow-key navigation, one tab stop, and the
   * platform's own announcement, and so the arrow that moves the selection
   * points the way the row it is in is laid out. The window says which option is
   * in force and what each one would mean; the page chooses none of them.
   */
  function appearanceOptions(listId, group, options, command) {
    var list = el(listId);
    if (!list) {
      return;
    }
    clear(list);
    (options || []).forEach(function (option) {
      list.appendChild(
        choiceRow(
          {
            id: option.id,
            label: option.label,
            facts: option.available ? option.facts : option.reason
          },
          group,
          Boolean(option.selected),
          !option.available,
          function () {
            call(command, { mode: option.id })
              .then(applyAppearance)
              .catch(function (error) {
                notice("error", "That could not be saved: " + error);
                // The window refused it, so the control goes back to where the
                // window says it is: the page does not keep a preference that was
                // not accepted.
                refreshAppearance();
              });
          }
        )
      );
    });
  }

  /* Report what this machine is doing, and take the answer.
   *
   * Two sensors on the same question, both of them reporters: the media query
   * tells the window which way `prefers-color-scheme` is pointing, and the
   * window pushes a change of its own on the platforms that report one to the
   * window rather than to the page. Neither decides anything; both arrive at the
   * same observation in the window, so the two cannot produce two themes.
   */
  function watchSystem() {
    var first = reportLocale()
      .then(function (view) {
        /* A locale report that never arrived must not cost the window its pickers,
           and asking for the answer on its own needs no locale, so the one call
           that has no dependency stands in for it. */
        return view || refreshAppearance();
      })
      .then(function (view) {
        /* The words come after the layout rather than with it: the language is
           asked for separately because it is a separate answer, and a window whose
           layout has settled is readable in the wrong language for a moment longer
           than one whose layout is still deciding. */
        return refreshLanguage().then(function () {
          return view;
        });
      });
    if (window.matchMedia) {
      var query = window.matchMedia("(prefers-color-scheme: dark)");
      var report = function () {
        call("report_system_theme", { dark: query.matches })
          .then(applyAppearance)
          .catch(function () {
            return null;
          });
      };
      if (typeof query.addEventListener === "function") {
        query.addEventListener("change", report);
      } else if (typeof query.addListener === "function") {
        query.addListener(report);
      }
      /* The query is reported once as well as on every change: the window is the
         authority from the moment the page has spoken once, so a machine that
         already had its dark theme set before the window opened is recorded as
         the page reading it rather than as the platform having said so. */
      report();
    }
    if (bridge && bridge.event && typeof bridge.event.listen === "function") {
      bridge.event.listen("appearance-changed", function (event) {
        applyAppearance(event && event.payload);
      });
      /* A question the window asked for itself. The countdown alert is normally
         opened by the page pressing a control; the queue finishing is not
         something the user does, so the window pushes the question instead and
         this is the page answering it through the same two controls. */
      bridge.event.listen("alert-asked", function (event) {
        showAskedAlert(event && event.payload);
      });
      /* The notification-area icon was pressed by somebody who cannot see this
         page, so the window decided what should open and said so. The page only
         routes: it does not decide whether a surface may be opened. */
      bridge.event.listen("tray-action", function (event) {
        var payload = event && event.payload;
        if (payload && payload.surface) {
          openSurface(payload.surface);
        }
      });
      /* The language was changed from the preferences, by somebody who may not have
         the window in front at all. The window sends the three answers that moved
         and the page puts them back; it does not reload the page, because that
         would throw away what has been typed into the configuration tabs. */
      bridge.event.listen("language-changed", function (event) {
        applyLanguage(event && event.payload);
      });
    }
    return first;
  }

  function reportLocale() {
    var languages = window.navigator && navigator.languages;
    var locale = (languages && languages[0]) || (window.navigator && navigator.language) || "";
    return call("report_locale", { locale: locale })
      .then(applyAppearance)
      .catch(function () {
        return null;
      });
  }

/* ------------------------------------------------------------------- tabs */

  /* Fill a single-choice control from the view model.
   *
   * Every control carries its own availability and the reason for it, so one
   * renderer covers every picker: an option that cannot be used is left in the
   * list but disabled, and a control that cannot be used at all is disabled with
   * its reason underneath. Nothing is ever shown closed with no explanation.
   */
  function applyChoice(control, onChange, node, hintNode) {
    if (!control) {
      return;
    }
    var select = node || el(control.id);
    if (!select) {
      return;
    }
    clear(select);
    (control.options || []).forEach(function (option) {
      var node = document.createElement("option");
      node.value = option.id;
      node.textContent = option.available ? option.label : option.label + " — " + option.reason;
      node.disabled = !option.available;
      node.selected = option.id === control.selected;
      select.appendChild(node);
    });
    select.disabled = !control.available;
    setHint(
      control.id + "Hint",
      control.available ? control.hint : control.reason,
      control.available ? "" : "blocked",
      hintNode
    );
    describe(select, hintNode || el(control.id + "Hint"));
    select.onchange = function () {
      onChange(select.value);
    };
  }

  /* Fill a number field. The typed text is never rewritten while the field has
     focus, so the caret does not jump while somebody is typing. */
  function applyNumber(field, onChange, node, hintNode) {
    if (!field) {
      return;
    }
    var input = node || el(field.id);
    if (!input) {
      return;
    }
    if (document.activeElement !== input) {
      input.value = field.text || "";
    }
    input.disabled = !field.available;
    var hint = field.available ? field.hint : field.reason;
    if (!field.valid && field.problem) {
      hint = field.problem;
    }
    setHint(
      field.id + "Hint",
      hint || (field.unit ? "Measured in " + field.unit + "." : ""),
      field.valid ? "" : field.available ? "error" : "blocked",
      hintNode
    );
    input.setAttribute("aria-invalid", field.valid ? "false" : "true");
    describe(input, hintNode || el(field.id + "Hint"));
    if (field.available) {
      input.onchange = function () {
        onChange(input.value);
      };
    } else {
      input.onchange = null;
    }
  }

  function applyText(field, onChange, node, hintNode) {
    if (!field) {
      return;
    }
    var input = node || el(field.id);
    if (!input) {
      return;
    }
    if (document.activeElement !== input && !input.readOnly) {
      input.value = field.text || "";
    }
    input.placeholder = field.placeholder || "";
    /* The window's explaining line is the hint, for the same reason it is on
       every other control: it is the reason the control is closed, and a
       preference nobody can read the reason for is a preference that looks
       broken. */
    var hint = field.available ? field.hint : field.reason;
    if (!field.valid && field.problem) {
      hint = field.problem;
    }
    setHint(
      field.id + "Hint",
      hint,
      field.valid ? "" : field.available ? "error" : "blocked",
      hintNode
    );
    describe(input, hintNode || el(field.id + "Hint"));
    if (onChange) {
      input.onchange = function () {
        onChange(input.value);
      };
    }
  }

  function applyToggle(toggle, onChange, node, hintNode) {
    if (!toggle) {
      return;
    }
    var input = node || el(toggle.id);
    if (!input) {
      return;
    }
    input.checked = Boolean(toggle.checked);
    input.disabled = !toggle.available;
    setHint(
      toggle.id + "Hint",
      toggle.available ? toggle.hint : toggle.reason,
      toggle.available ? "" : "blocked",
      hintNode
    );
    describe(input, hintNode || el(toggle.id + "Hint"));
    input.onchange = function () {
      onChange(input.checked);
    };
  }

  /* Point a control at the hint that explains it.
   *
     The reason a control is closed sits in the hint under it, which is the
     right thing for somebody who can see it. Without this the same text is
     never announced, so the explanation only exists for people who do not need
     it. Every control here already knows its hint, so the link is made once
     instead of being repeated in the markup. */
  function describe(control, hint) {
    if (!control || !hint || !hint.id) {
      return;
    }
    control.setAttribute("aria-describedby", hint.id);
  }

  /* The hint under a control, either looked up by the identifier the view model
     carries or handed over by a builder that made the node itself. */
  function setHint(id, text, tone, node) {
    var target = node || el(id);
    if (!target) {
      return;
    }
    setText(target, text || "");
    if (tone) {
      target.dataset.tone = tone;
    } else {
      delete target.dataset.tone;
    }
  }

  /* Send a tab change and paint every tab from the answer.
   *
   * One command returns the whole set, so a change that reaches several tabs —
   * switching to WebM closes the MP4-only flags and re-reads every codec list —
   * shows up everywhere at once, without the page working out what it touched.
   */
  function sendTab(command, change, failure) {
    state.pending += 1;
    return call(command, { change: change })
      .then(function (tabs) {
        if (tabs) {
          renderTabs(tabs);
        }
        return tabs;
      })
      .catch(function (error) {
        var message = String(error && error.message ? error.message : error);
        notice("error", (failure || "That change could not be used.") + " " + message);
        return null;
      })
      .then(function (tabs) {
        state.pending -= 1;
        return tabs;
      });
  }

  function refreshTabs() {
    state.pending += 1;
    return call("tab_set")
      .then(function (tabs) {
        if (tabs) {
          renderTabs(tabs);
        }
      })
      .catch(function (error) {
        notice("error", "The output settings could not be read: " + error);
      })
      .then(function () {
        state.pending -= 1;
      });
  }

  function renderTabs(tabs) {
    state.tabs = tabs;
    setText(
      el("tabSource"),
      tabs.source.chosen
        ? tabs.source.name + " · " + tabs.source.summary
        : tabs.source.summary
    );

    var problems = el("tabProblems");
    if (tabs.problems && tabs.problems.length) {
      setText(problems, tabs.problems.join(" "));
      show(problems, true);
    } else {
      show(problems, false);
    }

    renderTabStrip(tabs);
    renderSummaryTab(tabs.summary);
    renderDimensionsTab(tabs.dimensions);
    renderFiltersTab(tabs.filters);
    renderVideoTab(tabs.video);
    renderAudioTab(tabs.audio);
    renderSubtitlesTab(tabs.subtitles);
    renderChaptersTab(tabs.chapters);
    renderMetadataTab(tabs.metadata);

    var selection = state.selection || {};
    var usable = selection.phase === "ready" && selection.selectedTitleIndex > 0;
    el("queueBtn").disabled = !(usable && tabs.canQueue);
    setText(
      el("queueHint"),
      !usable
        ? "Scan a source and choose a title first."
        : tabs.canQueue
        ? "Queues the chosen title with the settings on these tabs."
        : "Fix the problems above before queueing."
    );
  }

  /* The strip.
   *
   * One tab stop for the whole strip, arrow keys to move between tabs, Home and
   * End for the ends: that is what the platform does, so it is what a keyboard
   * here expects to do. The arrows are the platform's too, which means they point
   * the way the strip is laid out, so a mirrored strip swaps them rather than
   * becoming unreachable with one of them.
   */
  function renderTabStrip(tabs) {
    var list = el("tabList");
    clear(list);
    tabs.tabs.forEach(function (tab, index) {
      var button = document.createElement("button");
      button.type = "button";
      button.id = "tab-" + tab.id;
      button.setAttribute("role", "tab");
      button.setAttribute("aria-selected", tab.id === tabs.active ? "true" : "false");
      button.setAttribute("aria-controls", "panel-" + tab.id);
      button.dataset.attention = tab.needsAttention ? "true" : "false";
      button.title = tab.hint;
      button.tabIndex = tab.id === tabs.active ? 0 : -1;
      /* The strip asks the window's words for its own label, and falls back to the
         label the window sent when the language has none: a tab nobody can name is
         worse than a tab named in the base language, and the window's label is
         already there. */
      button.dataset.i18n = "tab." + tab.id;
      setText(button, state.strings["tab." + tab.id] || tab.label);
      button.addEventListener("click", function () {
        activateTab(tab.id);
      });
      button.addEventListener("keydown", function (event) {
        handleTabKeys(event, tabs.tabs, index);
      });
      list.appendChild(button);
    });
    showPanels(tabs);
  }

  function activateTab(id) {
    if (!state.tabs) {
      return;
    }
    var known = state.tabs.tabs.some(function (tab) {
      return tab.id === id;
    });
    if (!known || state.tabs.active === id) {
      return;
    }
    /* What is on screen is the answer to "which tab am I on", so the stored copy
       is moved to it rather than left naming the tab the window last reported.
       Left naming the old one means the strip can be walked off a tab but never
       back onto it: the arrow key homewards is compared against a tab that is
       no longer showing, matches, and does nothing. The next read from the
       window replaces this anyway, and it is the window that decides which tab
       is active in the first place. */
    state.tabs.active = id;
    showPanels({ tabs: state.tabs.tabs, active: id });
    run("set_active_tab", { tab: id }, "Could not switch tabs.");
  }

  function showPanels(tabs) {
    (tabs.tabs || []).forEach(function (tab) {
      var button = el("tab-" + tab.id);
      var panel = el("panel-" + tab.id);
      if (button) {
        button.setAttribute("aria-selected", tab.id === tabs.active ? "true" : "false");
        button.tabIndex = tab.id === tabs.active ? 0 : -1;
        button.dataset.attention = tab.needsAttention ? "true" : "false";
      }
      show(panel, tab.id === tabs.active);
    });
  }

  /* Which way the strip is laid out, from what the window resolved.
   *
   * Home and End do not move: they go to the first and last tab either way,
   * which is where they sit on screen.
   */
  function tabArrows() {
    var mirrored = state.appearance.direction === "rtl";
    return {
      forwards: mirrored ? "ArrowLeft" : "ArrowRight",
      backwards: mirrored ? "ArrowRight" : "ArrowLeft"
    };
  }

  function handleTabKeys(event, list, index) {
    var last = list.length - 1;
    var next = null;
    var arrows = tabArrows();
    if (event.key === arrows.forwards) {
      next = index === last ? 0 : index + 1;
    } else if (event.key === arrows.backwards) {
      next = index === 0 ? last : index - 1;
    } else if (event.key === "Home") {
      next = 0;
    } else if (event.key === "End") {
      next = last;
    }
    if (next === null) {
      return;
    }
    event.preventDefault();
    var target = list[next];
    activateTab(target.id);
    var button = el("tab-" + target.id);
    if (button) {
      button.focus();
    }
  }

  function renderSummaryTab(tab) {
    applyChoice(tab.container, function (value) {
      sendTab("set_summary", { container: value }, "That container could not be used.");
    });
    applyChoice(tab.videoCodec, function (value) {
      sendTab("set_summary", { videoCodec: value }, "That video encoder could not be used.");
    });
    applyChoice(tab.audioCodec, function (value) {
      sendTab("set_summary", { audioCodec: value }, "That audio encoder could not be used.");
    });
    applyToggle(tab.webOptimized, function (value) {
      sendTab("set_summary", { webOptimized: value }, "That flag could not be changed.");
    });
    applyToggle(tab.ipodCompatible, function (value) {
      sendTab("set_summary", { ipodCompatible: value }, "That flag could not be changed.");
    });
    applyToggle(tab.alignAudioVideo, function (value) {
      sendTab("set_summary", { alignAudioVideo: value }, "That flag could not be changed.");
    });
    applyToggle(tab.passthroughMetadata, function (value) {
      sendTab("set_summary", { passthroughMetadata: value }, "That flag could not be changed.");
    });
    setText(el("summaryFormat"), tab.formatLabel);
    setText(el("summaryQuality"), tab.qualityLabel);
    setText(el("summaryFlagLabel"), tab.flagLabel);
  }

  /* Crop and scale fields all send one field each, so the window only ever sees
     the control the user actually touched. */
  var DIMENSION_FIELDS = [
    ["cropX", "cropX"],
    ["cropY", "cropY"],
    ["cropWidth", "cropWidth"],
    ["cropHeight", "cropHeight"],
    ["scaleWidth", "scaleWidth"],
    ["scaleHeight", "scaleHeight"],
    ["maxWidth", "maxWidth"],
    ["maxHeight", "maxHeight"]
  ];

  function renderDimensionsTab(tab) {
    applyChoice(tab.cropMode, function (value) {
      sendTab("set_dimensions", { cropMode: value }, "That crop mode could not be used.");
    });
    applyChoice(tab.scaleMode, function (value) {
      sendTab("set_dimensions", { scaleMode: value }, "That scaling mode could not be used.");
    });
    DIMENSION_FIELDS.forEach(function (pair) {
      applyNumber(tab[pair[0]], function (value) {
        var change = {};
        change[pair[1]] = value;
        sendTab("set_dimensions", change, "That size could not be used.");
      });
    });
    applyText(tab.aspectRatio, function (value) {
      sendTab("set_dimensions", { aspectRatio: value }, "That shape could not be read.");
    });
    applyChoice(tab.rotate, function (value) {
      sendTab("set_dimensions", { rotate: value }, "That turn could not be used.");
    });
    applyChoice(tab.flip, function (value) {
      sendTab("set_dimensions", { flip: value }, "That mirror could not be used.");
    });
    setText(el("dimensionsSummary"), tab.summary + " Output: " + tab.outputLabel + ".");
  }

  function renderFiltersTab(tab) {
    var host = el("filterRows");
    clear(host);
    tab.rows.forEach(function (row) {
      host.appendChild(filterRow(row));
    });
    setText(el("filtersSummary"), tab.summary);
  }

  function filterRow(row) {
    var card = document.createElement("div");
    card.className = "filter-row";
    card.dataset.enabled = row.enabled ? "true" : "false";
    card.dataset.available = row.available ? "true" : "false";

    var head = document.createElement("div");
    head.className = "filter-row-head";
    var title = document.createElement("span");
    title.className = "title";
    setText(title, row.label);
    head.appendChild(title);
    var toggle = document.createElement("input");
    toggle.type = "checkbox";
    toggle.checked = Boolean(row.enabled);
    toggle.disabled = !row.available;
    toggle.setAttribute("aria-label", row.label);
    toggle.addEventListener("change", function () {
      sendTab(
        "set_filter",
        { filter: row.id, enabled: toggle.checked },
        "That filter could not be changed."
      );
    });
    head.appendChild(toggle);
    card.appendChild(head);

    var hint = document.createElement("p");
    hint.className = "hint";
    if (!row.available) {
      hint.dataset.tone = "blocked";
    }
    setText(hint, row.available ? row.hint + " " + row.summary : row.reason);
    card.appendChild(hint);

    if (row.picks.length || row.fields.length) {
      var body = document.createElement("div");
      body.className = "filter-row-body";
      row.picks.forEach(function (pick) {
        body.appendChild(
          controlGroup(pick, function (value) {
            sendTab(
              "set_filter",
              { filter: row.id, choice: value },
              "That choice could not be used."
            );
          })
        );
      });
      row.fields.forEach(function (field, index) {
        body.appendChild(
          controlGroup(field, function (value) {
            // The first custom field of a row travels as `value`, the second as
            // `valueTwo`.
            var change = { filter: row.id };
            change[index === 0 ? "value" : "valueTwo"] = value;
            sendTab("set_filter", change, "That value could not be used.");
          })
        );
      });
      card.appendChild(body);
    }
    return card;
  }

  /* A labelled control with its hint underneath, whatever kind it is.
   *
   * The nodes are built here and wired here: a control that is not in the
   * document yet cannot be found by identifier, so the wiring is handed the
   * nodes rather than looking them up. */
  function controlGroup(control, onChange) {
    var field = document.createElement("div");
    field.className = "field";
    var label = document.createElement("label");
    label.textContent = control.label;
    label.htmlFor = control.id;
    field.appendChild(label);

    var hint = document.createElement("span");
    hint.className = "hint";
    hint.id = control.id + "Hint";
    field.appendChild(hint);

    if (control.options) {
      var select = document.createElement("select");
      select.id = control.id;
      field.insertBefore(select, hint);
      applyChoice(control, onChange, select, hint);
    } else {
      var input = document.createElement("input");
      input.type = "text";
      input.id = control.id;
      input.inputMode = control.unit === "fps" ? "decimal" : "numeric";
      field.insertBefore(input, hint);
      applyNumber(control, onChange, input, hint);
    }
    return field;
  }

  function renderVideoTab(tab) {
    applyChoice(tab.encoder, function (value) {
      sendTab("set_video", { encoder: value }, "That encoder could not be used.");
    });
    applyChoice(tab.rateControl, function (value) {
      sendTab("set_video", { rateControl: value }, "That rate control could not be used.");
    });
    applyNumber(tab.quality, function (value) {
      sendTab("set_video", { quality: value }, "That quality could not be used.");
    });
    applyNumber(tab.bitrate, function (value) {
      sendTab("set_video", { bitrate: value }, "That bit rate could not be used.");
    });
    applyNumber(tab.targetSize, function (value) {
      sendTab("set_video", { targetSize: value }, "That file size could not be used.");
    });
    applyToggle(tab.twoPass, function (value) {
      sendTab("set_video", { twoPass: value }, "That switch could not be changed.");
    });
    applyToggle(tab.lossless, function (value) {
      sendTab("set_video", { lossless: value }, "That switch could not be changed.");
    });
    applyChoice(tab.preset, function (value) {
      sendTab("set_video", { preset: value }, "That preset could not be used.");
    });
    applyChoice(tab.profile, function (value) {
      sendTab("set_video", { profile: value }, "That profile could not be used.");
    });
    applyChoice(tab.tune, function (value) {
      sendTab("set_video", { tune: value }, "That tune could not be used.");
    });
    applyChoice(tab.level, function (value) {
      sendTab("set_video", { level: value }, "That level could not be used.");
    });
    applyChoice(tab.colourRange, function (value) {
      sendTab("set_video", { colourRange: value }, "That colour range could not be used.");
    });
    applyChoice(tab.frameRateMode, function (value) {
      sendTab("set_video", { frameRateMode: value }, "That frame-rate mode could not be used.");
    });
    applyNumber(tab.frameRate, function (value) {
      sendTab("set_video", { frameRate: value }, "That frame rate could not be used.");
    });
    applyChoice(tab.dolbyVision, function (value) {
      sendTab("set_video", { dolbyVision: value }, "That Dolby Vision action could not be used.");
    });
    applyChoice(tab.hdr10Plus, function (value) {
      sendTab("set_video", { hdr10Plus: value }, "That HDR10+ action could not be used.");
    });
    setText(
      el("videoPlan"),
      tab.planLabel + (tab.notes.length ? " " + tab.notes.join(" ") : "")
    );
  }

  /* A set of audio rules, used for the defaults and for every track. */
  var AUDIO_FIELDS = [
    ["encoder", "encoder"],
    ["bitrate", "bitrate"],
    ["quality", "quality"],
    ["mixdown", "mixdown"],
    ["sampleRate", "sampleRate"],
    ["ditherMode", "ditherMode"],
    ["gain", "gain"],
    ["delay", "delay"]
  ];
  var AUDIO_SWITCHES = ["passthrough", "dither", "normalize"];

  function audioRuleSet(rules, title, own, onChange) {
    var card = document.createElement("div");
    card.className = "rule-set";
    var heading = document.createElement("h3");
    setText(heading, title);
    if (own && own.length) {
      var ownTag = document.createElement("span");
      ownTag.className = "own-rules";
      setText(ownTag, "own: " + own.join(", "));
      heading.appendChild(ownTag);
    }
    card.appendChild(heading);

    var grid = document.createElement("div");
    grid.className = "rule-grid";
    AUDIO_FIELDS.forEach(function (pair) {
      grid.appendChild(
        controlGroup(rules[pair[0]], function (value) {
          onChange(pair[1], value);
        })
      );
    });
    card.appendChild(grid);

    var flags = document.createElement("div");
    flags.className = "rule-flags";
    AUDIO_SWITCHES.forEach(function (name) {
      flags.appendChild(
        switchRow(rules[name], function (checked) {
          onChange(name, checked);
        })
      );
    });
    card.appendChild(flags);

    var summary = document.createElement("p");
    summary.className = "hint";
    if (rules.problems.length) {
      summary.dataset.tone = "error";
    }
    setText(
      summary,
      rules.summary + (rules.problems.length ? " " + rules.problems.join(" ") : "")
    );
    card.appendChild(summary);
    return card;
  }

  function switchRow(toggle, onChange) {
    var label = document.createElement("label");
    label.className = "switch";
    var input = document.createElement("input");
    input.type = "checkbox";
    input.id = toggle.id;
    input.checked = Boolean(toggle.checked);
    input.disabled = !toggle.available;
    var text = document.createElement("span");
    text.className = "text";
    var name = document.createElement("span");
    setText(name, toggle.label);
    var facts = document.createElement("span");
    facts.className = "facts";
    facts.id = toggle.id + "Hint";
    if (!toggle.available) {
      facts.dataset.tone = "blocked";
    }
    setText(facts, toggle.available ? toggle.hint : toggle.reason);
    text.appendChild(name);
    text.appendChild(facts);
    label.appendChild(input);
    label.appendChild(text);
    input.onchange = function () {
      onChange(input.checked);
    };
    return label;
  }

  function renderAudioTab(tab) {
    var defaults = el("audioDefaults");
    clear(defaults);
    defaults.appendChild(
      audioRuleSet(tab.defaults, "Defaults for every track", null, function (field, value) {
        var change = {};
        change[field] = value;
        sendTab("set_audio_defaults", change, "That audio default could not be used.");
      })
    );

    var tracks = el("audioTracks");
    clear(tracks);
    show(el("audioTabEmpty"), !tab.tracks.length);
    tab.tracks.forEach(function (track) {
      var wrap = document.createElement("div");
      var include = document.createElement("input");
      include.type = "checkbox";
      include.checked = track.enabled;
      include.setAttribute("aria-label", "Include " + track.label);
      include.addEventListener("change", function () {
        sendTab(
          "set_audio_track",
          { track: track.number, enabled: include.checked },
          "That track could not be changed."
        );
      });
      wrap.appendChild(
        switchRow(
          {
            id: "audioIncludeTrack" + track.number,
            label: track.label,
            hint: track.own.length ? "This track has its own settings." : "Inherits every default.",
            checked: track.enabled,
            available: true
          },
          function (checked) {
            sendTab(
              "set_audio_track",
              { track: track.number, enabled: checked },
              "That track could not be changed."
            );
          }
        )
      );
      wrap.appendChild(
        audioRuleSet(track.rules, "Track " + track.number, track.own, function (field, value) {
          var change = { track: track.number };
          change[field] = value;
          sendTab("set_audio_track", change, "That track setting could not be used.");
        })
      );
      tracks.appendChild(wrap);
    });
    setText(el("audioSummary"), tab.summary);
  }

  function renderSubtitlesTab(tab) {
    applyChoice(tab.behaviour, function (value) {
      sendTab("set_subtitles", { behaviour: value }, "That subtitle choice could not be used.");
    });
    applyToggle(tab.forcedOnly, function (value) {
      sendTab("set_subtitles", { forcedOnly: value }, "That switch could not be changed.");
    });
    applyText(tab.search, function (value) {
      sendTab("set_subtitles", { search: value }, "That search could not be applied.");
    });

    var external = el("externalSubtitle");
    state.importedSubtitle = tab.externalPath || "";
    if (document.activeElement !== external) {
      external.value = tab.externalPath || "";
    }
    setHint(
      "externalSubtitleHint",
      tab.external.valid
        ? tab.external.text
          ? "Imported: " + tab.external.text + ". It is written alongside the chosen tracks."
          : "Type a path and press Import to add a subtitle file to the output."
        : tab.external.problem,
      tab.external.valid ? "" : "blocked"
    );

    var tracks = el("subtitleTracks");
    clear(tracks);
    show(el("subtitleTabEmpty"), !tab.tracks.length);
    tab.tracks.forEach(function (track) {
      var card = document.createElement("div");
      card.className = "rule-set";
      card.appendChild(
        switchRow(
          {
            id: "subtitleIncludeTrack" + track.number,
            label: track.label,
            hint: track.available ? track.summary : track.reason,
            checked: track.enabled,
            available: true
          },
          function (checked) {
            sendTab(
              "set_subtitle_track",
              { track: track.number, enabled: checked },
              "That subtitle track could not be changed."
            );
          }
        )
      );
      var grid = document.createElement("div");
      grid.className = "rule-grid";
      grid.appendChild(
        controlGroup(track.behaviour, function (value) {
          sendTab(
            "set_subtitle_track",
            { track: track.number, behaviour: value },
            "That subtitle choice could not be used."
          );
        })
      );
      card.appendChild(grid);
      card.appendChild(
        switchRow(track.forcedOnly, function (checked) {
          sendTab(
            "set_subtitle_track",
            { track: track.number, forcedOnly: checked },
            "That switch could not be changed."
          );
        })
      );
      tracks.appendChild(card);
    });
    setText(
      el("subtitlesSummary"),
      tab.summary + (tab.hidden ? " " + tab.hidden + " track(s) hidden by the search." : "")
    );
  }

  function renderChaptersTab(tab) {
    applyChoice(tab.mode, function (value) {
      sendTab("set_chapters", { mode: value }, "That chapter choice could not be used.");
    });

    var host = el("chapterMarkers");
    clear(host);
    show(el("chapterTabEmpty"), !tab.markers.length);
    tab.markers.forEach(function (marker) {
      host.appendChild(markerCard(marker));
    });
    setText(el("chaptersSummary"), tab.summary + (tab.available ? "" : " " + tab.reason));
    offer(el("addMarkerBtn"), tab.available);
    paintChapterImport(tab.import);
  }

  // The import is shown before it is accepted: the claim says what replacing the
  // list would do, the preview says what would be in it, and the button stays
  // closed until there is something that actually parsed. A file that failed is
  // reported in the same place as a failed number rather than silently ignored.
  function paintChapterImport(model) {
    if (!model) return;
    applyChoice(model.format, function (value) {
      sendTab("import_chapters", { format: value }, "That chapter file type could not be read.");
    });
    var source = el("chapterImportSource");
    if (source && document.activeElement !== source) {
      source.value = model.source.text;
    }
    if (source) {
      source.setAttribute("aria-describedby", "chapterImportClaim");
    }
    setText(el("chapterImportClaim"), model.outcome.claim);
    setText(el("chapterImportProblem"), model.outcome.reason);

    var preview = el("chapterImportPreview");
    clear(preview);
    model.outcome.markers.forEach(function (marker) {
      var item = document.createElement("li");
      item.className = "marker";
      item.textContent = marker.label;
      preview.appendChild(item);
    });

    var button = el("chapterImportBtn");
    button.disabled = !model.outcome.parsed;
    button.setAttribute(
      "aria-describedby",
      model.outcome.parsed ? "chapterImportClaim" : "chapterImportProblem"
    );
    if (source) {
      source.oninput = function () {
        sendTab(
          "import_chapters",
          { text: source.value },
          "That chapter file could not be read."
        );
      };
    }
    button.onclick = function () {
      sendTab(
        "import_chapters",
        { accept: true },
        "That chapter file could not be used."
      );
    };
  }

  function markerCard(marker) {
    var card = document.createElement("div");
    card.className = "marker";

    var head = document.createElement("div");
    head.className = "marker-head";
    var include = document.createElement("input");
    include.type = "checkbox";
    include.checked = marker.included;
    include.setAttribute("aria-label", "Include " + marker.name);
    include.addEventListener("change", function () {
      sendTab(
        "change_marker",
        { action: include.checked ? "include" : "exclude", number: marker.number },
        "That marker could not be changed."
      );
    });
    var name = document.createElement("span");
    setText(name, marker.label);
    head.appendChild(include);
    head.appendChild(name);
    card.appendChild(head);

    var grid = document.createElement("div");
    grid.className = "marker-grid";
    ["start", "end"].forEach(function (field) {
      grid.appendChild(
        markerField(marker, field, field === "start" ? "Start" : "End", function (value) {
          var change = { action: "edit", number: marker.number };
          change[field] = value;
          sendTab("change_marker", change, "That marker could not be changed.");
        })
      );
    });
    grid.appendChild(
      markerField(marker, "nameField", "Name", function (value) {
        sendTab(
          "change_marker",
          { action: "edit", number: marker.number, name: value },
          "That marker name could not be changed."
        );
      })
    );
    card.appendChild(grid);

    if (marker.problem) {
      var problem = document.createElement("p");
      problem.className = "hint";
      problem.dataset.tone = "error";
      setText(problem, marker.problem);
      card.appendChild(problem);
    }

    var actions = document.createElement("div");
    actions.className = "marker-actions";
    var remove = document.createElement("button");
    remove.type = "button";
    remove.className = "quiet";
    remove.textContent = "Remove";
    remove.addEventListener("click", function () {
      sendTab(
        "change_marker",
        { action: "remove", number: marker.number },
        "That marker could not be removed."
      );
    });
    actions.appendChild(remove);
    card.appendChild(actions);
    return card;
  }

  function markerField(marker, field, label, onChange) {
    var control = marker[field];
    var wrapper = document.createElement("div");
    wrapper.className = "field";
    var caption = document.createElement("label");
    setText(caption, label);
    caption.htmlFor = control.id;
    wrapper.appendChild(caption);
    var input = document.createElement("input");
    input.type = "text";
    input.id = control.id;
    input.inputMode = field === "nameField" ? "text" : "decimal";
    input.disabled = control.available === false;
    input.setAttribute("aria-invalid", control.valid ? "false" : "true");
    if (document.activeElement !== input) {
      input.value = control.text || "";
    }
    wrapper.appendChild(input);
    var hint = document.createElement("span");
    hint.className = "hint";
    hint.id = control.id + "Hint";
    if (!control.available) {
      hint.dataset.tone = "blocked";
      setText(hint, control.reason);
    } else if (!control.valid) {
      hint.dataset.tone = "error";
      setText(hint, control.problem);
    }
    wrapper.appendChild(hint);
    if (control.available) {
      input.onchange = function () {
        onChange(input.value);
      };
    }
    return wrapper;
  }
  function renderMetadataTab(tab) {
    applyChoice(tab.mode, function (value) {
      sendTab("set_metadata", { action: "mode", mode: value }, "That metadata mode could not be used.");
    });
    setText(el("metadataSourceNote"), tab.sourceNote);

    var switches = el("metadataAttachments");
    clear(switches);
    [tab.coverArt, tab.attachments, tab.clear].forEach(function (toggle) {
      switches.appendChild(
        switchRow(toggle, function (checked) {
          var change = { action: toggle.id.replace("metadata", "").toLowerCase() };
          change.checked = checked;
          sendTab("set_metadata", change, "That switch could not be changed.");
        })
      );
    });

    var tags = el("metadataTags");
    clear(tags);
    show(el("metadataTagEmpty"), !tab.tags.length);
    (tab.tags || []).forEach(function (tag) {
      tags.appendChild(tagRow(tag));
    });

    var newKey = el("metadataNewKey");
    var newValue = el("metadataNewValue");
    fill(newKey, tab.newKey.text);
    newKey.disabled = !tab.newKey.available;
    setHint("metadataNewKeyHint", tab.newKey.available ? tab.newKey.hint : tab.newKey.reason,
      tab.newKey.available ? "" : "blocked");
    newKey.setAttribute("aria-invalid", tab.newKey.valid ? "false" : "true");
    if (tab.newKey.available && !tab.newKey.valid && tab.newKey.problem) {
      setHint("metadataNewKeyHint", tab.newKey.problem, "error");
    }
    newKey.setAttribute("aria-describedby", "metadataNewKeyHint");
    fill(newValue, tab.newValue.text);
    newValue.disabled = !tab.newValue.available;
    setHint("metadataNewValueHint", tab.newValue.available ? tab.newValue.hint : tab.newValue.reason,
      tab.newValue.available ? "" : "blocked");
    newValue.setAttribute("aria-describedby", "metadataNewValueHint");
    var add = el("metadataAddBtn");
    offer(add, tab.newKey.available && tab.newValue.available);
    if (!tab.newKey.available) {
      add.setAttribute("title", tab.newKey.reason);
    }

    var problems = (tab.problems || []).join(" ");
    setText(el("metadataSummary"), problems ? tab.summary + " " + problems : tab.summary);
    el("metadataSummary").dataset.tone = problems ? "error" : "";
  }

  function tagRow(tag) {
    var row = node("div", "tag");
    row.dataset.tone = tag.tone;
    row.appendChild(node("span", "tag-key", tag.key));
    row.appendChild(node("span", "chip", tag.origin));

    var value = node("input");
    value.id = tag.id;
    value.type = "text";
    value.spellcheck = false;
    value.disabled = !tag.editable;
    fill(value, tag.text);
    value.setAttribute("aria-label", "Value for " + tag.key);
    var hint = node("span", "hint", tag.reason || (tag.written ? "Written to the output." : "Not written."));
    hint.id = tag.id + "Hint";
    hint.dataset.tone = tag.editable ? "" : "blocked";
    value.setAttribute("aria-describedby", hint.id);
    value.onchange = function () {
      sendTab(
        "set_metadata",
        { action: "edit", key: tag.key, value: value.value },
        "That tag could not be changed."
      );
    };
    row.appendChild(value);
    row.appendChild(hint);

    if (tag.written) {
      row.appendChild(node("span", "chip written", "written"));
    } else {
      row.appendChild(node("span", "chip", "not written"));
    }

    var actions = node("div", "controls");
    if (tag.restorable) {
      var restore = node("button", "quiet", "Put back");
      restore.type = "button";
      restore.id = tag.id + "Restore";
      restore.addEventListener("click", function () {
        sendTab(
          "set_metadata",
          { action: "restore", key: tag.key },
          "That tag could not be put back."
        );
      });
      actions.appendChild(restore);
    }
    if (tag.removable) {
      var remove = node("button", "quiet", "Remove");
      remove.type = "button";
      remove.id = tag.id + "Remove";
      remove.addEventListener("click", function () {
        sendTab(
          "set_metadata",
          { action: "remove", key: tag.key },
          "That tag could not be removed."
        );
      });
      actions.appendChild(remove);
    }
    row.appendChild(actions);
    return row;
  }

  /* ------------------------------------------------------------------ wire */

  function wire() {
    el("openBtn").addEventListener("click", function () {
      var path = el("pathInput").value.trim();
      if (!path) {
        notice("error", "Type the full path of a file or folder first.");
        return;
      }
      openSource(path);
    });

    el("pathInput").addEventListener("keydown", function (event) {
      if (event.key === "Enter") {
        event.preventDefault();
        el("openBtn").click();
      }
    });

    el("scanBtn").addEventListener("click", startScan);

    el("clearSourceBtn").addEventListener("click", function () {
      run("clear_source", {}, "Could not forget the source.");
    });

    el("startPoint").addEventListener("change", function () {
      run(
        "choose_range",
        {
          choice: {
            mode: "",
            startSeconds: Number(el("startPoint").value),
            endSeconds: Number(el("endPoint").value)
          }
        },
        "That start point is not usable."
      );
    });

    el("endPoint").addEventListener("change", function () {
      run(
        "choose_range",
        {
          choice: {
            mode: "",
            startSeconds: Number(el("startPoint").value),
            endSeconds: Number(el("endPoint").value)
          }
        },
        "That end point is not usable."
      );
    });

    el("angleSelect").addEventListener("change", function () {
      run(
        "choose_range",
        { choice: { mode: "", angle: Number(el("angleSelect").value) } },
        "That angle is not usable."
      );
    });

    el("clearRecentsBtn").addEventListener("click", function () {
      call("clear_recent_sources").then(loadRecents);
    });

    el("importSubtitlesBtn").addEventListener("click", importSubtitles);

    el("forgetSubtitlesBtn").addEventListener("click", function () {
      // An empty path is how the window is told to forget the imported file.
      state.importedSubtitle = "";
      state.pending += 1;
      call("import_subtitles", { path: "" })
        .then(function (tabs) {
          if (tabs) {
            renderTabs(tabs);
          }
        })
        .catch(function (error) {
          notice("error", "That subtitle file could not be forgotten: " + error);
        })
        .then(function () {
          state.pending -= 1;
        });
    });

    el("addMarkerBtn").addEventListener("click", function () {
      sendTab(
        "change_marker",
        {
          action: "add",
          name: el("markerName").value,
          start: el("markerStart").value,
          end: el("markerEnd").value
        },
        "That marker could not be added."
      );
      el("markerName").value = "";
      el("markerStart").value = "";
      el("markerEnd").value = "";
    });

    el("queueBtn").addEventListener("click", queueSelection);

    el("openSelectionBtn").addEventListener("click", function () {
      openSurface("queue-selection");
    });

    el("metadataAddBtn").addEventListener("click", function () {
      sendTab(
        "set_metadata",
        {
          action: "add",
          key: el("metadataNewKey").value,
          value: el("metadataNewValue").value
        },
        "That tag could not be added."
      );
      el("metadataNewKey").value = "";
      el("metadataNewValue").value = "";
    });

    wireSurfaceKeys();
    wireDropzone();
  }

  function start() {
    // A control that silently stops working is worse than one that says why,
    // so a page-level failure is reported in the source card.
    window.addEventListener("error", function (event) {
      notice("error", "Page error: " + (event.message || "unknown"));
    });
    window.addEventListener("unhandledrejection", function (event) {
      var reason = event.reason;
      notice(
        "error",
        "Page error: " + String(reason && reason.message ? reason.message : reason)
      );
    });
    /* The window can ask a question on its own, and it does so when the queue
       drains with the when-done preferences set to ask. Confirming that question
       is routed here by the action's name. */
    state.confirmRunners = {
      close_window: function () {
        return call("when_done_confirm").catch(failed);
      }
    };
    wire();
    if (!invoke) {
      unavailable("This page needs the desktop window; open it from the application.");
      renderRecents();
      return;
    }
    loadCatalog()
      .then(function () {
        renderRecents();
        refreshTabs();
        refreshSelection();
        return refreshQueue();
      })
      .catch(function (error) {
        notice("error", "The windows could not be read: " + error);
      });
    /* The theme and the direction are independent of the rest of the window, so
       they are asked for alongside it rather than in front of it: the inline
       script has already put the last answer on the document, so nothing waits
       on this to be readable. */
    watchSystem();
    window.setInterval(pollWindows, 1500);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();
