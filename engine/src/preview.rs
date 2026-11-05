//! The sound preview of the work view: the file as a waveform, and the sound of it with the work view's effects on it.
//!
//! What is heard is what a conversion would make: `prepare` runs the same measuring passes and the same `effects::plan` a job runs
//! (the album's gain is the one thing it leaves out: a preview has no album), and `Sound` plays the chain it made. The device pulls
//! the samples at its own pace, so the sound never runs ahead of itself; where the playing has got to is counted from the samples
//! the device took.

use crate::{
    effects::{self, Analysis, EffectsEntry, Window},
    ffmpeg::command,
};
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, SampleRate, Source};
use std::{
    ffi::OsString,
    io::Read,
    path::Path,
    process::{Child, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering},
        mpsc::{Receiver, RecvTimeoutError, sync_channel},
    },
    thread,
    time::Duration,
};

/// Samples decoded and not yet played, a few seconds at most: ffmpeg waits when the device is behind.
const QUEUED_CHUNKS: usize = 24;
/// The waveform is read from a mono copy at this rate: enough for a picture, a fraction of the work of the file's own.
const WAVE_RATE: u32 = 4000;

/// A time as a person reads it on a player: `m:ss.mmm`, with hours when there are some.
pub fn clock(seconds: f64) -> String {
    let ms = (seconds.max(0.0) * 1000.0).round() as u64;
    let (h, m, s, milli) = (ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000);
    if h > 0 { format!("{h}:{m:02}:{s:02}.{milli:03}") } else { format!("{m}:{s:02}.{milli:03}") }
}

/// What the effects make of one file: which part of it is kept, and the filters that play it.
#[derive(Clone, Debug, PartialEq)]
pub struct Prepared {
    /// The source's length.
    pub duration_s: f64,
    /// The part of the source that is kept, and the silence appended after it.
    pub window: Window,
    /// The `-af` chain (the cut of the window is its first filter). `None` = the sound is played as it is.
    pub filters: Option<String>,
    /// How many times faster the effects play the kept part.
    pub ratio: f64,
    /// Where a time written in the range effect counts from: the start of what the effects above the range leave.
    pub origin: f64,
    /// One line per effect left out of the sound, each naming the effect and why: an effect ffmpeg refuses must not take the whole preview with it.
    pub skipped: Vec<String>,
}

impl Prepared {
    /// The point of the output a source time falls on, in seconds from the start of the output (0 before the kept part).
    pub fn output_at(&self, source_s: f64) -> f64 {
        (source_s - self.window.start).max(0.0) / self.ratio.max(1e-9)
    }

    /// The number the range effect takes for a source time.
    pub fn range_time(&self, source_s: f64) -> f64 {
        (source_s - self.origin).max(0.0)
    }

    /// The source time an output time stands for.
    pub fn source_at(&self, output_s: f64) -> f64 {
        (self.window.start + output_s * self.ratio).min(self.window.end)
    }
}

/// Why a pass or a playback could not be made, and the filter ffmpeg blamed when it named one.
struct Failure {
    why: String,
    filter: Option<String>,
}

impl From<String> for Failure {
    fn from(why: String) -> Failure {
        Failure { why, filter: None }
    }
}

/// What went wrong, from what ffmpeg said: the line that names the cause, not the closing lines ("Error opening output files") that every
/// failure ends with, and without the filter's own tag.
fn reason(text: &str) -> String {
    let closing = |l: &str| {
        l.starts_with("Error opening output") || l.starts_with("Error initializing filter") || l.starts_with("Error reinitializing filter") || l.starts_with("Conversion failed")
    };
    let line = text.lines().map(str::trim).find(|l| !l.is_empty() && !closing(l)).unwrap_or("the sound could not be made");
    // the filter's own tag ("[Parsed_aphaser_2 @ 000001bd6970df80]") says nothing to a reader
    match line.strip_prefix('[').and_then(|r| r.split_once("] ")) {
        Some((_, rest)) => rest.to_owned(),
        None => line.to_owned(),
    }
}

/// The filter ffmpeg names in its complaint (`[Parsed_aphaser_1 @ 0000` is `aphaser`).
fn blamed(text: &str) -> Option<String> {
    let tag = text.split("[Parsed_").nth(1)?.split(" @").next()?;
    Some(tag.rsplit_once('_')?.0.to_owned())
}

/// One measuring pass: ffmpeg runs `filter` over the file and what it printed is returned.
fn pass(ffmpeg: &Path, input: &Path, filter: &str) -> Result<String, Failure> {
    let out = command(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-nostats", "-loglevel", "info", "-i"])
        .arg(input)
        .args(["-map", "0:a:0", "-vn", "-af", filter, "-f", "null", "-"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run ffmpeg: {e}"))?;
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    if out.status.success() {
        Ok(text)
    } else {
        let why = reason(&text);
        crate::log::error("preview measuring", &format!("{why}
input: {}
filter: {filter}
what ffmpeg said:
{}", input.display(), text.trim()));
        Err(Failure { why, filter: blamed(&text) })
    }
}

/// The first moments of the sound through `filters`: what a playback would meet at once, found out before the device is opened.
fn try_chain(ffmpeg: &Path, input: &Path, filters: &str) -> Result<(), Failure> {
    let out = command(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-nostats", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-map", "0:a:0", "-vn", "-af", filters, "-t", "0.5", "-f", "null", "-"])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("cannot run ffmpeg: {e}"))?;
    if out.status.success() {
        return Ok(());
    }
    let text = String::from_utf8_lossy(&out.stderr).into_owned();
    let why = reason(&text);
    crate::log::error("preview playback", &format!("{why}
input: {}
filter: {filters}
what ffmpeg said:
{}", input.display(), text.trim()));
    Err(Failure { why, filter: blamed(&text) })
}

/// The measuring a job would do for `entry`, and the plan it makes of the result. Blocking: a file that needs its silence found or its
/// loudness measured takes a moment. An effect that ffmpeg refuses is left out of the sound and said in `skipped`, so the rest still plays.
pub fn prepare(ffmpeg: &Path, input: &Path, duration_s: f64, entry: &EffectsEntry) -> Result<Prepared, String> {
    let mut entry = entry.clone().normalized();
    entry.check()?;
    let mut skipped: Vec<String> = Vec::new();
    loop {
        match attempt(ffmpeg, input, duration_s, &entry) {
            Ok(mut p) => {
                p.skipped = skipped;
                return Ok(p);
            }
            Err(f) => {
                let culprit = f.filter.as_deref().and_then(|name| entry.items.iter().position(|i| i.uses_filter(name)));
                match culprit {
                    Some(at) if skipped.len() < 4 => {
                        skipped.push(format!("{}: {}", entry.items[at].title(), f.why));
                        entry.items.remove(at);
                    }
                    _ => return Err(f.why),
                }
            }
        }
    }
}

fn attempt(ffmpeg: &Path, input: &Path, duration_s: f64, entry: &EffectsEntry) -> Result<Prepared, Failure> {
    let a = measure(ffmpeg, input, duration_s, entry)?;
    let plan = effects::plan(entry, &a, None);
    let filters = plan.af();
    if let Some(af) = &filters {
        try_chain(ffmpeg, input, af)?;
    }
    Ok(Prepared { duration_s, window: effects::window(entry, &a), filters, ratio: entry.tempo_ratio(), origin: effects::window(&entry.without_cuts(), &a).start, skipped: Vec::new() })
}

/// What the effects of `entry` need measured of the file (nothing for most), and the plan made of it.
pub fn plan_for(ffmpeg: &Path, input: &Path, duration_s: f64, entry: &EffectsEntry) -> Result<effects::EffectsPlan, String> {
    Ok(effects::plan(entry, &measure(ffmpeg, input, duration_s, entry).map_err(|f| f.why)?, None))
}

fn measure(ffmpeg: &Path, input: &Path, duration_s: f64, entry: &EffectsEntry) -> Result<Analysis, Failure> {
    let mut a = Analysis { duration_s, ..Analysis::default() };
    if entry.needs_scan() {
        a = effects::parse_analysis(&pass(ffmpeg, input, &effects::scan_filter(entry.scan_threshold_db()))?, duration_s);
    }
    if entry.needs_meter() {
        let w = effects::window(&entry, &a);
        let m = effects::parse_analysis(&pass(ffmpeg, input, &effects::meter_filter(&entry, &w, duration_s))?, w.length() / entry.tempo_ratio());
        a.loudness_lufs = m.loudness_lufs;
        a.true_peak_db = m.true_peak_db;
        a.max_volume_db = m.max_volume_db;
        a.rg_gain_db = m.rg_gain_db;
        a.rg_peak = m.rg_peak;
    }
    Ok(a)
}

/// The loudness of the file along its length as `buckets` values from 0 to 1 (each the highest sample of its part, the whole scaled so
/// the loudest part reaches 1). `duration_s` is the file's length.
pub fn waveform(ffmpeg: &Path, input: &Path, duration_s: f64, buckets: usize) -> Result<Vec<f32>, String> {
    if duration_s <= 0.0 || buckets == 0 {
        return Err("the file has no length".to_owned());
    }
    let mut child = command(ffmpeg)
        .args(["-hide_banner", "-nostdin", "-loglevel", "error", "-i"])
        .arg(input)
        .args(["-map", "0:a:0", "-vn", "-ac", "1", "-ar", &WAVE_RATE.to_string(), "-f", "f32le", "-"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("cannot run ffmpeg: {e}"))?;
    let mut out = child.stdout.take().ok_or("no output pipe")?;
    let total = duration_s * f64::from(WAVE_RATE);
    let mut peaks = vec![0.0_f32; buckets];
    let (mut buf, mut loose, mut at) = (vec![0u8; 64 * 1024], Vec::<u8>::new(), 0_u64);
    while let Ok(n) = out.read(&mut buf) {
        if n == 0 {
            break;
        }
        loose.extend_from_slice(&buf[..n]);
        let whole = loose.len() / 4 * 4;
        for b in loose[..whole].chunks_exact(4) {
            let v = f32::from_le_bytes([b[0], b[1], b[2], b[3]]).abs();
            let i = ((at as f64 / total * buckets as f64) as usize).min(buckets - 1);
            if v > peaks[i] {
                peaks[i] = v;
            }
            at += 1;
        }
        loose.drain(..whole);
    }
    let _ = child.wait();
    let top = peaks.iter().copied().fold(0.0_f32, f32::max);
    if at == 0 {
        return Err("the file has no sound to show".to_owned());
    }
    Ok(peaks.into_iter().map(|p| if top > 0.0 { p / top } else { 0.0 }).collect())
}

/// The samples ffmpeg made, read by the device; the volume is read at every sample so a change is heard at once.
struct Pcm {
    chunks: Receiver<Vec<f32>>,
    now: std::vec::IntoIter<f32>,
    channels: ChannelCount,
    rate: SampleRate,
    volume: Arc<AtomicU32>,
    played: Arc<AtomicU64>,
    done: Arc<AtomicBool>,
}

impl Iterator for Pcm {
    type Item = f32;

    fn next(&mut self) -> Option<f32> {
        loop {
            if let Some(s) = self.now.next() {
                self.played.fetch_add(1, Ordering::Relaxed);
                return Some(s * f32::from_bits(self.volume.load(Ordering::Relaxed)));
            }
            match self.chunks.recv_timeout(Duration::from_millis(20)) {
                Ok(next) => self.now = next.into_iter(),
                // ffmpeg is slower than the device for a moment: silence, not a stop (and not counted as played).
                Err(RecvTimeoutError::Timeout) => return Some(0.0),
                Err(RecvTimeoutError::Disconnected) => {
                    self.done.store(true, Ordering::Relaxed);
                    return None;
                }
            }
        }
    }
}

impl Source for Pcm {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        self.channels
    }
    fn sample_rate(&self) -> SampleRate {
        self.rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

/// A sound playing; stopped when dropped.
pub struct Sound {
    child: Child,
    volume: Arc<AtomicU32>,
    played: Arc<AtomicU64>,
    done: Arc<AtomicBool>,
    /// Samples a second, all channels together.
    per_second: f64,
    _device: MixerDeviceSink,
}

impl Sound {
    /// The file from the source time `at`, with the effects of `prepared`, at `volume` (0 is silent, 1 is the file's own level).
    pub fn start(ffmpeg: &Path, input: &Path, prepared: &Prepared, at: f64, volume: f32) -> Result<Sound, String> {
        let mut device = DeviceSinkBuilder::open_default_sink().map_err(|e| format!("no sound output: {e}"))?;
        device.log_on_drop(false);
        let (channels, rate) = (device.config().channel_count(), device.config().sample_rate());
        let mut args: Vec<OsString> = vec!["-hide_banner".into(), "-nostdin".into(), "-loglevel".into(), "error".into(), "-i".into(), input.into()];
        args.extend(["-map", "0:a:0", "-vn"].map(OsString::from));
        if let Some(af) = &prepared.filters {
            args.extend(["-af".into(), af.into()]);
        }
        // The start is an OUTPUT seek, so the fades and the gain are those of the whole kept part, as in the conversion.
        let skip = prepared.output_at(at);
        if skip > 0.0 {
            args.extend(["-ss".to_owned(), format!("{skip:.6}")].map(OsString::from));
        }
        args.extend(["-f".to_owned(), "f32le".to_owned(), "-ac".to_owned(), channels.get().to_string(), "-ar".to_owned(), rate.get().to_string(), "-".to_owned()].map(OsString::from));
        let mut child = command(ffmpeg).args(&args).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn().map_err(|e| {
            let why = format!("cannot run ffmpeg: {e}");
            crate::log::error("preview playback", &format!("{why}\ninput: {}", input.display()));
            why
        })?;
        let mut out = child.stdout.take().ok_or("no output pipe")?;
        let (tx, chunks) = sync_channel::<Vec<f32>>(QUEUED_CHUNKS);
        thread::spawn(move || {
            // 4-byte samples can be split across two reads: the loose bytes wait for the next one.
            let (mut buf, mut loose) = (vec![0u8; 16 * 1024], Vec::<u8>::new());
            while let Ok(n) = out.read(&mut buf) {
                if n == 0 {
                    break;
                }
                loose.extend_from_slice(&buf[..n]);
                let whole = loose.len() / 4 * 4;
                let samples: Vec<f32> = loose[..whole].chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
                loose.drain(..whole);
                if tx.send(samples).is_err() {
                    break;
                }
            }
        });
        let volume = Arc::new(AtomicU32::new(volume.max(0.0).to_bits()));
        let (played, done) = (Arc::new(AtomicU64::new(0)), Arc::new(AtomicBool::new(false)));
        device.mixer().add(Pcm { chunks, now: Vec::new().into_iter(), channels, rate, volume: volume.clone(), played: played.clone(), done: done.clone() });
        Ok(Sound { child, volume, played, done, per_second: f64::from(channels.get()) * f64::from(rate.get()), _device: device })
    }

    pub fn set_volume(&self, volume: f32) {
        self.volume.store(volume.max(0.0).to_bits(), Ordering::Relaxed);
    }

    /// How much of the output the device has played, in seconds.
    pub fn played_s(&self) -> f64 {
        self.played.load(Ordering::Relaxed) as f64 / self.per_second
    }

    /// Everything has been played.
    pub fn finished(&self) -> bool {
        self.done.load(Ordering::Relaxed)
    }
}

impl Drop for Sound {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clock_reads_minutes_seconds_and_thousandths() {
        assert_eq!(clock(0.0), "0:00.000");
        assert_eq!(clock(75.25), "1:15.250");
        assert_eq!(clock(3725.5), "1:02:05.500");
    }

    #[test]
    fn a_source_time_and_an_output_time_map_through_the_kept_part_and_the_tempo() {
        let p = Prepared { duration_s: 100.0, window: Window { start: 10.0, end: 50.0, pad: 0.0 }, filters: None, ratio: 2.0, origin: 4.0, skipped: Vec::new() };
        assert_eq!(p.output_at(5.0), 0.0, "before the kept part the output starts at its start");
        assert_eq!(p.output_at(30.0), 10.0, "twenty seconds into the kept part at double speed is ten of output");
        assert_eq!(p.source_at(10.0), 30.0);
        assert_eq!(p.range_time(10.0), 6.0, "a range counts from where the effects above it start");
        assert_eq!(p.range_time(1.0), 0.0);
        assert_eq!(p.source_at(1000.0), 50.0, "never past the end of the kept part");
    }

    #[test]
    fn a_waveform_follows_the_loudness_along_the_file() {
        let Some(ff) = crate::ffmpeg::ffmpeg_for_tests() else { return };
        let dir = std::env::temp_dir().join(format!("yac-wave-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("half.wav");
        // one second of tone, then one second of silence
        let made = command(&ff)
            .args(["-y", "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "sine=f=440:d=1", "-f", "lavfi", "-i", "anullsrc=r=44100:cl=mono:d=1", "-filter_complex", "[0][1]concat=n=2:v=0:a=1"])
            .arg(&file)
            .status()
            .unwrap();
        assert!(made.success());
        let w = waveform(&ff, &file, 2.0, 20).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(w.len(), 20);
        assert!(w[..8].iter().all(|v| *v > 0.5), "the tone shows: {w:?}");
        assert!(w[12..].iter().all(|v| *v < 0.05), "the silence shows: {w:?}");
    }

    #[test]
    fn the_reason_is_the_line_that_names_the_cause_not_the_closing_one() {
        let said = "[Parsed_aphaser_1 @ 000001bd6970df80] Option 'decay' out of range\nError initializing filters\nError opening output file -.\nError opening output files: Invalid argument";
        assert_eq!(reason(said), "Option 'decay' out of range");
        assert_eq!(blamed(said).as_deref(), Some("aphaser"));
        assert_eq!(reason("Error opening output files: Invalid argument\n"), "the sound could not be made");
        assert_eq!(blamed("No such filter: 'zzz'"), None);
    }
}
