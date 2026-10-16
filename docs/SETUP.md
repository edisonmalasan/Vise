# Local Setup

## 1. Install Rust

Install rustup with the MSVC toolchain:

```powershell
winget install Rustlang.Rustup --silent --accept-package-agreements --accept-source-agreements
```

Ensure Cargo is on PATH, or run in the installed shell:

```powershell
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
cargo --version
```

## 2. Install FFmpeg

```powershell
winget install Gyan.FFmpeg --silent --accept-package-agreements --accept-source-agreements
```

If needed, add the WinGet package `bin` directory to PATH:

```powershell
$ffdir = "C:\Users\Edison\AppData\Local\Microsoft\WinGet\Packages\Gyan.FFmpeg_Microsoft.Winget.Source_8wekyb3d8bbwe\ffmpeg-9.0.2-full_build\bin"
$env:PATH = "$ffdir;$env:PATH"
ffmpeg -version
ffprobe -version
```

## 3. Install Visual Studio C++ Build Tools

```powershell
winget install Microsoft.VisualStudio.2022.BuildTools --silent --accept-package-agreements --accept-source-agreements --override "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"
```

## 4. Build and test

```powershell
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## 5. Manual smoke test

```powershell
ffmpeg -y -f lavfi -i testsrc=duration=2:size=320x240:rate=30 -f lavfi -i sine=frequency=1000:duration=2 -shortest "$env:TEMP\in.mp4"
cargo run -p vise-cli -- transcode --input "$env:TEMP\in.mp4" --output "$env:TEMP\out.mp4"
ffprobe -v error -show_entries stream=codec_name -of default=noprint_wrappers=1 "$env:TEMP\out.mp4"
```