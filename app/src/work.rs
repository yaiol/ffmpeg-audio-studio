//! The work view: ONE file at a time, heard with its own list of effects within reach (`work_effects.rs`) and shown as a waveform
//! (`work_preview.rs`). A copy of the batch view (`converter.rs`) with a compact panel; the list of effects is the view's own, it
//! is not a saved effects entry.
//!
//! ⚠ CLAUDE: NO HARDCODED UI STRINGS. Every user-visible string goes through the t() lookup with the key
//! a string literal at the call site.

use crate::{
    catalog::{UisAppHeader, UisFldLabelled, LbsDropZone},
    converter::{Item, NEXT_ID, RunTag, TagKind, nothing_to_do, overall_percent, percent_text, range_between},
    ffmpeg_panel::{FfmpegPopup, Popup, download_note},
    work_command::CommandPanel,
    i18n, manager,
    preview_session::{Drawer, Mark, State, WidgetEdit},
    runner::{RUNNER, RunState},
    settings::{RunChoice, Settings},
    work_effects::{FxView, WorkEffects},
    work_preview::PreviewPane,
    work_channels::ChannelsDrawer,
    work_compressor::CompressorDrawer,
    work_level::LevelDrawer,
    work_tone::ToneDrawer,
};
use dioxus::prelude::*;
use dioxus_free_icons::{
    Icon,
    icons::ld_icons::{LdEraser, LdFilePlus, LdFolder, LdFolderOpen, LdMusic, LdPlay, LdVolume2, LdSquare, LdX},
};
use engine::{
    JobSpec, JobStatus, Overwrite,
    destination::{PICK_ID, SOURCE_ID},
    download::DownloadState,
    effects::EffectsEntry,
    settings::Panel,
};
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    sync::atomic::Ordering,
    time::Duration,
};

/// What a mark of the preview does to the work view's list: the range effect gets the time (and is added when the list has none).
fn apply_mark(mut work_fx: Signal<EffectsEntry>, standard: &[String], mark: Mark) {
    let mut e = work_fx.peek().clone();
    match mark {
        Mark::Start(t) => e.mark_range(Some(t), None),
        Mark::End(t) => e.mark_range(None, Some(t)),
        Mark::Clear => e.clear_range(),
    }
    work_fx.set(e.normalized().arranged(standard));
}

/// The panel's choices as one file's run settings (the effects are the work list, as they are now).
fn panel_run(settings: Signal<Settings>, work_fx: Signal<EffectsEntry>, effects_name: &str) -> RunChoice {
    let s = settings.peek();
    let mut run = s.choice_of(&s.work);
    let mut fx = work_fx.peek().clone();
    fx.name = effects_name.to_owned();
    run.effects_inline = Some(fx);
    run
}

/// What the panel was last known to hold, and for which selection: the point the next change is measured from.
#[derive(Default)]
struct PanelSeen {
    selection: HashSet<u64>,
    run: Option<RunChoice>,
}

/// Where the selected files do not agree: the panel shows those blank, until a touch makes them agree.
#[derive(Clone, Default, PartialEq)]
pub struct Mixed {
    /// The panel's fields (`preset`, `tags`, `destination`, `overwrite`) that differ.
    pub fields: HashSet<&'static str>,
    /// Effects some of the files have and others do not.
    pub partial: HashSet<String>,
    /// Effects every file has, with values that are not the same.
    pub differing: HashSet<String>,
    /// The controls of those (`<effect>.<control>`) the files do not agree on.
    pub controls: HashSet<String>,
}

fn mixed_of(runs: &[&RunChoice]) -> Mixed {
    let mut m = Mixed::default();
    let Some((first, rest)) = runs.split_first() else { return m };
    if rest.is_empty() {
        return m;
    }
    let differ = |f: &dyn Fn(&RunChoice) -> String| rest.iter().any(|r| f(r) != f(first));
    let checks: [(&'static str, &dyn Fn(&RunChoice) -> String); 4] = [
        ("preset", &|r| r.preset_id.clone()),
        ("tags", &|r| r.cleanup_id.clone()),
        ("destination", &|r| r.destination_id.clone()),
        ("overwrite", &|r| format!("{:?}", r.overwrite)),
    ];
    for (name, f) in checks {
        if differ(f) {
            m.fields.insert(name);
        }
    }
    let items_of = |r: &RunChoice| r.effects_inline.as_ref().map(|e| e.items.clone()).unwrap_or_default();
    let all: Vec<Vec<engine::effects::Item>> = runs.iter().map(|r| items_of(r)).collect();
    let keys: HashSet<&String> = all.iter().flatten().map(|i| &i.key).collect();
    for key in keys {
        let found: Vec<Option<&engine::effects::Item>> = all.iter().map(|l| l.iter().find(|i| &i.key == key)).collect();
        if found.iter().any(Option::is_none) {
            m.partial.insert(key.clone());
        } else if found.iter().any(|i| i != &found[0]) {
            m.differing.insert(key.clone());
            let values: Vec<_> = found.iter().flatten().map(|i| i.all_values()).collect();
            let ids: HashSet<&String> = values.iter().flat_map(|v| v.keys()).collect();
            for id in ids {
                if values.iter().any(|v| v.get(id) != values[0].get(id)) {
                    m.controls.insert(format!("{key}.{id}"));
                }
            }
        }
    }
    m
}

/// Write one change into every selected file that is not queued or running: a touch of a control that the files disagree on
/// reaches them all, even when it picks what the panel already held.
fn write_selected(mut items: Signal<Vec<Item>>, selected: Signal<HashSet<u64>>, statuses: Signal<HashMap<u64, JobStatus>>, change: impl Fn(&mut RunChoice)) {
    let sel = selected.peek().clone();
    let st = statuses.peek();
    for i in items.write().iter_mut() {
        if sel.contains(&i.id) && !st.get(&i.id).is_some_and(JobStatus::is_active) {
            change(&mut i.run);
        }
    }
}

/// The controls touched in the panel, copied from the panel's effects whatever they held before.
fn write_forced(run: &mut RunChoice, panel: &RunChoice, forced: &HashSet<String>) {
    let (Some(from), Some(own)) = (panel.effects_inline.as_ref(), run.effects_inline.as_mut()) else { return };
    for f in forced {
        let Some((key, id)) = f.split_once('.') else { continue };
        // what the panel holds for it: its own value, or the control's default when it holds none
        let value = from.items.iter().find(|i| i.key == key).and_then(|i| i.values.get(id).cloned().or_else(|| i.all_values().get(id).map(|v| v.to_json())));
        if let (Some(v), Some(item)) = (value, own.items.iter_mut().find(|i| i.key == key)) {
            item.values.insert(id.to_owned(), v);
        }
    }
}

/// What the panel changed since `base`, written into `run`: every field it touched, and nothing else - so a curve moved for
/// several files leaves each file's own effects alone.
fn write_change(run: &mut RunChoice, base: &RunChoice, panel: &RunChoice) {
    if panel.preset_id != base.preset_id {
        run.preset_id = panel.preset_id.clone();
    }
    if panel.cleanup_id != base.cleanup_id {
        run.cleanup_id = panel.cleanup_id.clone();
    }
    if panel.destination_id != base.destination_id {
        run.destination_id = panel.destination_id.clone();
    }
    if panel.overwrite != base.overwrite {
        run.overwrite = panel.overwrite;
    }
    let none: Vec<engine::effects::Item> = Vec::new();
    let before = base.effects_inline.as_ref().map_or(&none, |e| &e.items);
    let after = panel.effects_inline.as_ref().map_or(&none, |e| &e.items);
    if before == after {
        return;
    }
    let mut fx = run.effects_inline.clone().unwrap_or_else(|| EffectsEntry::new(""));
    for old in before {
        if !after.iter().any(|n| n.key == old.key) {
            fx.items.retain(|i| i.key != old.key);
        }
    }
    for new in after {
        let old = before.iter().find(|o| o.key == new.key);
        if old == Some(new) {
            continue;
        }
        match (fx.items.iter_mut().find(|i| i.key == new.key), old) {
            // the file has it too: only the values the panel changed
            (Some(own), Some(old)) => {
                for (k, v) in &new.values {
                    if old.values.get(k) != Some(v) {
                        own.values.insert(k.clone(), v.clone());
                    }
                }
                own.values.retain(|k, _| old.values.get(k).is_none() || new.values.contains_key(k));
            }
            (Some(own), None) => *own = new.clone(),
            (None, _) => fx.items.push(new.clone()),
        }
    }
    run.effects_inline = Some(fx.normalized().arranged(&crate::work_effects::standard()));
}

/// The panel and the selected files are ONE setting. A new selection loads its first file's choices into the panel; after that, whatever the panel
/// changed is written, control by control, to every selected file that is not queued or running (each file otherwise keeps its own). Run every tick
/// by the view's loop.
fn sync_panel(
    mut settings: Signal<Settings>,
    mut work_fx: Signal<EffectsEntry>,
    mut items: Signal<Vec<Item>>,
    selected: Signal<HashSet<u64>>,
    statuses: Signal<HashMap<u64, JobStatus>>,
    mut mixed: Signal<Mixed>,
    mut touched: Signal<HashSet<String>>,
    seen: &mut PanelSeen,
) {
    let sel = selected.peek().clone();
    let now = {
        let list = items.peek();
        let runs: Vec<&RunChoice> = list.iter().filter(|i| sel.contains(&i.id)).map(|i| &i.run).collect();
        mixed_of(&runs)
    };
    if *mixed.peek() != now {
        mixed.set(now);
    }
    let Some(first) = items.peek().iter().find(|i| sel.contains(&i.id)).map(|i| i.run.clone()) else {
        seen.selection = sel;
        seen.run = None;
        return;
    };
    let name = first.effects_inline.as_ref().map(|e| e.name.clone()).unwrap_or_default();
    if sel != seen.selection || seen.run.is_none() {
        // a new selection: the panel shows the first file's choices
        seen.selection = sel;
        let p = settings.peek().work.clone();
        let loaded = Panel { preset_id: first.preset_id.clone(), cleanup_id: first.cleanup_id.clone(), destination_id: first.destination_id.clone(), overwrite: first.overwrite, ..p.clone() };
        if loaded != p {
            settings.write().work = loaded;
        }
        let fx = settings.peek().effects_of(&first);
        if work_fx.peek().items != fx.items {
            work_fx.set(fx);
        }
        seen.run = Some(panel_run(settings, work_fx, &name));
        return;
    }
    let panel = panel_run(settings, work_fx, &name);
    let Some(base) = seen.run.clone() else { return };
    let forced: HashSet<String> = std::mem::take(&mut *touched.write());
    if panel == base && forced.is_empty() {
        return;
    }
    let st = statuses.peek();
    for i in items.write().iter_mut() {
        if sel.contains(&i.id) && !st.get(&i.id).is_some_and(JobStatus::is_active) {
            write_change(&mut i.run, &base, &panel);
            write_forced(&mut i.run, &panel, &forced);
        }
    }
    seen.run = Some(panel);
}

/// What a drawer did to the effect it edits: its item takes the new values, and leaves the list while every value is neutral.
fn apply_widget(mut work_fx: Signal<EffectsEntry>, standard: &[String], edit: WidgetEdit) {
    let mut e = work_fx.peek().clone();
    if edit.keep {
        e.set_item(&edit.key, edit.values.as_ref());
    } else {
        e.set_widget(&edit.key, edit.values.as_ref());
    }
    work_fx.set(e.normalized().arranged(standard));
}

/// What the preview is told: the first selected file in list order, the work view's effects, and the language.
fn chosen_preview(settings: Signal<Settings>, work_fx: Signal<EffectsEntry>, items: Signal<Vec<Item>>, selected: Signal<HashSet<u64>>) -> State {
    let sel = selected.peek();
    State {
        source: items.peek().iter().find(|i| sel.contains(&i.id)).map(|i| i.path.clone()),
        effects: work_fx.peek().clone(),
        lang: settings.peek().lang.clone(),
    }
}

#[component]
pub fn Work(
    /// What the app knows about the runs and ffmpeg.
    run: RunState,
    /// True while this view is the one on screen: both stay mounted, the other is only hidden.
    active: bool,
    settings: Signal<Settings>,
    app_name: String,
    app_version: String,
    /// The header's first group (work / batch) - the app owns it.
    header_start: Element,
    /// The header's right-hand group (GitHub, help, settings) - the app owns those.
    on_manage: EventHandler<String>,
    children: Element,
) -> Element {
    let mut items = use_signal(Vec::<Item>::new);
    let mut selected = use_signal(HashSet::<u64>::new);
    let mut anchor = use_signal(|| None::<u64>);
    // The view's own list of effects, heard in the preview and applied to the files it is applied to.
    let work_fx = use_signal(|| EffectsEntry::new("work"));
    // Where the selected files disagree, and the controls touched in the panel since the last tick (a touch reaches every selected file even
    // when it sets what the panel already held).
    let mixed = use_signal(Mixed::default);
    let touched = use_signal(HashSet::<String>::new);
    let fx_view = FxView::new();
    // The tone drawer (the equaliser's curve) in the place of the file list.
    let drawer = use_signal(Drawer::default);
    // the panel under the preview folded away, so the picture takes its room
    let folded = use_signal(|| false);
    // Where the preview was, in seconds of the file.
    let pos = use_signal(|| 0.0_f64);
    let mut pane_state = use_signal(State::default);
    let RunState { statuses, download, ffmpeg_ready, mut dismissed } = run;
    let mut pick_nonce = use_signal(|| 0u32);

    // What the preview pane in this window is told.
    use_future(move || async move {
        let mut seen = PanelSeen::default();
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            sync_panel(settings, work_fx, items, selected, statuses, mixed, touched, &mut seen);
            let chosen = chosen_preview(settings, work_fx, items, selected);
            if *pane_state.peek() != chosen {
                pane_state.set(chosen);
            }
        }
    });

    // The window may not be shorter than the layout needs: measured whenever this view comes on screen (both views stay
    // mounted, and a hidden one has no layout to measure).
    use_effect(use_reactive!(|active| {
        if active {
            spawn(crate::fit_window_height());
        }
    }));

    let lang = settings.read().lang.clone();
    let t = |key: &str| i18n::t(&lang, key);

    // The command the first selected file would run with, the panel's choices and the effects as they are now (measured off the UI thread).
    let command = use_resource(move || {
        let s = settings.read();
        let sel = selected.read();
        let spec = items.read().iter().find(|i| sel.contains(&i.id)).map(|i| {
            let run = panel_run(settings, work_fx, "");
            JobSpec {
                id: i.id,
                input: i.path.clone(),
                source_root: None,
                preset: s.preset_by(&run.preset_id),
                cleanup: s.cleanup_by(&run.cleanup_id),
                destination: s.destination_by(&run.destination_id),
                overwrite: Overwrite::Overwrite,
                effects: s.effects_of(&run),
            }
        });
        async move {
            let spec = spec?;
            tokio::task::spawn_blocking(move || {
                let dir = engine::settings::ffmpeg_dir();
                let ffmpeg = engine::ffmpeg::locate(dir.as_deref()).ok_or_else(|| "ffmpeg not found".to_owned())?;
                engine::command_line(&ffmpeg, &spec)
            })
            .await
            .ok()
        }
    });
    let (command_text, command_failed) = match &*command.read() {
        Some(Some(Ok(line))) => (line.clone(), false),
        Some(Some(Err(reason))) => (reason.clone(), true),
        _ => (String::new(), false),
    };

    let mut add_paths = move |paths: Vec<PathBuf>| {
        // Each file holds its own settings: an added file starts with no effects and takes the preset, tags and destination of the file
        // before it (the panel's own, for the first one). It becomes the selection.
        let mut run = match items.peek().last() {
            Some(prev) => prev.run.clone(),
            None => settings.read().choice_of(&settings.read().work),
        };
        run.effects_id = crate::settings::NONE_ID.to_owned();
        let mut fx = EffectsEntry::new("");
        fx.name = format!("{} (0)", i18n::t(&settings.peek().lang, "lblWorkEffects"));
        run.effects_inline = Some(fx);
        let mut fresh = Vec::new();
        let mut list = items.write();
        for p in &paths {
            // Work takes files, never a folder: a dropped folder is ignored.
            if p.is_dir() {
                continue;
            }
            let mut found = Vec::new();
            engine::collect_audio(p, &mut found);
            for path in found {
                if list.iter().any(|i| i.path == path) {
                    continue;
                }
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
                fresh.push(id);
                list.push(Item { id, name, path, root: None, run: run.clone() });
            }
        }
        if let Some(last) = fresh.last().copied() {
            selected.set(fresh.into_iter().collect());
            anchor.set(Some(last));
        }
    };

    let pick_files = move |_| {
        spawn(async move {
            let exts: Vec<&str> = engine::AUDIO_EXTENSIONS.to_vec();
            if let Some(files) = rfd::AsyncFileDialog::new().add_filter("audio", &exts).pick_files().await {
                add_paths(files.into_iter().map(|f| f.path().to_path_buf()).collect());
            }
        });
    };

    let ffmpeg_missing = !ffmpeg_ready();
    let busy = statuses.read().values().any(JobStatus::is_active);

    // Every file runs with ITS OWN choices; only the number at once belongs to the run. A file whose choices would only
    // re-encode it as it is has nothing to do and is left out.
    let convert = move |_| {
        let s = settings.read();
        let parallel = s.work.parallel as usize;
        let st = statuses.peek();
        let specs: Vec<JobSpec> = items
            .read()
            .iter()
            // Active ones are already queued or running; everything else - new, done, failed, skipped, cancelled - runs again
            // with its current choices.
            .filter(|i| !st.get(&i.id).is_some_and(JobStatus::is_active))
            .filter(|i| !nothing_to_do(&s, &i.run))
            .map(|i| JobSpec {
                id: i.id,
                input: i.path.clone(),
                source_root: None,
                preset: s.preset_by(&i.run.preset_id),
                cleanup: s.cleanup_by(&i.run.cleanup_id),
                destination: s.destination_by(&i.run.destination_id),
                overwrite: i.run.overwrite,
                effects: s.effects_of(&i.run),
            })
            .collect();
        drop(st);
        drop(s);
        RUNNER.start(specs, parallel);
    };
    let stop = move |_| RUNNER.cancel_all();
    let have_items = !items.read().is_empty();

    let s = settings.read();
    let mut preset_options: Vec<(String, String)> = vec![(engine::preset::ORIGINAL_ID.to_owned(), t("lblMainPresetOriginal"))];
    preset_options.extend(s.presets.iter().map(|p| (p.id.clone(), p.name.clone())));
    let preset_id = s.work.preset_id.clone();
    let mut cleanup_options: Vec<(String, String)> = vec![(crate::settings::NONE_ID.to_owned(), t("lblMainNone"))];
    cleanup_options.extend(s.cleanups.iter().map(|c| (c.id.clone(), c.name.clone())));
    let cleanup_id = s.work.cleanup_id.clone();
    let mut dest_options: Vec<(String, String)> = vec![(SOURCE_ID.to_owned(), t("lblMainDestSource"))];
    dest_options.extend(s.destinations.iter().map(|d| (d.id.clone(), d.name.clone())));
    dest_options.push((PICK_ID.to_owned(), t("lblMainDestPick")));
    let destination_id = s.work.destination_id.clone();
    let saved_folder = Some(PathBuf::from(s.destination_by(&s.work.destination_id).folder.trim())).filter(|p| !p.as_os_str().is_empty());
    let parallel = s.work.parallel;
    let overwrite = s.work.overwrite;
    // Where the selected files disagree the box shows nothing chosen: the value the panel holds is only the first file's.
    let mixed_now = mixed.read().clone();
    let is_mixed = |f: &str| mixed_now.fields.contains(f);
    let (mx_preset, mx_tags, mx_dest, mx_over) = (is_mixed("preset"), is_mixed("tags"), is_mixed("destination"), is_mixed("overwrite"));
    let shown = |mixed: bool, id: &str| if mixed { String::new() } else { id.to_owned() };
    let (shown_preset, shown_tags, shown_dest) = (shown(mx_preset, &preset_id), shown(mx_tags, &cleanup_id), shown(mx_dest, &destination_id));
    // Every file keeps its format, its tags and applies nothing: converting would only re-encode them, so the button is off.
    let all_idle_and_plain = {
        let st = statuses.read();
        !items.read().iter().any(|i| !st.get(&i.id).is_some_and(JobStatus::is_active) && !nothing_to_do(&s, &i.run))
    };
    drop(s);

    let s_main_add_files = t("btnMainAddFiles");
    let s_main_stop = t("btnMainStop");
    let s_main_convert = t("btnMainConvert");
    let s_main_preset = t("lblMainAudio");
    let s_main_cleanup = t("tipMainCleanup");
    let s_main_effects = t("tipMainEffects");
    let s_lbl_tags = t("lblWorkTags");
    let s_lbl_effects = t("lblWorkEffects");
    let s_lbl_destination = t("lblWorkDestination");
    let s_lbl_overwrite = t("lblWorkOverwrite");
    let s_main_edit_presets = t("tipMainEditPresets");
    let s_main_edit_cleanups = t("tipMainEditCleanups");
    let s_main_destination = t("tipMainDestination");
    let s_main_edit_destinations = t("tipMainEditDestinations");
    let s_main_overwrite = t("tipMainOverwrite");
    let s_main_overwrite_skip = t("lblMainOverwriteSkip");
    let s_main_overwrite_overwrite = t("lblMainOverwriteOverwrite");
    let s_main_overwrite_rename = t("lblMainOverwriteRename");
    let s_main_parallel = t("tipMainParallel");
    let s_main_open_output = t("tipMainOpenOutput");
    let sel_now = selected.read().clone();
    // The names tags are made of, once per render: a row only looks its choices up.
    let cfg = settings.read().clone();
    let drop_label = t("msgMainDropLabel");
    let standard_keys = crate::work_effects::standard();

    // The left of the footer: what the list holds, how far a run is, or how the last run ended.
    let empty_list = items.read().is_empty();
    let drop_text = t("msgWorkDrop");
    let status_text = {
        let st = statuses.read();
        let total = items.read().len();
        let touched = items.read().iter().filter(|i| st.contains_key(&i.id)).count();
        if ffmpeg_missing && !download().is_active() {
            t("msgMainNoFfmpeg")
        } else if total == 0 {
            String::new()
        } else if busy {
            let finished = st.values().filter(|s| !s.is_active()).count();
            format!("{} · {}", t("msgMainProgress").replace("{done}", &finished.to_string()).replace("{total}", &touched.to_string()), percent_text(overall_percent(st.values())))
        } else if touched > 0 {
            engine::run_summary(st.values(), &lang)
        } else {
            t("msgMainReady").replace("{n}", &total.to_string())
        }
    };
    let status_text = if !sel_now.is_empty() && !busy {
        let n = t("msgMainSelected").replace("{n}", &sel_now.len().to_string());
        if status_text.is_empty() { n } else { format!("{status_text} · {n}") }
    } else {
        status_text
    };

    // The ffmpeg popup: the install's progress, or why it failed. An ffmpeg already in place being replaced is an
    // update; with none, an install.
    let downloading = download().is_active();
    let update = ffmpeg_ready();
    let popup = if downloading {
        download_note(&lang, &download()).map(|text| Popup::Progress { text, update })
    } else if dismissed() {
        None
    } else if matches!(download(), DownloadState::Failed(_)) {
        download_note(&lang, &download()).map(|text| Popup::Failed { text, update })
    } else {
        None
    };

    // Where "open the output folder" goes: the folder of the newest finished file - which is right for "beside the source" too -
    // and otherwise the chosen folder.
    let open_target: Option<PathBuf> = statuses
        .read()
        .iter()
        .filter_map(|(id, s)| match s {
            JobStatus::Done { output } => Some((*id, output.clone())),
            _ => None,
        })
        .max_by_key(|(id, _)| *id)
        .and_then(|(_, out)| out.parent().map(Path::to_path_buf))
        .or(saved_folder)
        .filter(|p| p.is_dir());

    rsx! {
        // ⚠ CLAUDE: the header holds the file actions only (icon-only, tooltips). The choices that shape a
        // run - preset, tags, effects, where it goes, how many at once, Convert - live in the right-hand panel below,
        // one aligned column.
        UisAppHeader { app_name, app_version, bare: true,
            {header_start}
            div { class: "uis-barh-grp",
                button {
                    class: "uis-btn ico",
                    title: "{s_main_add_files}",
                    "aria-label": "{s_main_add_files}",
                    onclick: pick_files,
                    Icon { width: 14, height: 14, icon: LdFilePlus }
                }
            }
        }
        LbsDropZone {
            class: "uis-app-main",
            label: drop_label,
            on_files: move |dropped: Vec<(String, String)>| {
                add_paths(dropped.into_iter().map(|(_, p)| PathBuf::from(p)).collect());
            },
            {children}
        div { class: "converter-body",
        div { class: match (empty_list, folded()) {
            (true, _) => "converter-main is-empty",
            (false, true) => "converter-main is-folded",
            (false, false) => "converter-main",
        },
        if let Some(popup) = popup {
            FfmpegPopup { lang: lang.clone(), popup, on_close: move |_| dismissed.set(true) }
        }
        if empty_list {
            div { class: "work-empty", p { class: "wv-failure", "{drop_text}" } }
        }
        PreviewPane {
            state: pane_state,
            pos,
            visible: active,
            on_mark: {
                let standard = standard_keys.clone();
                move |mark: Mark| apply_mark(work_fx, &standard, mark)
            },
            drawer,
            folded,
            on_widget: {
                let standard = standard_keys.clone();
                move |edit: WidgetEdit| apply_widget(work_fx, &standard, edit)
            },
        }
        if drawer() == Drawer::Tone {
            ToneDrawer { list: work_fx, lang: lang.clone(), on_edit: { let standard = standard_keys.clone(); move |edit: WidgetEdit| apply_widget(work_fx, &standard, edit) }, mixed: mixed_now.differing.contains("equalizer") || mixed_now.partial.contains("equalizer") }
        } else if drawer() == Drawer::Compressor {
            CompressorDrawer { list: work_fx, lang: lang.clone(), on_edit: { let standard = standard_keys.clone(); move |edit: WidgetEdit| apply_widget(work_fx, &standard, edit) }, mixed: mixed_now.differing.contains("compressor") || mixed_now.partial.contains("compressor") }
        } else if drawer() == Drawer::Level {
            LevelDrawer { list: work_fx, lang: lang.clone(), on_edit: { let standard = standard_keys.clone(); move |edit: WidgetEdit| apply_widget(work_fx, &standard, edit) }, mixed: ["volume-gain", "volume-loudness", "volume-peak", "volume-replaygain"].iter().any(|k| mixed_now.differing.contains(*k) || mixed_now.partial.contains(*k)) }
        } else if drawer() == Drawer::Channels {
            ChannelsDrawer { list: work_fx, lang: lang.clone(), on_edit: { let standard = standard_keys.clone(); move |edit: WidgetEdit| apply_widget(work_fx, &standard, edit) }, mixed: mixed_now.differing.contains("channels") || mixed_now.partial.contains("channels") }
        } else {
        // the file list and the command share the height every drawer has
        div { class: "work-strip",
        // ⚠ CLAUDE: tabindex makes the list the thing a click focuses, which is what lets Ctrl+A reach it;
        // a click on the empty part clears the selection (a row's own click stops here).
        div {
            class: "uis-lvw-scroll converter-list work-list",
            tabindex: "0",
            onclick: move |_| selected.set(HashSet::new()),
            onkeydown: move |e| {
                let m = e.modifiers();
                let ctrl = m.contains(Modifiers::CONTROL) || m.contains(Modifiers::META);
                if ctrl && matches!(e.key(), Key::Character(ref c) if c.eq_ignore_ascii_case("a")) {
                    e.prevent_default();
                    selected.set(items.peek().iter().map(|i| i.id).collect());
                }
            },
            for item in items.read().iter().cloned() {
                {
                    let status = statuses.read().get(&item.id).cloned();
                    let active = status.as_ref().is_some_and(JobStatus::is_active);
                    let (badge, detail, state) = match &status {
                        None => (String::new(), String::new(), "idle"),
                        Some(JobStatus::Queued) => (t("msgMainStatusQueued"), String::new(), "idle"),
                        Some(JobStatus::Probing) => (t("msgMainStatusReading"), String::new(), "idle"),
                        Some(JobStatus::Analysing { percent }) => (format!("{} {}", t("msgMainStatusAnalysing"), percent_text(*percent)), String::new(), "run"),
                        Some(JobStatus::Running { percent }) => (percent_text(*percent), String::new(), "run"),
                        Some(JobStatus::Done { output }) => (t("msgMainStatusDone"), output.display().to_string(), "ok"),
                        Some(JobStatus::Skipped { output }) => (t("msgMainStatusSkipped"), output.display().to_string(), "idle"),
                        Some(JobStatus::Cancelled) => (t("msgMainStatusCancelled"), String::new(), "idle"),
                        Some(JobStatus::Failed { reason }) => (t("msgMainStatusFailed"), reason.clone(), "err"),
                    };
                    let id = item.id;
                    let path = item.path.display().to_string();
                    let tip = if active { t("tipMainCancel") } else { t("tipMainRemove") };
                    // The row's own choices as tags: the control's icon and the name chosen. What keeps everything as it is is
                    // left out, so a plain row shows only its preset.
                    let r = &item.run;
                    let preset_name = if r.preset_id == engine::preset::ORIGINAL_ID { t("lblMainPresetOriginal") } else { cfg.preset_by(&r.preset_id).name };
                    let mut tags: Vec<(TagKind, String, String)> = vec![(TagKind::Preset, preset_name, s_main_preset.clone())];
                    let cleanup = cfg.cleanup_by(&r.cleanup_id);
                    if cleanup.edits_tags() || cleanup.remove_cover {
                        tags.push((TagKind::Cleanup, cleanup.name.clone(), s_main_cleanup.clone()));
                    }
                    let effects = cfg.effects_of(r);
                    if !effects.items.is_empty() {
                        tags.push((TagKind::Effects, effects.name.clone(), s_main_effects.clone()));
                    }
                    if r.destination_id != SOURCE_ID {
                        tags.push((TagKind::Destination, cfg.destination_by(&r.destination_id).name, s_main_destination.clone()));
                    }
                    match r.overwrite {
                        Overwrite::Skip => {}
                        Overwrite::Overwrite => tags.push((TagKind::Overwrite, s_main_overwrite_overwrite.clone(), s_main_overwrite.clone())),
                        Overwrite::Rename => tags.push((TagKind::Overwrite, s_main_overwrite_rename.clone(), s_main_overwrite.clone())),
                    }
                    let row_class = if sel_now.contains(&id) { "uis-lvw-item converter-row active" } else { "uis-lvw-item converter-row" };
                    rsx! {
                        div {
                            key: "{id}",
                            class: "{row_class}",
                            onclick: move |e| {
                                e.stop_propagation();
                                let m = e.modifiers();
                                let ctrl = m.contains(Modifiers::CONTROL) || m.contains(Modifiers::META);
                                let ids: Vec<u64> = items.peek().iter().map(|i| i.id).collect();
                                if m.contains(Modifiers::SHIFT) {
                                    let from = (*anchor.peek()).unwrap_or(id);
                                    let mut set = if ctrl { selected.peek().clone() } else { HashSet::new() };
                                    set.extend(range_between(&ids, from, id));
                                    selected.set(set);
                                } else if ctrl {
                                    let mut set = selected.peek().clone();
                                    if !set.remove(&id) {
                                        set.insert(id);
                                    }
                                    selected.set(set);
                                    anchor.set(Some(id));
                                } else {
                                    selected.set(HashSet::from([id]));
                                    anchor.set(Some(id));
                                }
                            },
                            div { class: "uis-lvw-item-icon converter-cover-box",
                                Icon { width: 18, height: 18, icon: LdMusic }
                            }
                            div { class: "uis-lvw-item-body",
                                div { class: "uis-lvw-item-name", "{item.name}" }
                                div { class: "uis-lvw-item-sub", "{path}" }
                                div { class: "converter-tags",
                                    for (n, (kind, text, title)) in tags.into_iter().enumerate() {
                                        RunTag { key: "{n}", kind, text, title }
                                    }
                                }
                            }
                            if !badge.is_empty() {
                                span { class: "uis-lvw-item-badge converter-state converter-state-{state}", title: "{detail}", "{badge}" }
                            }
                            button {
                                class: "uis-btn ico subtle",
                                title: "{tip}",
                                "aria-label": "{tip}",
                                onclick: move |e| {
                                    e.stop_propagation();
                                    if active {
                                        RUNNER.cancel(id);
                                    } else {
                                        RUNNER.remove(id);
                                        items.write().retain(|i| i.id != id);
                                        selected.write().remove(&id);
                                    }
                                },
                                Icon { width: 14, height: 14, icon: LdX }
                            }
                        }
                    }
                }
            }
        }
        CommandPanel { text: command_text.clone(), failed: command_failed, lang: lang.clone() }
        }
        }
        }
        // The run's choices, one column, every row the same width. Tooltips name them (no labels).
        aside { class: "uis-pnl converter-panel work-panel uis-cmp-panel uis-cmp-side",
            UisFldLabelled { compact: true, label: s_main_preset.clone(), title: s_main_preset.clone(),
                select {
                    class: "uis-ddn-std",
                    title: "{s_main_preset}",
                    "aria-label": "{s_main_preset}",
                    value: "{shown_preset}",
                    onchange: move |e| {
                        let v = e.value();
                        settings.write().work.preset_id = v.clone();
                        write_selected(items, selected, statuses, |r| r.preset_id = v.clone());
                    },
                    if mx_preset { option { value: "", selected: true, disabled: true, hidden: true } }
                    for (id, name) in preset_options {
                        option { key: "{id}", value: "{id}", selected: id == shown_preset, "{name}" }
                    }
                }
                button {
                    class: "uis-btn ico",
                    title: "{s_main_edit_presets}",
                    "aria-label": "{s_main_edit_presets}",
                    onclick: move |_| on_manage.call(manager::PRESETS.to_owned()),
                    Icon { width: 14, height: 14, icon: LdVolume2 }
                }
            }
            UisFldLabelled { compact: true, label: s_lbl_tags.clone(), title: s_main_cleanup.clone(),
                select {
                    class: "uis-ddn-std",
                    title: "{s_main_cleanup}",
                    "aria-label": "{s_main_cleanup}",
                    value: "{shown_tags}",
                    onchange: move |e| {
                        let v = e.value();
                        settings.write().work.cleanup_id = v.clone();
                        write_selected(items, selected, statuses, |r| r.cleanup_id = v.clone());
                    },
                    if mx_tags { option { value: "", selected: true, disabled: true, hidden: true } }
                    for (id, name) in cleanup_options {
                        option { key: "{id}", value: "{id}", selected: id == shown_tags, "{name}" }
                    }
                }
                button {
                    class: "uis-btn ico",
                    title: "{s_main_edit_cleanups}",
                    "aria-label": "{s_main_edit_cleanups}",
                    onclick: move |_| on_manage.call(manager::CLEANUPS.to_owned()),
                    Icon { width: 14, height: 14, icon: LdEraser }
                }
            }
            div { class: "work-fx",
                div { class: "uis-fld-labelled-label wfx-label", title: "{s_main_effects}", "{s_lbl_effects}" }
                WorkEffects { list: work_fx, view: fx_view, lang: lang.clone(), mixed: mixed_now.clone(), touched }
            }
            UisFldLabelled { compact: true, label: s_lbl_destination.clone(), title: s_main_destination.clone(),
                select {
                    class: "uis-ddn-std",
                    title: "{s_main_destination}",
                    "aria-label": "{s_main_destination}",
                    // A new key rebuilds the box after the folder picker closes: cancelled, it must not go on
                    // showing the pick entry as if it were the choice.
                    key: "{pick_nonce}",
                    value: "{shown_dest}",
                    onchange: move |e| {
                        let chosen = e.value();
                        if chosen == PICK_ID {
                            // Owned by the app's window: opened from the drop-down's own pop-up, a dialog without
                            // an owner came up BEHIND the window.
                            let window = dioxus::desktop::window();
                            spawn(async move {
                                if let Some(dir) = rfd::AsyncFileDialog::new().set_parent(&window.window).pick_folder().await {
                                    let id = settings.write().destination_for_folder(dir.path());
                                    settings.write().work.destination_id = id.clone();
                                    write_selected(items, selected, statuses, |r| r.destination_id = id.clone());
                                }
                                pick_nonce += 1;
                            });
                        } else {
                            settings.write().work.destination_id = chosen.clone();
                            write_selected(items, selected, statuses, |r| r.destination_id = chosen.clone());
                        }
                    },
                    if mx_dest { option { value: "", selected: true, disabled: true, hidden: true } }
                    for (id, name) in dest_options {
                        option { key: "{id}", value: "{id}", selected: id == shown_dest, "{name}" }
                    }
                }
                button {
                    class: "uis-btn ico",
                    title: "{s_main_edit_destinations}",
                    "aria-label": "{s_main_edit_destinations}",
                    onclick: move |_| on_manage.call(manager::DESTINATIONS.to_owned()),
                    Icon { width: 14, height: 14, icon: LdFolder }
                }
            }
            UisFldLabelled { compact: true, label: s_lbl_overwrite.clone(), title: s_main_overwrite.clone(),
                select {
                    class: "uis-ddn-std",
                    title: "{s_main_overwrite}",
                    "aria-label": "{s_main_overwrite}",
                    value: if mx_over { "" } else { match overwrite { Overwrite::Skip => "skip", Overwrite::Overwrite => "overwrite", Overwrite::Rename => "rename" } },
                    onchange: move |e| {
                        let chosen = match e.value().as_str() {
                            "overwrite" => Overwrite::Overwrite,
                            "rename" => Overwrite::Rename,
                            _ => Overwrite::Skip,
                        };
                        settings.write().work.overwrite = chosen;
                        write_selected(items, selected, statuses, |r| r.overwrite = chosen);
                    },
                    if mx_over { option { value: "", selected: true, disabled: true, hidden: true } }
                    option { value: "skip", selected: !mx_over && overwrite == Overwrite::Skip, "{s_main_overwrite_skip}" }
                    option { value: "overwrite", selected: !mx_over && overwrite == Overwrite::Overwrite, "{s_main_overwrite_overwrite}" }
                    option { value: "rename", selected: !mx_over && overwrite == Overwrite::Rename, "{s_main_overwrite_rename}" }
                }
            }
            // The panel and the selected files are one setting (`sync_panel`): a change is written to the selected files at once,
            // so there is no Apply and what the panel shows is what Convert runs.
            div { class: "converter-panel-run uis-cmp-run",
                div { class: "uis-barh-grp",
                    select {
                        class: "uis-ddn-std",
                        title: "{s_main_parallel}",
                        "aria-label": "{s_main_parallel}",
                        value: "{parallel}",
                        onchange: move |e| {
                            if let Ok(n) = e.value().parse::<u32>() {
                                settings.write().work.parallel = n.clamp(1, crate::settings::max_parallel());
                            }
                        },
                        for n in 1..=crate::settings::max_parallel() {
                            option { key: "{n}", value: "{n}", selected: n == parallel, "{n}" }
                        }
                    }
                    if busy {
                        button {
                            class: "uis-btn ico",
                            title: "{s_main_stop}",
                            "aria-label": "{s_main_stop}",
                            onclick: stop,
                            Icon { width: 14, height: 14, icon: LdSquare }
                        }
                    }
                    button {
                        class: "uis-btn ico",
                        title: "{s_main_convert}",
                        "aria-label": "{s_main_convert}",
                        disabled: ffmpeg_missing || !have_items || busy || all_idle_and_plain,
                        onclick: convert,
                        Icon { width: 14, height: 14, icon: LdPlay }
                    }
                }
                button {
                    class: "uis-btn ico",
                    title: "{s_main_open_output}",
                    "aria-label": "{s_main_open_output}",
                    disabled: open_target.is_none(),
                    onclick: move |_| {
                        if let Some(p) = &open_target {
                            let _ = open::that(p);
                        }
                    },
                    Icon { width: 14, height: 14, icon: LdFolderOpen }
                }
            }
        }
        }
        }
        div { class: "uis-barh-footer",
            span { class: "converter-status", "{status_text}" }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::effects::Item as Fx;
    use serde_json::json;

    fn run_with(effects: Vec<Fx>, tags: &str) -> RunChoice {
        let mut fx = EffectsEntry::new("");
        fx.items = effects;
        RunChoice { preset_id: String::new(), cleanup_id: tags.to_owned(), effects_id: String::new(), effects_inline: Some(fx), destination_id: String::new(), overwrite: Overwrite::Skip }
    }

    fn keys_of(run: RunChoice) -> Vec<String> {
        run.effects_inline.unwrap().items.iter().map(|i| i.key.clone()).collect()
    }

    #[test]
    fn a_panel_change_reaches_a_file_as_that_control_only() {
        let base = run_with(vec![], "");
        let panel = run_with(vec![Fx::new("fade")], "");
        let mut own = run_with(vec![Fx::new("normalize")], "strip");
        write_change(&mut own, &base, &panel);
        assert_eq!(own.cleanup_id, "strip");
        let keys = keys_of(own);
        assert!(keys.contains(&"fade".to_owned()) && keys.contains(&"normalize".to_owned()));
    }

    #[test]
    fn the_files_disagree_only_where_they_differ() {
        let one = run_with(vec![Fx::new("fade"), Fx::new("normalize")], "");
        let two = run_with(vec![Fx::new("fade")], "strip");
        let m = mixed_of(&[&one, &two]);
        assert!(m.fields.contains("tags") && !m.fields.contains("preset"));
        assert!(m.partial.contains("normalize") && !m.partial.contains("fade"));
        assert!(mixed_of(&[&one]).fields.is_empty());
    }

    #[test]
    fn a_touched_control_reaches_the_files_even_when_it_sets_the_same_value() {
        let panel = run_with(vec![Fx::with("fade", &[("length", json!(0.5))])], "");
        let mut own = run_with(vec![Fx::with("fade", &[("length", json!(0.9))])], "");
        let forced: HashSet<String> = ["fade.length".to_owned()].into();
        write_forced(&mut own, &panel, &forced);
        assert_eq!(own.effects_inline.unwrap().items[0].values.get("length"), Some(&json!(0.5)));
    }

    #[test]
    fn a_removed_effect_leaves_only_the_files_that_had_it() {
        let base = run_with(vec![Fx::new("fade")], "");
        let panel = run_with(vec![], "");
        let mut own = run_with(vec![Fx::new("fade"), Fx::new("normalize")], "");
        write_change(&mut own, &base, &panel);
        assert_eq!(keys_of(own), vec!["normalize".to_owned()]);
    }
}
