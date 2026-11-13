//! The preview: the selected file as a waveform with the work view's effects on its sound, a timeline to move along, and the
//! buttons to step, play and mark where the kept range starts and ends. ONE pane (`PreviewPane`), in the work view above the
//! file list.
//!
//! A pane is told what to show through a signal (`State`: the file and the effects) and says what the user marked through
//! `on_mark`. The work view writes the marks into the "keep a range" effect of its list, so the marks and the effect's boxes
//! are one setting.
//!
//! The sound is made by ffmpeg from the same plan as a conversion (`engine::preview`): the measuring passes a volume or a cut of
//! silence needs run first (the pane says "Measuring the sound..."), then the chain plays on the default output. The waveform
//! is the source's, so the timeline stays the source's time whatever the effects keep.
//!
//! ⚠ CLAUDE: NO HARDCODED UI STRINGS. Every user-visible string goes through the t() lookup with the key
//! a string literal at the call site.

use crate::{
    catalog::{UisFldLabelled, UisNum, UisWaveform},
    effects_editor::SYSTEM,
    i18n,
    preview_session::{Drawer, Mark, State, WidgetEdit},
    runner::RUNNER,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::ld_icons::{LdChevronLeft, LdChevronRight, LdChevronsDown, LdChevronsUp, LdActivity, LdArrowLeftRight, LdAudioLines, LdList, LdPause, LdPlay, LdSlidersVertical, LdVolume2, LdVolumeX, LdX},
};
use engine::{
    effects,
    ffmpeg::Probe,
    ffmpeg_effects::{V, Values},
    preview::{self, Prepared, Sound},
};
use std::{rc::Rc, time::Duration};

/// The fade widget (the fade in and the fade out, drawn as two handles under the waveform), when the catalogue has it.
fn fade_widget() -> Option<effects::WidgetSpec> {
    effects::catalog().and_then(|c| effects::widgets(&c, SYSTEM).into_iter().find(|w| w.kind == "fades"))
}

/// The values of the fade effect's controls now (their defaults while the effect is not in the list).
fn fade_values(w: &effects::WidgetSpec, list: &effects::EffectsEntry) -> Values {
    list.items.iter().find(|i| i.key == w.key).cloned().unwrap_or_else(|| effects::Item::new(&w.key)).all_values()
}

/// The two lengths in seconds, the fade in and the fade out.
fn fades_of(w: &effects::WidgetSpec, values: &Values) -> (f64, f64) {
    let length = |end: &str| match values.get(w.controls.get(end).map_or("", String::as_str)) {
        Some(V::Num(n)) => n.max(0.0),
        _ => 0.0,
    };
    (length("in"), length("out"))
}

/// The values of the effect with these two lengths.
fn with_fades(w: &effects::WidgetSpec, mut values: Values, (fade_in, fade_out): (f64, f64)) -> Values {
    for (end, v) in [("in", fade_in), ("out", fade_out)] {
        values.insert(w.controls.get(end).cloned().unwrap_or_default(), V::Num(v));
    }
    values
}

/// Which end of the kept part a fade handle belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FadeEnd {
    In,
    Out,
}

/// A fade handle being dragged: which one, and where the bar is on screen (its left edge and width).
#[derive(Clone, Copy, Debug)]
struct FadeDrag {
    end: FadeEnd,
    left: f64,
    width: f64,
}

/// The longest fade the control takes, in seconds.
const MAX_FADE: f64 = 60.0;

/// The two lengths after the pointer moved to `x`, on a bar that stands for `length` seconds with the kept part `kept` (start, end) on it and the
/// lengths `now`: the handle moved sets its own length in tenths of a second, never past the kept part or into the other fade.
fn dragged_fades(d: &FadeDrag, x: f64, length: f64, kept: (f64, f64), now: (f64, f64)) -> (f64, f64) {
    let at = ((x - d.left) / d.width * length).clamp(0.0, length);
    let room = (kept.1 - kept.0).max(0.0);
    let tenth = |v: f64| ((v * 10.0).round() / 10.0).min(MAX_FADE);
    match d.end {
        FadeEnd::In => (tenth((at - kept.0).clamp(0.0, (room - now.1).max(0.0))), now.1),
        FadeEnd::Out => (now.0, tenth((kept.1 - at).clamp(0.0, (room - now.0).max(0.0)))),
    }
}

/// The fade handle is let go; the lengths stay as dragged until the work view's list has followed.
fn release_fade(mut drag: Signal<Option<FadeDrag>>, mut live: Signal<Option<(f64, f64)>>) {
    if drag.peek().is_some() {
        drag.set(None);
        spawn(async move {
            tokio::time::sleep(Duration::from_millis(600)).await;
            if drag.peek().is_none() {
                live.set(None);
            }
        });
    }
}

/// The user pressed on a fade handle: remember where the bar is on screen (asked of the browser once, here).
fn begin_fade(end: FadeEnd, now: (f64, f64), strip: Signal<Option<Rc<MountedData>>>, mut drag: Signal<Option<FadeDrag>>, mut live: Signal<Option<(f64, f64)>>) {
    let Some(element) = strip.peek().clone() else { return };
    spawn(async move {
        if let Ok(r) = element.get_client_rect().await {
            if r.size.width > 0.0 {
                drag.set(Some(FadeDrag { end, left: r.origin.x, width: r.size.width }));
                live.set(Some(now));
            }
        }
    });
}

/// How many bars the waveform is drawn with.
const BUCKETS: usize = 900;
/// What the step buttons move by.
const STEP_S: f64 = 1.0;

#[component]
pub fn PreviewPane(state: Signal<State>, mut pos: Signal<f64>, visible: bool, on_mark: EventHandler<Mark>, on_widget: EventHandler<WidgetEdit>, mut drawer: Signal<Drawer>, mut folded: Signal<bool>) -> Element {
    // The fade handles: the bar's element on screen, the handle held, and the lengths while dragging (the list follows a moment later).
    let mut strip = use_signal(|| None::<Rc<MountedData>>);
    let fade_drag = use_signal(|| None::<FadeDrag>);
    let mut live_fade = use_signal(|| None::<(f64, f64)>);
    let mut info = use_signal(|| None::<Result<Probe, String>>);
    let mut wave = use_signal(|| None::<Vec<f32>>);
    let mut prepared = use_signal(|| None::<Prepared>);
    let mut measuring = use_signal(|| false);
    let mut playing = use_signal(|| false);
    let mut failure = use_signal(String::new);
    let mut sound = use_signal(|| None::<Sound>);
    // Where in the output the running sound started, and a number that grows with every preparation, so a late answer is ignored.
    let mut base = use_signal(|| 0.0_f64);
    let mut ticket = use_signal(|| 0_u64);
    // The sound level, 0 to 100, and whether it is muted (the level is kept while muted).
    let mut volume = use_signal(|| 100_u32);
    let mut muted = use_signal(|| false);
    let level = move || if *muted.peek() { 0.0 } else { *volume.peek() as f32 / 100.0 };

    let mut stop_sound = move || {
        let at = match (sound.peek().as_ref(), prepared.peek().as_ref()) {
            (Some(s), Some(p)) => Some(p.source_at(*base.peek() + s.played_s())),
            _ => None,
        };
        sound.set(None);
        playing.set(false);
        if let Some(at) = at {
            pos.set(at);
        }
    };

    // A view that is not on screen does not play: the waveform it was showing stays, paused, as it was.
    use_effect(use_reactive!(|visible| {
        if !visible && *playing.peek() {
            stop_sound();
        }
    }));

    let mut start_sound = move |at: f64| {
        sound.set(None);
        let (Some(source), Some(ff), Some(p)) = (state.peek().source.clone(), RUNNER.ffmpeg(), prepared.peek().clone()) else { return };
        let at = at.clamp(p.window.start, p.window.end);
        match Sound::start(&ff, &source, &p, at, level()) {
            Ok(s) => {
                base.set(p.output_at(at));
                pos.set(at);
                sound.set(Some(s));
                playing.set(true);
                failure.set(String::new());
            }
            Err(why) => {
                failure.set(why);
                playing.set(false);
            }
        }
    };

    // Measure what the effects need and make the plan; `restart` carries a running sound over to the new plan.
    let mut prepare_now = move |restart: bool| {
        let st = state.peek().clone();
        let (Some(source), Some(Ok(probe))) = (st.source.clone(), info.peek().clone()) else { return };
        let duration = probe.duration_us as f64 / 1_000_000.0;
        let mine = *ticket.peek() + 1;
        ticket.set(mine);
        measuring.set(true);
        spawn(async move {
            let entry = st.effects.clone();
            let made = tokio::task::spawn_blocking(move || match RUNNER.ffmpeg() {
                Some(ff) => preview::prepare(&ff, &source, duration, &entry),
                None => Err(String::new()),
            })
            .await
            .unwrap_or_else(|e| Err(e.to_string()));
            if *ticket.peek() != mine {
                return;
            }
            measuring.set(false);
            match made {
                Ok(p) => {
                    prepared.set(Some(p));
                    failure.set(String::new());
                    if restart && *playing.peek() {
                        let at = *pos.peek();
                        start_sound(at);
                    }
                }
                Err(why) => {
                    prepared.set(None);
                    failure.set(why);
                    stop_sound();
                }
            }
        });
    };

    // Follows what it is told: another file, or other effects.
    use_future(move || async move {
        let mut last = State::default();
        loop {
            let st = state.peek().clone();
            if st != last {
                if st.source != last.source {
                    stop_sound();
                    // Coming back to a file the pane was already on keeps the moment: it is only zeroed when one file
                    // replaces another, or none is left.
                    if st.source.is_none() || last.source.is_some() {
                        pos.set(0.0);
                    }
                    ticket += 1;
                    measuring.set(false);
                    info.set(None);
                    wave.set(None);
                    prepared.set(None);
                    failure.set(String::new());
                    if let Some(source) = st.source.clone() {
                        spawn(async move {
                            let path = source.clone();
                            let probed = tokio::task::spawn_blocking(move || match RUNNER.ffmpeg() {
                                Some(ff) => engine::ffmpeg::probe(&ff, &path),
                                None => Err(String::new()),
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()));
                            let length = probed.as_ref().map_or(0.0, |p| p.duration_us as f64 / 1_000_000.0);
                            if state.peek().source.as_ref() != Some(&source) {
                                return;
                            }
                            info.set(Some(probed));
                            prepare_now(false);
                            let shape = tokio::task::spawn_blocking(move || match RUNNER.ffmpeg() {
                                Some(ff) => preview::waveform(&ff, &source, length, BUCKETS),
                                None => Err(String::new()),
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()));
                            match shape {
                                Ok(w) => wave.set(Some(w)),
                                Err(why) if !why.is_empty() && failure.peek().is_empty() => failure.set(why),
                                Err(_) => {}
                            }
                        });
                    }
                } else if st.effects != last.effects {
                    prepare_now(true);
                }
                last = st;
            }
            tokio::time::sleep(Duration::from_millis(150)).await;
        }
    });

    // Playback: the moment follows the samples the device took, and playing ends with the sound.
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(100)).await;
            if !*playing.peek() {
                continue;
            }
            let (at, ended) = match (sound.peek().as_ref(), prepared.peek().as_ref()) {
                (Some(s), Some(p)) => (p.source_at(*base.peek() + s.played_s()), s.finished()),
                _ => (*pos.peek(), true),
            };
            pos.set(at);
            if ended {
                stop_sound();
                pos.set(at);
            }
        }
    });

    let st = state();
    let lang = st.lang.clone();
    let t = |key: &str| i18n::t(&lang, key);
    let s_skipped = t("lblWorkSkipped");
    let left_out: Vec<String> = prepared().map(|p| p.skipped).unwrap_or_default();
    let s_set_in = t("tipWorkSetIn");
    let s_set_out = t("tipWorkSetOut");
    let s_to_in = t("tipWorkToIn");
    let s_to_out = t("tipWorkToOut");
    let s_back = t("tipWorkBackSecond");
    let s_forward = t("tipWorkForwardSecond");
    let s_play = if playing() { t("tipWorkPause") } else { t("tipWorkPlay") };
    let s_clear = t("tipWorkClearCut");
    let s_mute = if muted() { t("tipWorkUnmute") } else { t("tipWorkMute") };
    let s_volume = t("tipWorkVolume");
    let s_files = t("tipWorkShowFiles");
    let (s_fold_on, s_fold_off) = (t("tipWorkUnfold"), t("tipWorkFold"));
    let s_fold = if folded() { s_fold_on } else { s_fold_off };
    let item_title = |key: &str| engine::effects::Item::new(key).title();
    let drawer_buttons = vec![
        (Drawer::Files, s_files.clone(), 0_u8),
        (Drawer::Tone, item_title(crate::work_tone::KEY), 1),
        (Drawer::Compressor, item_title(crate::work_compressor::KEY), 2),
        (Drawer::Level, s_volume.clone(), 3),
        (Drawer::Channels, item_title(crate::work_channels::KEY), 4),
    ];
    let duration_label = t("lblWorkDuration");
    let no_file = t("msgWorkNoSound");
    let s_measuring = t("msgWorkMeasuring");

    let (line, length) = match &*info.read() {
        Some(Ok(p)) => {
            let kbps = p.bitrate_kbps.map(|b| format!(" · {b} kbps")).unwrap_or_default();
            (format!("{}{kbps}", p.codec), p.duration_us as f64 / 1_000_000.0)
        }
        _ => (String::new(), 0.0),
    };
    let plan = prepared.read().clone();
    // The fade lengths are counted on the sound the effects leave: a tempo change makes a second of output more or fewer seconds of the file.
    let ratio = plan.as_ref().map_or(1.0, |p| p.ratio.max(1e-9));
    let now = pos();
    // The part the effects keep. With nothing cut the track stays plain (an empty span), instead of the whole file looking cut.
    let kept = plan.as_ref().map(|p| p.window).filter(|w| w.start > 0.001 || w.end < length - 0.001);
    let tc_now = preview::clock(now);
    let tc_length = match (&plan, kept) {
        (Some(p), Some(w)) => format!("{} → {}", preview::clock(length), preview::clock(w.length() / p.ratio.max(1e-9))),
        _ => preview::clock(length),
    };
    let tc_in = kept.map_or_else(String::new, |w| preview::clock(w.start));
    let tc_out = kept.map_or_else(String::new, |w| preview::clock(w.end));
    let ready = plan.is_some() && length > 0.0;
    let shown_failure = failure();
    let range_time = std::rc::Rc::new(move |at: f64| plan.as_ref().map_or(at, |p| p.range_time(at)));
    let range_time_end = range_time.clone();
    let window_start = prepared.read().as_ref().map_or(0.0, |p| p.window.start);
    let window_end = prepared.read().as_ref().map_or(length, |p| p.window.end);

    // The fade: its two lengths (as dragged, while dragging) and the kept part they count from, all on the sound the effects leave.
    let fade_w = fade_widget();
    let fade_held_values = fade_w.as_ref().map(|w| fade_values(w, &st.effects)).unwrap_or_default();
    let fade_held = fade_w.as_ref().map_or((0.0, 0.0), |w| fades_of(w, &fade_held_values));
    let fades = live_fade().unwrap_or(fade_held);
    let (length_out, kept_out) = (length / ratio, (window_start / ratio, window_end / ratio));
    let room = (kept_out.1 - kept_out.0).max(0.0);
    let fade_shown = (fades.0.min(room), fades.1.min((room - fades.0.min(room)).max(0.0)));
    // the label and the unit of each length, as the catalogue words them
    let fade_words = fade_w.as_ref().and_then(|w| {
        let cat = effects::catalog()?;
        let effect = effects::Item::new(&w.key).effect()?;
        let controls = engine::ffmpeg_effects::resolve_controls(&effect, &cat.filters).ok()?;
        let word = |end: &str| controls.iter().find(|c| Some(&c.id) == w.controls.get(end)).map(|c| (c.label.clone(), c.unit.clone().unwrap_or_default()));
        Some((word("in")?, word("out")?))
    });

    rsx! {
        div {
            class: "wv uis-cmp-panel",
            // The pointer is followed on the whole pane while a fade handle is held (and, with the sheet below, outside the pane too).
            onmousemove: {
                let fade = fade_w.clone().map(|w| (w, fade_held_values.clone()));
                move |e| {
                    let Some(d) = *fade_drag.peek() else { return };
                    let Some((w, values)) = fade.as_ref() else { return };
                    // The button was let go outside the window: the drag is over.
                    if !e.held_buttons().contains(dioxus::html::input_data::MouseButton::Primary) {
                        release_fade(fade_drag, live_fade);
                        return;
                    }
                    let now = live_fade.peek().unwrap_or(fade_held);
                    let next = dragged_fades(&d, e.client_coordinates().x, length_out, kept_out, now);
                    if *live_fade.peek() != Some(next) {
                        live_fade.set(Some(next));
                        on_widget.call(WidgetEdit { key: w.key.clone(), values: Some(with_fades(w, values.clone(), next)), keep: false });
                    }
                }
            },
            onmouseup: move |_| release_fade(fade_drag, live_fade),
            if fade_drag().is_some() {
                div { class: "wv-catcher" }
            }
            if st.source.is_none() {
                div { class: "wv-screen", p { class: "wv-failure", "{no_file}" } }
            } else {
                div { class: "wv-info",
                    "{line}"
                    if measuring() {
                        span { class: "wv-measuring", "{s_measuring}" }
                    }
                }
                UisWaveform {
                    peaks: wave.read().clone().unwrap_or_default(),
                    length,
                    now,
                    kept: kept.map(|w| (w.start, w.end)),
                    now_label: tc_now.clone(),
                    length_label: format!("{duration_label}: {tc_length}"),
                    disabled: !ready,
                    on_seek: move |v: f64| {
                        pos.set(v);
                        if *playing.peek() {
                            start_sound(v);
                        }
                    },
                    if !shown_failure.is_empty() {
                        p { class: "wv-failure", "{shown_failure}" }
                    } else if !left_out.is_empty() {
                        p { class: "wv-failure", "{s_skipped}
{left_out.join(\"\n\")}" }
                    }
                    if let (true, Some(w), Some(((label_in, unit_in), (label_out, unit_out)))) = (ready, fade_w.clone(), fade_words.clone()) {
                        {
                            let at = |v: f64| if length_out > 0.0 { (v / length_out * 100.0).clamp(0.0, 100.0) } else { 0.0 };
                            let start = at(kept_out.0);
                            let end = at(kept_out.1);
                            let in_at = at(kept_out.0 + fade_shown.0);
                            let out_at = at(kept_out.1 - fade_shown.1);
                            let held_now = (fade_shown.0, fade_shown.1);
                            let (values_in, values_out, w_in, w_out) = (fade_held_values.clone(), fade_held_values.clone(), w.clone(), w.clone());
                            rsx! {
                                div {
                                    class: "wv-fades",
                                    onmounted: move |e| strip.set(Some(e.data())),
                                    div { class: "wv-fade-ramp wv-fade-ramp-in", style: "left: {start}%; width: {in_at - start}%" }
                                    div { class: "wv-fade-ramp wv-fade-ramp-out", style: "left: {out_at}%; width: {end - out_at}%" }
                                    div {
                                        class: "wv-fade-handle",
                                        title: "{label_in}",
                                        style: "left: {in_at}%",
                                        onmousedown: move |e| {
                                            e.stop_propagation();
                                            e.prevent_default();
                                            begin_fade(FadeEnd::In, held_now, strip, fade_drag, live_fade);
                                        },
                                    }
                                    div {
                                        class: "wv-fade-handle",
                                        title: "{label_out}",
                                        style: "left: {out_at}%",
                                        onmousedown: move |e| {
                                            e.stop_propagation();
                                            e.prevent_default();
                                            begin_fade(FadeEnd::Out, held_now, strip, fade_drag, live_fade);
                                        },
                                    }
                                }
                                div { class: "wv-fadenums uis-cmp-inline",
                                    UisFldLabelled { compact: true, label: label_in.clone(),
                                        UisNum {
                                            value: Some(fade_shown.0),
                                            min: Some(0.0),
                                            max: Some(MAX_FADE),
                                            step: 0.1,
                                            decimals: Some(1),
                                            width: "64px".to_owned(),
                                            on_change: move |v: Option<f64>| on_widget.call(WidgetEdit { key: w_in.key.clone(), values: Some(with_fades(&w_in, values_in.clone(), (v.unwrap_or(0.0), fade_held.1))), keep: false }),
                                        }
                                        span { "{unit_in}" }
                                    }
                                    UisFldLabelled { compact: true, label: label_out.clone(),
                                        UisNum {
                                            value: Some(fade_shown.1),
                                            min: Some(0.0),
                                            max: Some(MAX_FADE),
                                            step: 0.1,
                                            decimals: Some(1),
                                            width: "64px".to_owned(),
                                            on_change: move |v: Option<f64>| on_widget.call(WidgetEdit { key: w_out.key.clone(), values: Some(with_fades(&w_out, values_out.clone(), (fade_held.0, v.unwrap_or(0.0)))), keep: false }),
                                        }
                                        span { "{unit_out}" }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "wv-bar",
                    button {
                        class: "uis-btn",
                        title: "{s_set_in}",
                        "aria-label": "{s_set_in}",
                        disabled: !ready,
                        onclick: move |_| on_mark.call(Mark::Start(range_time(*pos.peek()))),
                        "["
                    }
                    button {
                        class: "uis-btn ico",
                        title: "{s_to_in}",
                        "aria-label": "{s_to_in}",
                        disabled: !ready,
                        onclick: move |_| {
                            pos.set(window_start);
                            if *playing.peek() {
                                start_sound(window_start);
                            }
                        },
                        "|<"
                    }
                    button {
                        class: "uis-btn ico",
                        title: "{s_back}",
                        "aria-label": "{s_back}",
                        disabled: !ready,
                        onclick: move |_| {
                            stop_sound();
                            let v = *pos.peek();
                            pos.set((v - STEP_S).max(0.0));
                        },
                        Icon { width: 14, height: 14, icon: LdChevronLeft }
                    }
                    button {
                        class: "uis-btn ico",
                        title: "{s_play}",
                        "aria-label": "{s_play}",
                        disabled: !ready,
                        onclick: move |_| {
                            if *playing.peek() {
                                stop_sound();
                            } else {
                                // Play from the end starts again from the start of what is kept.
                                let mut at = *pos.peek();
                                if at >= window_end - 0.05 {
                                    at = window_start;
                                }
                                start_sound(at);
                            }
                        },
                        if playing() {
                            Icon { width: 14, height: 14, icon: LdPause }
                        } else {
                            Icon { width: 14, height: 14, icon: LdPlay }
                        }
                    }
                    button {
                        class: "uis-btn ico",
                        title: "{s_forward}",
                        "aria-label": "{s_forward}",
                        disabled: !ready,
                        onclick: move |_| {
                            stop_sound();
                            let v = *pos.peek();
                            pos.set((v + STEP_S).min(length));
                        },
                        Icon { width: 14, height: 14, icon: LdChevronRight }
                    }
                    button {
                        class: "uis-btn ico",
                        title: "{s_to_out}",
                        "aria-label": "{s_to_out}",
                        disabled: !ready,
                        onclick: move |_| {
                            stop_sound();
                            pos.set(window_end);
                        },
                        ">|"
                    }
                    button {
                        class: "uis-btn",
                        title: "{s_set_out}",
                        "aria-label": "{s_set_out}",
                        disabled: !ready,
                        onclick: move |_| on_mark.call(Mark::End(range_time_end(*pos.peek()))),
                        "]"
                    }
                    button {
                        class: "uis-btn ico subtle",
                        title: "{s_clear}",
                        "aria-label": "{s_clear}",
                        disabled: kept.is_none(),
                        onclick: move |_| on_mark.call(Mark::Clear),
                        Icon { width: 14, height: 14, icon: LdX }
                    }
                    button {
                        class: "uis-btn ico subtle",
                        title: "{s_mute}",
                        "aria-label": "{s_mute}",
                        onclick: move |_| {
                            let now_muted = !*muted.peek();
                            muted.set(now_muted);
                            if let Some(s) = sound.peek().as_ref() {
                                s.set_volume(level());
                            }
                        },
                        if muted() || volume() == 0 {
                            Icon { width: 14, height: 14, icon: LdVolumeX }
                        } else {
                            Icon { width: 14, height: 14, icon: LdVolume2 }
                        }
                    }
                    input {
                        class: "uis-sld wv-volume",
                        r#type: "range",
                        min: "0",
                        max: "100",
                        title: "{s_volume}",
                        "aria-label": "{s_volume}",
                        value: "{volume}",
                        oninput: move |e| {
                            if let Ok(v) = e.value().parse::<u32>() {
                                volume.set(v);
                                muted.set(false);
                                if let Some(s) = sound.peek().as_ref() {
                                    s.set_volume(level());
                                }
                            }
                        },
                    }
                    // the panels under the waveform: the file list and the drawers (one at a time)
                    for (which , tip , kind) in drawer_buttons.clone() {
                        button {
                            key: "{tip}",
                            class: if drawer() == which { "uis-btn ico active" } else { "uis-btn ico" },
                            title: "{tip}",
                            "aria-label": "{tip}",
                            "aria-pressed": "{drawer() == which}",
                            onclick: move |_| {
                                folded.set(false);
                                drawer.set(which);
                            },
                            match kind {
                                0 => rsx! { Icon { width: 14, height: 14, icon: LdList } },
                                1 => rsx! { Icon { width: 14, height: 14, icon: LdSlidersVertical } },
                                2 => rsx! { Icon { width: 14, height: 14, icon: LdActivity } },
                                3 => rsx! { Icon { width: 14, height: 14, icon: LdAudioLines } },
                                _ => rsx! { Icon { width: 14, height: 14, icon: LdArrowLeftRight } },
                            }
                        }
                    }
                    span { class: "uis-bar-spacer" }
                    button {
                        class: if folded() { "uis-btn ico active" } else { "uis-btn ico" },
                        title: "{s_fold}",
                        "aria-label": "{s_fold}",
                        "aria-pressed": "{folded()}",
                        onclick: move |_| folded.set(!folded()),
                        if folded() {
                            Icon { width: 14, height: 14, icon: LdChevronsUp }
                        } else {
                            Icon { width: 14, height: 14, icon: LdChevronsDown }
                        }
                    }
                    span { class: "wv-cut", "{tc_in}" }
                    span { class: "wv-cut", "{tc_out}" }
                }
            }
        }
    }
}
