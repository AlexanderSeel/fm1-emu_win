# Windows x64 VST3 build

This fork adds a Windows build path for the existing Rust CLAP FM-1 firmware emulator and its clap-wrapper VST3 adapter.

## Download from GitHub Actions

Open [Windows VST3 workflow](../../actions/workflows/windows-vst3.yml), run it on `master` (or open its latest successful run), and download artifact **FM-1-Windows-x64-VST3**. This is a build artifact, not yet an officially validated release.

## Local build

Requires Windows x64, Rust MSVC toolchain, Git, CMake and Visual Studio 2022 C++ build tools.

```powershell
git clone https://github.com/AlexanderSeel/fm1-emu_win.git
cd fm1-emu_win
./tools/build_plugin_windows.ps1 -NoInstall
```

Output: `target/plugin/windows-x64/FM-1.vst3` and `FM-1.clap`. Both must stay together for VST3 operation because clap-wrapper loads the CLAP dynamically.

To install automatically (requires write access to the system VST3 folder):

```powershell
./tools/build_plugin_windows.ps1
```

For manual installation, copy **both** files to `C:\Program Files\Common Files\VST3\`, restart and rescan your DAW, and add **FM-1** as an instrument. Import your own `.fwsc` using the editor.

## Status and testing

The build path and artifact publication have been added. A successful CI compile is necessary but not sufficient to establish correct loading or audio. The Windows VST3 must still be tested in a host (for example REAPER) for editor rendering, firmware boot, MIDI, output audio, save/restore, and multiple instances. Firmware is not included in distributed artifacts. Do not confuse this emulator with sound-engine-only recreations.

## Troubleshooting

- No Rust: install rustup with the `x86_64-pc-windows-msvc` toolchain.
- CMake compiler missing: install the Visual Studio 2022 **Desktop development with C++** workload.
- Plugin not discovered: confirm `FM-1.vst3` is in the VST3 directory and rescan in your DAW.
- Plugin opens but doesn't instantiate: check that `FM-1.clap` exists beside `FM-1.vst3` and use a 64-bit host.
- Heavy CPU usage: the emulator simulates the original processor; start with one plugin instance.
