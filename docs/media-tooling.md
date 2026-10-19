# Bundled media tooling

A shipped build carries its own `ffmpeg` and `ffprobe` rather than relying on
whatever the user's machine happens to have. Two reasons:

- The encoders and filters the application names (deblock, telecine removal,
  the lossless and hardware codecs) are only present in some builds. A user who
  installs the application should not also have to install a matching encoder.
- Results should not change because the machine changed.

## How a tool is found

`crates/vise-core/src/tooling.rs` owns the order, and it is fixed rather than
configurable:

1. The application's own copy, searched under `BUNDLED_SUBDIRECTORIES` beneath
   the executable: `""`, `media`, `bin`, `Resources`, `Resources/media`,
   `libexec`. All of these are tried because the three platforms package
   differently.
2. The machine's `PATH`.
3. A platform-specific fallback (a per-user package directory on Windows, a
   per-user `bin` on macOS).

A file only counts as a tool if it exists, is a file rather than a directory,
and is executable — otherwise a directory called `ffmpeg.exe` would be found and
then fail at the first spawn with a far less useful message.

When the application bundles nothing, `MediaTooling::describe()` says so, so a
development build cannot quietly behave differently from a shipped one. This is
reported through the Group 20 diagnostics bundle.

## Producing a bundled build

```sh
# 1. Fetch the pinned tools into vendor/ffmpeg/<platform>/
./scripts/fetch-media-tools.sh

# 2. Build with bundling on
./scripts/build-release.sh --bundled
```

`scripts/fetch-media-tools.sh` writes `media-tools.manifest.json` next to the
tools it fetched. The manifest is the reproducibility record: it carries the
version, the upstream URL, and the SHA-256 of every binary, so a release can be
checked against what it claims to contain. **`scripts/verify-media-tools.sh`
re-checks that claim on a machine that already has the tools**, which is what a
release job should run before it publishes anything.

The version, URL, and expected digests live in `media-tools.json` at the
repository root. Change it deliberately: a new ffmpeg means new encoder
availability and therefore new test expectations.

## Platform notes

| Platform | Tools land in | Notes |
|---|---|---|
| Windows | beside the executable | The worker is launched from the same directory, so it resolves the same copy. |
| macOS | `Contents/Resources/media` | Inside the `.app`, where the signature covers them. |
| Linux | beside the executable | `libexec` is also searched for distributions that split binaries out. |

A platform key in the pin file names the **tools**, not the machine they run
on. macOS publishes one encoder build, for Intel, and Apple silicon runs it
through Rosetta 2, so both architectures resolve to `darwin-x86_64` rather
than to a `darwin-aarch64` that nothing is pinned under. The three scripts
that name a platform agree on this, and a key the pin file does not hold
stops the fetch with the reason rather than producing a build with no
encoder.

## Why not let `PATH` win

The order is deliberate. A user who needs a particular build can place it in a
portable install's directory, which is searched first. What must not happen is a
build silently picking up a system encoder that lacks a filter the user was
offered, and only discovering it when an encode fails.