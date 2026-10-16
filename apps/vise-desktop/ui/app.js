/* Vise Transcoder desktop page.
 *
 * The page is a renderer. Every decision about a source, a scan, a title, a
 * range, or a track is made in the window (Rust) and arrives here as a finished
 * object, so this file only wires controls to commands and paints the result.
 * No media logic, no formatting rules: what the user reads is built and tested
 * in Rust.
 */
(function () {
  "use strict";

  var bridge = window.__TAURI__ || null;
  var invoke = bridge && bridge.core ? bridge.core.invoke : null;
  var state = { selection: null, recents: [], pending: 0, queue: [] };

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

    var usable = selection.phase === "ready" && selection.selectedTitleIndex > 0;
    el("queueBtn").disabled = !usable;
    setText(
      el("queueHint"),
      usable
        ? "Queues the chosen title with the tracks marked above."
        : "Scan a source and choose a title first."
    );
  }

  /* ------------------------------------------------------------------ calls */

  function run(command, args, failure) {
    state.pending += 1;
    return call(command, args)
      .then(function (result) {
        if (result && result.phase !== undefined) {
          render(result);
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
    var spec = {
      input_path: (state.selection && state.selection.source && state.selection.source.path) || "",
      output_path: el("outPath").value.trim(),
      container: el("container").value,
      video_codec: el("video").value,
      audio_codec: el("audioCodec").value,
      subtitle_mode: "passthrough",
      crop: null,
      scale_to: null,
      quality: null
    };
    state.pending += 1;
    return call("queue_selection", { spec: spec })
      .then(refreshQueue)
      .catch(function (error) {
        notice("error", "Could not queue the selection: " + error);
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
