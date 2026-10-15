# FM-1 as a plugin (AU, VST3, CLAP)

The emulator as an instrument in a DAW: load a firmware package (`.fwsc`) in the plugin's
window, play it from the host's MIDI, and play its panel with the mouse. It's the same panel as
on the page, so an SMK-37 package gets the SMK-37's.

| Format | For | Built by |
|---|---|---|
| AU (v2) | Logic, GarageBand, and other Mac hosts | clap-wrapper |
| VST3 | Ableton Live, Cubase, FL Studio, Reaper, Bitwig, Studio One… | clap-wrapper |
| CLAP | Bitwig, Reaper, FL Studio, Studio One… | nih-plug (Rust) |

One plugin is written in Rust as a CLAP with [nih-plug](https://github.com/robbert-vdh/nih-plug),
built without its VST3 bindings (they are GPL). [clap-wrapper](https://github.com/free-audio/clap-wrapper)
makes the AU and the VST3 of it, each carrying the CLAP in its bundle. Every part is
permissively licensed:

| Part | Licence |
|---|---|
| this plugin | MIT |
| nih-plug | ISC |
| clap-wrapper and the CLAP SDK | MIT |
| Apple's AudioUnitSDK | Apache-2.0 |
| Steinberg's VST3 SDK (3.8 and later) | MIT |

## Build and install (macOS)

```sh
tools/build_plugin.sh
```

You need Rust, CMake and Ninja (`brew install cmake ninja`). The script builds the CLAP, then the
AU and the VST3 (clap-wrapper downloads the SDKs it needs on the first run). It signs them ad hoc,
installs them in `~/Library/Audio/Plug-Ins/{Components,VST3,CLAP}` and runs Apple's AU validator
(`auval -v aumu Fm1s Fm1e`). Add `NO_INSTALL=1` to only build them, into `target/plugin/`.

A host that is already running doesn't see the new build: GarageBand and Logic look for plugins
when they start, so quit them (⌘Q) and open them again.

**Steinberg's validator and test host** are built from the VST3 SDK that clap-wrapper downloaded.
The SDK wants a full Xcode, so `XCODE_VERSION` stands in for one when only the Command Line Tools
are installed:

```sh
cd target/plugin
XCODE_VERSION=16.0 cmake -S build/cpm/vst3sdk -B vst3sdk-build -G Ninja -DXCODE_VERSION=16.0 \
  -DCMAKE_BUILD_TYPE=Release -DSMTG_ENABLE_VST3_PLUGIN_EXAMPLES=OFF -DSMTG_ENABLE_VSTGUI_SUPPORT=OFF
cmake --build vst3sdk-build --target validator editorhost
vst3sdk-build/bin/Release/validator ~/Library/Audio/Plug-Ins/VST3/FM-1.vst3   # 47 tests pass
open vst3sdk-build/bin/Release/editorhost.app --args ~/Library/Audio/Plug-Ins/VST3/FM-1.vst3
```

An ad hoc signature is enough on the Mac that built the plugin. To give it to others, it has to be
signed with an Apple Developer ID and notarized.

## Using it

- **Load firmware…** picks a `.fwsc`: a community firmware (the page's list links each author's
  package) or M-VAVE's own. The package is saved in the host's project. So is every flash sector
  the firmware writes, so your patches come back when the project reopens.
- **MIDI** from the host goes to the MIDI IN jack. **MIDI channel** moves channel messages to one
  channel; M-VAVE's own firmware listens on 2.
- **Master** is the panel's MASTER pot, and a host parameter you can automate.
- **The panel** works as on the page:
  - a key or button is held while pressed, and a drag across the keys plays a glissando;
  - a knob turns when you drag it up or down, or scroll over it;
  - on the SMK-37 the pads, faders and wheels work too.
- **The status bar** shows the emulation's speed against real time, and counts the frames that
  came late when the computer couldn't keep up.

## How it runs

The firmware makes its audio in DMA halves, and how long a half takes to emulate varies. So the
machine runs on a thread of its own, a little ahead of the host:
- **Frames:** it keeps a ring of its 44.1 kHz frames about one host block plus 1024 frames ahead
  of what the host has taken.
- **MIDI timing:** host MIDI arrives stamped with the frame it belongs to, and the machine takes
  it when it reaches that frame. A note sounds where the host placed it, and the delay is a
  constant the plugin reports to the host as its latency, so the host compensates for it.
- **Sample rate:** the output is resampled to the host's rate.

**A host that takes no audio:** some hosts show the window without processing audio, such as
Steinberg's test host, or a track that isn't playing. If the host hasn't taken any frames for
50 ms, the machine runs on in real time, as the device would. Its audio is dropped and not
counted, so the host's stamps still line up when it takes frames again.

**Load:** one instance takes about one CPU core. The slowest firmwares run at 1.4–1.7x real time
on an Apple Silicon Mac (most at 3–6x), so keep two or three heavy instances per project.

`cargo test --release` (here) runs a firmware on the machine's thread. It checks that a stamped
note sounds on time, and that the firmware boots and draws its screen when nothing takes its
audio. It uses `reference/firmwares/felucca.fwsc` (`node tools/fetch_firmwares.mjs`);
`FM1_TEST_FW=<id>` picks another firmware for the second test.

## Tested

| Check | Result |
|---|---|
| Apple's AU validator (`auval`) | passes: rendering at 11.025–192 kHz, latency, state, the window |
| Steinberg's VST3 validator | 47 of 47 tests pass |
| GarageBand | AU: loads a firmware and runs; the window's redraw on reopening is fixed but not yet seen there |
| Steinberg's editorhost | VST3: the window draws, firmwares boot and react to the panel |
| Logic, Ableton, REAPER, Bitwig | not yet tried |
| Windows, Linux | not built yet: nih-plug and clap-wrapper support them, but `tools/build_plugin.sh` is macOS-only |
