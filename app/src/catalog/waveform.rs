// twin: uis-ctl-waveform.jsx jsx@4d66e685205f rsx@c7de728af730
//! UisWaveform - twin of `uis-ctl-waveform.jsx`: a sound's waveform on a screen, the part a cut drops dimmed, the
//! moment as a line, the time row and the timeline under it (a range whose track shows the kept part). Same classes and
//! element order, so the paired `uis-ctl-waveform.css` skins it.
//!
//! `peaks` are the waveform's buckets, 0 to 1; `length` the file's seconds and `now` the moment; `kept` is `(start, end)`
//! in seconds when a cut keeps only part. `now_label` / `length_label` are the two time texts, already formatted.
//! `on_seek` says where the user put the moment. `children` are drawn on the screen over the waveform.
//! Differs: the stylesheet is added to the page once by id through `document::eval` (the JSX imports its CSS), because a `document::Style` inside a control logs a warning on every re-render.

use dioxus::prelude::*;

const CSS: &str = include_str!("../../assets/uis-ctl-waveform.css");

#[component]
pub fn UisWaveform(
    #[props(default)] peaks: Vec<f32>,
    #[props(default)] length: f64,
    #[props(default)] now: f64,
    #[props(default)] kept: Option<(f64, f64)>,
    #[props(default)] now_label: String,
    #[props(default)] length_label: String,
    #[props(default)] disabled: bool,
    on_seek: EventHandler<f64>,
    children: Element,
) -> Element {
    let pct = move |v: f64| if length > 0.0 { (v / length * 100.0).clamp(0.0, 100.0) } else { 0.0 };
    let mut d = String::new();
    for (i, v) in peaks.iter().enumerate() {
        let h = (v * 46.0).max(0.8);
        d.push_str(&format!("M{i},{:.1}V{:.1}", 50.0 - h, 50.0 + h));
    }
    let n = peaks.len();
    let region = match kept {
        Some((s, e)) => format!("--in: {:.2}%; --out: {:.2}%", pct(s), pct(e)),
        None => "--in: 0%; --out: 0%".to_owned(),
    };
    let dim_left = kept.map(|(s, _)| format!("left: 0; width: {:.2}%", pct(s)));
    let dim_right = kept.map(|(_, e)| format!("left: {:.2}%; right: 0", pct(e)));
    let head = format!("left: {:.2}%", pct(now));

    // the stylesheet goes into the page once; a `document::Style` here logs a warning on every re-render
    use_hook(|| {
        let _ = document::eval(&format!(
            "if (!document.getElementById({id:?})) {{ const s = document.createElement('style'); s.id = {id:?}; s.textContent = {css:?}; document.head.appendChild(s); }}",
            id = "uis-wvf-css",
            css = CSS
        ));
    });
    rsx! {
        div { class: "uis-wvf",
            div { class: "uis-wvf-screen",
                if n > 0 {
                    svg { class: "uis-wvf-svg", view_box: "0 0 {n} 100", preserve_aspect_ratio: "none",
                        path { d: "{d}", stroke: "currentColor", stroke_width: "0.8", fill: "none" }
                    }
                    if let Some(style) = dim_left {
                        div { class: "uis-wvf-dim", style: "{style}" }
                    }
                    if let Some(style) = dim_right {
                        div { class: "uis-wvf-dim", style: "{style}" }
                    }
                    div { class: "uis-wvf-head", style: "{head}" }
                }
                {children}
            }
            div { class: "uis-wvf-times",
                span { class: "uis-wvf-now", "{now_label}" }
                span { class: "uis-wvf-length", "{length_label}" }
            }
            input {
                class: "uis-wvf-timeline",
                r#type: "range",
                min: "0",
                max: "{length}",
                step: "0.01",
                value: "{now}",
                style: "{region}",
                disabled,
                oninput: move |e| {
                    if let Ok(v) = e.value().parse::<f64>() {
                        on_seek.call(v);
                    }
                },
            }
        }
    }
}
