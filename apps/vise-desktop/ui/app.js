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
  var state = { selection: null, recents: [], pending: 0, queue: [], tabs: null, importedSubtitle: "" };

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
    return call("begin_scan", {
      options: {
        previewCount: 0,
        minDuration: "",
        maxDuration: "",
        keepDuplicateTitles: false,
        fileExclusions: []
      }
    })
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

  /* ------------------------------------------------------------------ queue */

  function refreshQueue() {
    return call("list_jobs")
      .then(function (jobs) {
        state.queue = jobs || [];
        renderQueue();
      })
      .catch(function () {
        state.queue = [];
        renderQueue();
      });
  }

  function renderQueue() {
    var body = el("queueBody");
    clear(body);
    var jobs = state.queue;
    show(el("queueEmpty"), jobs.length === 0);
    jobs.forEach(function (job) {
      var row = document.createElement("tr");

      var input = document.createElement("td");
      input.className = "name";
      var status = job.status || {};
      var percent = Math.round(((status.progress && status.progress.fraction) || 0) * 100);
      input.title = job.spec.input_path;
      input.textContent = (job.spec.input_path || "").split(/[\\/]/).pop();
      row.appendChild(input);

      var stateCell = document.createElement("td");
      stateCell.textContent = status.state || "";
      row.appendChild(stateCell);

      var progressCell = document.createElement("td");
      var bar = document.createElement("div");
      bar.className = "progress";
      var fill = document.createElement("div");
      fill.style.width = percent + "%";
      bar.appendChild(fill);
      progressCell.appendChild(bar);
      var percentText = document.createElement("span");
      percentText.textContent = " " + percent + "%";
      progressCell.appendChild(percentText);
      row.appendChild(progressCell);

      var message = document.createElement("td");
      message.textContent = status.message || "";
      row.appendChild(message);

      var controls = document.createElement("td");
      controls.className = "controls";
      [
        ["Run", "run_job"],
        ["Pause", "pause_job"],
        ["Resume", "resume_job"],
        ["Cancel", "cancel_job"],
        ["Remove", "remove_job"]
      ].forEach(function (entry) {
        var button = document.createElement("button");
        button.type = "button";
        button.textContent = entry[0];
        button.addEventListener("click", function () {
          call(entry[1], { id: job.id })
            .then(refreshQueue)
            .catch(function (error) {
              notice("error", entry[0] + " failed: " + error);
            });
        });
        controls.appendChild(button);
      });
      row.appendChild(controls);
      body.appendChild(row);
    });
  }

  function refreshLogs() {
    return call("logs")
      .then(function (lines) {
        var list = el("logList");
        clear(list);
        if (!lines || !lines.length) {
          var empty = document.createElement("div");
          setText(empty, "No activity recorded yet.");
          list.appendChild(empty);
          return;
        }
        lines.forEach(function (line) {
          var entry = document.createElement("div");
          entry.textContent = line;
          list.appendChild(entry);
        });
        list.scrollTop = list.scrollHeight;
      })
      .catch(function () {
        clear(el("logList"));
      });
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
    input.onchange = function () {
      onChange(input.checked);
    };
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
  /* ------------------------------------------------------------------- wire */

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

    el("clearLogsBtn").addEventListener("click", function () {
      call("clear_logs").then(refreshLogs);
    });

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
    renderRecents();
    refreshTabs();
    refreshSelection();
    refreshQueue();
    refreshLogs();
    window.setInterval(refreshQueue, 1500);
    window.setInterval(refreshLogs, 1500);
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", start);
  } else {
    start();
  }
})();
