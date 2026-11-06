<p align="center">
  <img src="docs/assets/logo.png" alt="FFmpeg Audio Studio" width="110" height="110">
</p>

<h1 align="center">FFmpeg Audio Studio</h1>

<div align="center">
  <strong>Convert audio between 18 formats without losing the tags.</strong><br>
  Drop files in, pick a preset, convert - with the tags, the cover and the language still there.
</div>

<br>

<!-- readme:nav -->

<div align="center">
  <a href="../../releases/latest"><img src="https://img.shields.io/github/v/release/yaiol/ffmpeg-audio-studio?color=5a4fff&label=release&style=flat-square" alt="Release"></a>
  <a href="../../releases"><img src="https://img.shields.io/github/downloads/yaiol/ffmpeg-audio-studio/total?color=5a4fff&label=downloads&style=flat-square" alt="Downloads"></a>
</div>

<h3 align="center">
  <a href="https://apps.yaiol.com/p/ffmpeg-audio-studio/">Website</a>
  <span>&nbsp;·&nbsp;</span>
  <a href="#install">Install</a>
  <span>&nbsp;·&nbsp;</span>
  <a href="#effects">Effects</a>
  <span>&nbsp;·&nbsp;</span>
  <a href="#what-it-is">Features</a>
  <span>&nbsp;·&nbsp;</span>
  <a href="#documentation">Documentation</a>
  <span>&nbsp;·&nbsp;</span>
  <a href="#build-from-source">Development</a>
</h3>

<div align="center">
  <sub><a href="https://apps.yaiol.com/p/ffmpeg-audio-studio/help/"><b>Help in 28 languages</b></a></sub>
</div>

<!-- /readme:nav -->

---

<p align="center">
  <img src="docs/assets/hero.png" alt="FFmpeg Audio Studio with a list of files, the preset, tags and effects choices in the header" width="900">
</p>

---

## Install

<div align="center">

<table align="center">
<tr>
<th align="center">Windows</th>
<th align="center">macOS</th>
<th align="center">Linux</th>
</tr>
<tr>
<td align="center"><a href="../../releases/latest"><img src="https://img.shields.io/badge/Windows-.exe-5a4fff?style=for-the-badge&amp;logo=windows&amp;logoColor=white" alt="Windows"></a></td>
<td align="center"><a href="../../releases/latest"><img src="https://img.shields.io/badge/macOS-.dmg-5a4fff?style=for-the-badge&amp;logo=apple&amp;logoColor=white" alt="macOS"></a></td>
<td align="center"><a href="../../releases/latest"><img src="https://img.shields.io/badge/Linux-.deb%20%7C%20.AppImage-5a4fff?style=for-the-badge&amp;logo=linux&amp;logoColor=white" alt="Linux"></a></td>
</tr>
<tr>
<td align="center">x64 installer</td>
<td align="center">Apple Silicon</td>
<td align="center">deb package or AppImage</td>
</tr>
</table>

</div>

> **Windows note:** SmartScreen may warn on first launch because the app is not code-signed. Click "More info", then "Run anyway".
>
> Windows is the platform the app is developed and tested on; the macOS and Linux packages are built from the same source on release and have had less use.

---

## What it is

FFmpeg Audio Studio is a graphical studio on top of ffmpeg, with two views, chosen by the first two buttons of the header:

- **The work view (the studio)** opens first. One file at a time: its waveform, real playback with your effects applied, a cut set with two buttons, fades you drag, and drawers for the channels, the equaliser, the compressor and the level. You hear what the conversion will make before you make it.
- **The batch view** converts many files at once, in the spirit of fre:ac and MediaHuman: drop files or folders, pick what the output should be, and convert. Several files run at once, each with its own progress and its own settings, and a run can be cancelled per file.

In both views the choices are kept separate: the **preset** (what the output is), the **tags** setting (which tags and cover are kept) and the **effects** (what is done to the sound). One preset can be used with a setting that strips the comments, with another that normalises the volume, or with neither. ffmpeg is not hidden: a panel shows the exact command that will run, and a button copies it.

---

## Effects

Effects are what is done to the sound, and they are the heart of the studio. They are called **Audio Effects** and work in both views.

- **Standard effects as switches.** The work view lists the effects most people need as rows you switch on and off while the sound plays: trim silence, keep a range, limit the length, channels, equaliser, compressor, the four ways to set the volume (a fixed gain, a loudness, a peak or ReplayGain), fades, silence at the end and ReplayGain tags. Some have a drawer of their own, drawn as a graph: the equaliser, the compressor and the channels.
- **Every ffmpeg audio filter, on top.** The **Add an effect** menu offers all of ffmpeg's audio filters, grouped by category and searchable by name, each with ordinary controls generated from the filter itself: times such as 1:30, a picker for colours and files, and a button that puts a value back to its default. Which effects the work view lists ready to switch on is your choice in Settings.
- **You set the order.** Effects run from the top of the list to the bottom as one ffmpeg filter chain; drag a card to change the result. A line warns when an effect counts on something a later one changes, and the app never reorders your list.
- **Saved lists for the batch.** In the batch view an effects entry is a named list kept in Settings and chosen from a drop-down, so one entry goes with any preset.

---

## Features

- **A work view and a batch view** - hear a file with its effects on a waveform, cut it and fade it; or convert a whole folder at once.
- **Eighteen output formats** - MP3, AAC, ALAC, FLAC, WAV, AIFF, Ogg Vorbis, Opus, WMA, WavPack, TTA, MP2, AC-3, Wave64, AU, FLAC in Ogg, iPhone ringtone (M4R) and raw AAC. Anything ffmpeg can decode goes in.
- **Tags that survive the trip** - a field keeps its meaning from format to format under the name a tag editor shows (album artist, sort names, composer, BPM, lyrics, ISRC, custom fields), with repeated fields, track and disc totals and the whole date kept, and the cover picture carried to every format that can hold one.
- **Tag cleaning as a setting** - keep everything except a list, or delete everything except a list, with wildcards; the cover has its own switch.
- **Audio Effects** - the standard effects as switches, every ffmpeg audio filter in a menu, in an order you choose (see above), with ReplayGain tags and album gain.
- **Presets with the encoder's own switches** - one quality setting per format, plus MP3, Vorbis, Opus, FLAC and AAC options, optional dither at 16 bit, and an (Original) choice that keeps each file's own format.
- **Where the files go** - a folder and a file-name pattern (`{artist}/{album}/{track} - {title}`), with the source's folder structure kept, and skip, overwrite or keep-both when a file exists.
- **A command line** - `yac` ships beside the app and uses the same saved presets, tag settings, effects and destinations: `yac -p "MP3 320 kbps" -d Export Music/`. `yac --list` shows what can be named.
- **Your language** - the interface is available in 52 languages.

---

## Documentation

| | |
|---|---|
| **User manual** | [Read it online](https://apps.yaiol.com/p/ffmpeg-audio-studio/help/) |
| **What's new** | [Release notes](https://apps.yaiol.com/p/ffmpeg-audio-studio/help/releases/) |
| **Tag name reference** | [Technical reference](https://apps.yaiol.com/p/ffmpeg-audio-studio/help/technical-reference/) |
| **Product page** | [apps.yaiol.com](https://apps.yaiol.com/p/ffmpeg-audio-studio/) |

---

## Build from source

Requires the Rust toolchain (stable) and, for the installer, the Dioxus CLI (`cargo install dioxus-cli`). The app drives **ffmpeg**, which is not part of the repository and is not bundled: the app downloads the build yaiol has approved into a folder of its own the first time it starts, and never uses an ffmpeg found elsewhere on the machine.

```bash
cargo run -p ffmpeg-audio-studio                                                    # run in development
cargo test                                                      # engine and app tests (the conversion tests need ffmpeg)
cd app && dx bundle --desktop --release --package-types nsis    # Windows installer → release/
```

---

## Architecture

A Cargo workspace of three crates: the **engine** (everything that is not the window - formats, presets, tags, effects, the job queue and the ffmpeg driver, with no UI dependency) the **app** (the Dioxus desktop shell, which polls the engine every 200 ms; the engine never calls back into the window) and the **cli** (`yac`: argument parsing and a printed summary over the same engine and the same saved settings).

| Layer | Technology |
|-------|-----------|
| UI | Dioxus 0.7 on the system WebView |
| Audio engine | ffmpeg, a separate executable the app downloads and installs itself |
| Tag writing | lofty, plus a header rewriter for WMA |
| Interface languages | flat JSON bundles embedded at build time |
| Packaging | `dx bundle` (NSIS, deb, AppImage, dmg) |

<details>
<summary><b>How tags are carried</b></summary>

ffmpeg copies most tags, but it renames some, drops others and flattens repeated fields. After every conversion the engine reads the source again and writes back what ffmpeg lost, through each format's own tag type. One table, `engine/src/mapping.rs`, says what each field is called in every standard (ID3v2.3, ID3v2.4, MP4 atoms, WMA attributes, Vorbis and APE names, WAV INFO ids). A test converts every one of the eighteen formats into every other and checks that each field is still there.

</details>

<details>
<summary><b>How effects are measured</b></summary>

Effects run in a fixed order as one ffmpeg audio-filter chain. At most two measuring passes come first: a scan for silence, and loudness meters taken through the effects that come before the volume, so a normalisation reaches its target without clipping. Album gain measures all files of one album as one programme.

</details>

---

## License

Released under the [MIT License](LICENSE).

The app downloads [ffmpeg](https://ffmpeg.org) as a separate executable from its builders; it is not part of this repository or of the installer and keeps its own licence.

<div align="center">
  <sub>FFmpeg Audio Studio is part of <a href="https://apps.yaiol.com">yaiol Applications</a>.</sub>
</div>
