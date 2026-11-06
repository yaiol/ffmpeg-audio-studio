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

The whole list, from the catalogue that comes with ffmpeg:

<!-- generated:effects:begin -->

<details>
<summary><b>All the Audio Effects</b></summary>

<p><strong>Time and frames</strong> - Frame tricks: time-lapse, smooth motion, padding</p>
<ul><li><strong>Play backwards</strong>: Run the clip from its end to its start, sound included. The whole clip is held in memory.</li></ul>
<p><strong>Volume and dynamics</strong> - Louder, quieter, even out, compress, limit, gate, normalise</p>
<ul><li><strong>Audio contrast</strong>: Make the sound punchier by expanding or compressing its dynamics lightly.</li><li><strong>Compressor</strong>: Everything above a level is lowered by a ratio, so the loud and the quiet parts get closer.</li><li><strong>Compressor / expander</strong>: Compress or expand the dynamics with a transfer curve you set.</li><li><strong>Dynamic equaliser</strong>: An equaliser whose gain follows the level of the sound.</li><li><strong>Dynamic normaliser</strong>: Even out the level over time, quiet and loud parts alike.</li><li><strong>Dynamic smoothing</strong>: Smooth sudden jumps in the sound.</li><li><strong>Limiter</strong>: Stop the sound going above a level without hard clipping.</li><li><strong>Loudness normaliser</strong>: Bring the sound to a standard loudness (EBU R128).</li><li><strong>Multiband compressor</strong>: Compress each frequency band on its own.</li><li><strong>Noise gate</strong>: Silence the sound when it falls below a level.</li><li><strong>Psychoacoustic clipper</strong>: Clip loud peaks in a way the ear notices less.</li><li><strong>ReplayGain: write tags</strong>: Write the track gain and peak of the new file as tags; the sound is untouched.</li><li><strong>Shift the DC level</strong>: Move the whole waveform up or down.</li><li><strong>Soft clipper</strong>: Round off peaks gently instead of cutting them.</li><li><strong>Spectral dynamic range control</strong>: Control the level of each frequency band separately.</li><li><strong>Speech normaliser</strong>: Bring speech to an even level.</li><li><strong>Volume</strong>: Make the sound louder or quieter.</li><li><strong>Volume: apply ReplayGain</strong>: Apply the ReplayGain gain of the track (or of the album) to the sound itself, with a limiter.</li><li><strong>Volume: fixed gain</strong>: Make the sound louder or quieter by a fixed amount, with a limiter.</li><li><strong>Volume: normalise to a loudness</strong>: Bring the loudness of the part that is kept to a target, with a limiter so the gain cannot clip.</li><li><strong>Volume: normalise to a peak</strong>: Bring the highest sample to a target level.</li></ul>
<p><strong>Equaliser and filters</strong> - Bass, treble, keep or cut a range of frequencies</p>
<ul><li><strong>18-band equaliser</strong>: An equaliser with 18 fixed bands.</li><li><strong>All-pass filter</strong>: Change the phase of the sound without changing its loudness.</li><li><strong>Band-pass filter</strong>: Keep a band of frequencies and remove the rest.</li><li><strong>Band-reject filter</strong>: Remove one band of frequencies, for example a hum.</li><li><strong>Bass</strong>: Boost or cut the low frequencies.</li><li><strong>Biquad filter from coefficients</strong>: Apply a two-pole, two-zero filter you give as numbers.</li><li><strong>Boost the bass</strong>: Add subwoofer-range frequencies.</li><li><strong>Cut the sub-bass</strong>: Remove the lowest frequencies.</li><li><strong>Cut very high frequencies</strong>: Remove frequencies above the range of hearing.</li><li><strong>Emphasis (RIAA and others)</strong>: Apply or undo a recording emphasis curve such as RIAA.</li><li><strong>Equaliser (one band)</strong>: Boost or cut one band of frequencies.</li><li><strong>Equaliser (ten bands)</strong>: A gain for each octave band, from 31 Hz to 16 kHz.</li><li><strong>FIR equaliser</strong>: A precise equaliser with a curve you describe.</li><li><strong>Frequency shift</strong>: Move every frequency up or down by a fixed amount.</li><li><strong>Frequency-domain expression filter</strong>: Change the sound with formulas applied to its frequencies.</li><li><strong>High shelf</strong>: Boost or cut everything above a frequency.</li><li><strong>High-order band-pass</strong>: Keep one frequency band with a very steep filter.</li><li><strong>High-order band-stop</strong>: Remove one frequency band with a very steep filter.</li><li><strong>High-pass filter</strong>: Remove the frequencies below a limit: rumble, hum.</li><li><strong>IIR filter from coefficients</strong>: Apply an infinite impulse response filter you give as numbers.</li><li><strong>Low shelf</strong>: Boost or cut everything below a frequency.</li><li><strong>Low-pass filter</strong>: Remove the frequencies above a limit: hiss.</li><li><strong>Parametric equaliser</strong>: Many-band equaliser with its own curve per channel.</li><li><strong>Phase shift</strong>: Shift the phase of all frequencies.</li><li><strong>Sound derivative</strong>: Compute the rate of change of the sound, a signal-processing tool.</li><li><strong>Sound from an expression</strong>: Rewrite each sample of the sound with a formula.</li><li><strong>Sound integral</strong>: Compute the running sum of the sound, a signal-processing tool.</li><li><strong>Spectral tilt</strong>: Tilt the balance between low and high frequencies.</li><li><strong>Tilt shelf</strong>: A shelf filter that tilts the whole frequency balance.</li><li><strong>Treble</strong>: Boost or cut the high frequencies.</li></ul>
<p><strong>Noise and repair</strong> - Hiss, clicks, clipping, hum, sibilance</p>
<ul><li><strong>De-esser</strong>: Soften harsh s sounds in speech.</li><li><strong>Decode HDCD</strong>: Decode High Definition Compatible Digital discs.</li><li><strong>Denoise (FFT)</strong>: Reduce steady noise such as hiss using frequency analysis.</li><li><strong>Denoise (non-local means)</strong>: Reduce broadband noise with a patch-matching method.</li><li><strong>Denoise (wavelets)</strong>: Reduce noise using wavelets.</li><li><strong>Denoise speech (neural network)</strong>: Reduce noise in speech with a trained neural network.</li><li><strong>Remove clicks</strong>: Take out short clicks and pops, as from old records.</li><li><strong>Repair clipping</strong>: Restore peaks that were cut off by too high a level.</li></ul>
<p><strong>Time and pitch</strong> - Fade, speed, pitch, cut, loop, silence</p>
<ul><li><strong>Change the rate without resampling</strong>: Play the same samples at another rate: pitch and speed change together.</li><li><strong>Change the speed, keep the pitch</strong>: Make the sound faster or slower without changing its pitch.</li><li><strong>Cut the sound</strong>: Keep one section of the sound and drop the rest.</li><li><strong>Delay channels</strong>: Delay one or more channels by a number of milliseconds.</li><li><strong>Distance delay</strong>: Delay the sound as if it travelled a distance, to line up speakers.</li><li><strong>Fade</strong>: Fade the sound in from silence at the start of the part that is kept, and out to silence at its end. Each length is in seconds; 0 leaves that end as it is.</li><li><strong>Fade in or out</strong>: Fade the sound in or out over a chosen time.</li><li><strong>Keep a range</strong>: Keep only the part between two times, counted on what the effects above it left.</li><li><strong>Limit the duration</strong>: Keep at most this many seconds from the start.</li><li><strong>Loop the sound</strong>: Repeat a section of the sound.</li><li><strong>Pad with silence</strong>: Add silence at the end, for a fixed length or a number of samples.</li><li><strong>Remove silence</strong>: Cut out silent stretches.</li><li><strong>Resample</strong>: Change the sample rate of the sound.</li><li><strong>Silence at the end</strong>: Cut the song where its trailing silence begins and leave exactly this much silence after it, so an LP plays as separate tracks.</li><li><strong>Time-stretch and pitch-shift</strong>: Change the speed and the pitch independently, with high quality.</li><li><strong>Trim silence at the ends</strong>: Cut the silence before the sound starts and after it stops - never silence inside the song.</li></ul>
<p><strong>Stereo and channels</strong> - Widen, pan, mix channels, headphones, surround</p>
<ul><li><strong>Channels</strong>: Swap left and right, play one channel on both sides, mix both into one, or mix any layout down to stereo.</li><li><strong>Decorrelate channels</strong>: Make the channels less alike, which widens the sound.</li><li><strong>Dialogue enhancer</strong>: Make speech stand out in a film mix.</li><li><strong>Extra stereo</strong>: Increase the difference between left and right.</li><li><strong>Haas stereo enhancer</strong>: Widen the sound with a tiny delay between the sides.</li><li><strong>Headphone crossfeed</strong>: Mix a little of each side into the other to ease headphone listening.</li><li><strong>Headphone stereo widening</strong>: Make the stereo image wider, tuned for headphones.</li><li><strong>Mix channels</strong>: Remix channels with your own coefficients: stereo to mono, swap sides.</li><li><strong>Remap channels</strong>: Move channels to other positions.</li><li><strong>Stereo tools</strong>: Balance, mid and side levels, mute, phase and more in one filter.</li><li><strong>Upmix to surround</strong>: Spread a stereo sound over a surround layout.</li><li><strong>Widen the stereo</strong>: Make the stereo image wider with a delayed feedback.</li></ul>
<p><strong>Sound effects</strong> - Echo, chorus, phaser, tremolo, distortion</p>
<ul><li><strong>Bit crusher</strong>: Reduce the sound's bit depth for a lo-fi, crunchy effect.</li><li><strong>Chorus</strong>: Thicken the sound with slightly detuned copies.</li><li><strong>Crystalizer</strong>: Sharpen the sound by exaggerating its changes.</li><li><strong>Echo</strong>: Add one or more echoes to the sound.</li><li><strong>Exciter</strong>: Add brightness by enhancing the high frequencies.</li><li><strong>Flanger</strong>: Add a sweeping, jet-like effect.</li><li><strong>Phaser</strong>: Add a sweeping phasing effect.</li><li><strong>Pulsator</strong>: Make the level pulse, with the stereo sides taking turns.</li><li><strong>Tremolo</strong>: Make the level wobble.</li><li><strong>Vibrato</strong>: Make the pitch wobble.</li><li><strong>Virtual bass</strong>: Make small speakers seem to play deeper notes.</li></ul>

</details>

<!-- generated:effects:end -->

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
