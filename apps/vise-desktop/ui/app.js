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
    /* What the countdown alert is holding on the user's behalf. */
    confirm: null,
    alertTimer: null
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

    host.appendChild(presetGroup("Presets that ship with this build", view.builtIn));
    host.appendChild(presetGroup("Your presets", view.user));
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
        return run ? run() : null;
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
    var hint = field.available ? "" : field.reason;
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
   * here expects to do.
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
      setText(button, tab.label);
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
    if (!state.tabs || state.tabs.active === id) {
      return;
    }
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

  function handleTabKeys(event, list, index) {
    var last = list.length - 1;
    var next = null;
    if (event.key === "ArrowRight") {
      next = index === last ? 0 : index + 1;
    } else if (event.key === "ArrowLeft") {
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
    window.setInterval(pollWindows, 1500);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();
