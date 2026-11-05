//! Effects: an audio chain applied while converting (see PLAN-effects.md).
//!
//! An [`EffectsEntry`] is a named setting like a tag-cleaning entry, chosen apart from the preset so one preset can be used with several
//! chains. It is a LIST of effects in the order the user put them, the same list as the FFmpeg Video Studio's: each item is one effect (a
//! hand-written effect's id, or `try:` and an ffmpeg filter's name) and the values of its controls. Being in the list means it is on;
//! removing it turns it off.
//!
//! The effects themselves are the catalogue installed with ffmpeg (`effects.json` in the pair's folder, see `download`; a form per effect,
//! read by `ffmpeg_effects`, the module shared with the FFmpeg Video Studio). No copy of it is embedded: without it no effect can be built.
//! Most effects are plain filters and are written in the order listed. A few need the file MEASURED first (where the sound starts and stops,
//! its loudness): they are the *measured* effects. The catalogue says which they are: an effect with a `measure` block names an operation
//! ([`Op`], the fixed list of what this app can do) and which of its controls feeds each input of it; no effect is named in this file. The
//! cuts act on the source before the filters whatever their place in the list; the volume acts where it is listed, on what the filters
//! above it left; the silence at the end comes last.
//!
//! The work is split in three. [`Analysis`] is what the measuring passes of ffmpeg report about a file (where the sound starts and stops,
//! its loudness, its peak, the ReplayGain figures); [`window`] decides which part of the source is kept; [`plan`] turns an entry and an
//! analysis into exact filter arguments, the length the output will have, and the tags to write. Nothing here runs ffmpeg: the queue does,
//! and hands the text over.

use crate::ffmpeg_effects::{self as fx, Catalog, Values, V};
use serde::{Deserialize, Serialize};
use serde_json::Value;
pub use crate::measured::*;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, RwLock},
};

/// What the app converts, for the catalogue's menu: the effects that work on it.
pub const WORKS: &str = "audio";

/// This app's id, as the catalogue names it (`hiddenIn`).
pub const APP: &str = "ffmpeg-audio-studio";

/// The id of the entry a fresh install starts with: an ordinary entry with no effect in it.
pub const STANDARD_ID: &str = "default-effects";


/// The catalogue as read, with the words of every language; `CATALOG` is the copy in the language of the window.
static BASE: RwLock<Option<Arc<Catalog>>> = RwLock::new(None);
static LANG: RwLock<String> = RwLock::new(String::new());
static CATALOG: RwLock<Option<Arc<Catalog>>> = RwLock::new(None);

fn publish() {
    let base = BASE.read().unwrap().clone();
    let lang = LANG.read().unwrap().clone();
    *CATALOG.write().unwrap() = base.map(|b| Arc::new(b.localized(&lang)));
}

/// The language the catalogue speaks (`fr`, `pt_BR`...): the window calls it with its own, at launch and on every change.
pub fn set_language(lang: &str) {
    {
        let mut l = LANG.write().unwrap();
        if *l == lang {
            return;
        }
        *l = lang.to_owned();
    }
    publish();
}

/// The catalogue installed with ffmpeg, when it is loaded (`load_installed`).
pub fn catalog() -> Option<Arc<Catalog>> {
    #[cfg(test)]
    test_fixture();
    CATALOG.read().unwrap().clone()
}

/// Read the catalogue from its text (the tests give it the fixture).
pub fn set_catalog_text(text: &str) -> Result<(), String> {
    let catalog = Catalog::from_json(text).map_err(|e| format!("the effects catalogue could not be read: {e}"))?;
    *BASE.write().unwrap() = Some(Arc::new(catalog));
    publish();
    Ok(())
}

/// The favourites live beside the catalogue of the installed pair.
fn load_favourites(dir: Option<&Path>) {
    FAVOURITES.load(dir.and_then(crate::download::catalogue_path).as_deref().and_then(Path::parent));
}

/// Load the catalogue of the pair installed in `dir`, or none when there is no pair there (or it cannot be read). Called at launch and
/// after every install.
#[cfg(not(debug_assertions))]
pub fn load_installed(dir: Option<&Path>) -> Result<(), String> {
    load_favourites(dir);
    let loaded = dir
        .and_then(crate::download::catalogue_path)
        .map(|path| std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display())).and_then(|text| set_catalog_text(&text)));
    match loaded {
        Some(result) => {
            if result.is_err() {
                *BASE.write().unwrap() = None;
                publish();
            }
            result
        }
        None => {
            *BASE.write().unwrap() = None;
            publish();
            Ok(())
        }
    }
}

/// A debug build (the "(Dev)" app) works on the catalogue of the source tree - the one the shared folder makes, which the tests also read - and not on the
/// one installed from the site, so an effect being written shows at once and nothing is published to try it. Never part of a release build.
#[cfg(debug_assertions)]
pub fn load_installed(dir: Option<&Path>) -> Result<(), String> {
    load_favourites(dir);
    set_catalog_text(include_str!("../testdata/effects.json"))
}

/// The tests' catalogue: a copy of the one the shared folder makes (never part of a build).
#[cfg(test)]
fn test_fixture() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| set_catalog_text(include_str!("../testdata/effects.json")).expect("the fixture reads"));
}

/// One effect of an entry: which (a hand-written effect's id, or `try:` and an ffmpeg filter's name) and the values of its controls.
/// A control with no value has its default.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Item {
    pub key: String,
    pub values: BTreeMap<String, Value>,
}

impl Item {
    pub fn new(key: &str) -> Item {
        Item { key: key.to_owned(), values: BTreeMap::new() }
    }

    pub fn with(key: &str, values: &[(&str, Value)]) -> Item {
        Item { key: key.to_owned(), values: values.iter().map(|(k, v)| ((*k).to_owned(), v.clone())).collect() }
    }

    /// The effect this item is, when the catalogue has it.
    pub fn effect(&self) -> Option<fx::Effect> {
        catalog()?.form(&self.key)
    }

    /// Whether one of the effect's steps, on the sound or on the picture, is the ffmpeg filter `name`.
    pub fn uses_filter(&self, name: &str) -> bool {
        self.effect().is_some_and(|e| e.steps.iter().chain(e.audio.iter()).flatten().any(|st| st.filter.as_deref() == Some(name)))
    }

    /// The values of every control: the defaults, then what the user set.
    pub fn all_values(&self) -> Values {
        let (Some(cat), Some(effect)) = (catalog(), self.effect()) else { return Values::new() };
        let mut v = fx::defaults(&effect, &cat.filters).unwrap_or_default();
        for (id, x) in &self.values {
            v.insert(id.clone(), V::from_json(x));
        }
        v
    }

    /// Keep the values as they are after an edit (the whole map: a value equal to its default is kept too).
    pub fn set_values(&mut self, values: &Values) {
        self.values = values.iter().map(|(k, v)| (k.clone(), v.to_json())).collect();
    }

    /// The title the user reads.
    pub fn title(&self) -> String {
        let Some(cat) = catalog() else { return self.key.clone() };
        match self.key.strip_prefix("try:") {
            Some(name) => cat.filters.get(name).and_then(|f| f.plain.clone()).unwrap_or_else(|| name.to_owned()),
            None => cat.effects.get(&self.key).and_then(|e| e.plain.clone()).unwrap_or_else(|| self.key.clone()),
        }
    }

    /// Every control the effect asks for that is mandatory has a value; an item that is not is left out of the command.
    pub fn is_ready(&self) -> bool {
        let (Some(cat), Some(effect)) = (catalog(), self.effect()) else { return false };
        let values = self.all_values();
        fx::resolve_controls(&effect, &cat.filters)
            .map(|cs| cs.iter().filter(|c| c.required).all(|c| values.get(&c.id).is_some_and(|v| !matches!(v, V::Null) && !v.js_string().trim().is_empty())))
            .unwrap_or(false)
    }
}


/// The order of the STANDARD effects (the catalogue's `standard` level), which the work view shows as rows and runs in: what cuts the
/// file first, then the processing, the volume, the fades, and last the silence and the tags. Fixed on purpose: the user has no order to
/// manage there (the volume effects are modes of one thing, so they sit together). A standard effect not named here comes after these, by
/// title.
pub const STANDARD_ORDER: &[&str] = crate::measured::SOUND_STANDARD_ORDER;

/// The catalogue's effect that keeps a range: the one the work view's cut marks write into.
pub const KEEP_RANGE: &str = "keep-range";

/// What the user chose to list in the work view, kept beside the catalogue (`favourites.json`).
static FAVOURITES: fx::favourites::Choice = fx::favourites::Choice::new("ffmpeg-audio-studio");

/// The effects the user has chosen to list (`None`: he has not chosen, and the catalogue's standard effects are listed).
pub fn favourites() -> Option<Vec<String>> {
    FAVOURITES.get()
}

/// Choose the effects to list in the work view (`None` takes the choice back).
pub fn set_favourites(keys: Option<Vec<String>>) -> Result<(), String> {
    FAVOURITES.set(keys)
}

/// An effect the app itself is wired to, so it is always listed: one drawn by a widget, or a hand-written effect the work view reads by key.
pub fn is_locked(entry: &fx::Entry) -> bool {
    entry.widget.is_some() || (STANDARD_ORDER.contains(&entry.key.as_str()) && !entry.key.starts_with("try:"))
}

/// Is this effect listed in the work view, given what the user chose (`None`: nothing): a locked one by the catalogue's level, the others by
/// the choice, or by the level when there is none.
fn is_listed(entry: &fx::Entry, chosen: Option<&[String]>) -> bool {
    if is_locked(entry) {
        return entry.level == "standard";
    }
    chosen.map_or(entry.level == "standard", |keys| keys.contains(&entry.key))
}

/// Is this effect listed in the work view now.
pub fn is_standard(entry: &fx::Entry) -> bool {
    is_listed(entry, FAVOURITES.get().as_deref())
}

/// The keys of the standard effects this catalogue offers, in the fixed order of the work view.
pub fn standard_keys(cat: &Catalog, system: &str) -> Vec<String> {
    standard_keys_by(cat, system, FAVOURITES.get().as_deref())
}

fn standard_keys_by(cat: &Catalog, system: &str, chosen: Option<&[String]>) -> Vec<String> {
    let mut entries: Vec<_> = cat.picker(system, WORKS).into_iter().filter(|e| is_listed(e, chosen)).collect();
    let rank = |key: &str| STANDARD_ORDER.iter().position(|k| *k == key).unwrap_or(STANDARD_ORDER.len());
    entries.sort_by(|a, b| rank(&a.key).cmp(&rank(&b.key)).then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase())));
    entries.into_iter().map(|e| e.key).collect()
}

/// A standard effect the catalogue draws with a widget of its own: the work view shows it as a drawer (a graph), never as a row, and it takes
/// no place in the "Add an effect" menu there.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetSpec {
    pub key: String,
    pub kind: String,
    /// Which control of the effect feeds each input of the widget.
    pub controls: BTreeMap<String, String>,
}

/// The widget effects this catalogue offers, in the fixed order of the standard effects.
pub fn widgets(cat: &Catalog, system: &str) -> Vec<WidgetSpec> {
    standard_keys(cat, system)
        .into_iter()
        .filter_map(|key| {
            let w = cat.effects.get(&key)?.widget.clone()?;
            Some(WidgetSpec { key, kind: w.kind, controls: w.controls })
        })
        .collect()
}

impl Item {
    /// Every control is at its default: the item does nothing, and a widget's item is left out of the list in that case.
    pub fn is_default(&self) -> bool {
        let (Some(cat), Some(effect)) = (catalog(), self.effect()) else { return false };
        self.all_values() == fx::defaults(&effect, &cat.filters).unwrap_or_default()
    }
}

/// A named chain.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EffectsEntry {
    pub id: String,
    pub name: String,
    /// In the order the user put them.
    pub items: Vec<Item>,
}

impl Default for EffectsEntry {
    fn default() -> Self {
        EffectsEntry { id: STANDARD_ID.to_owned(), name: String::new(), items: Vec::new() }
    }
}

impl EffectsEntry {
    /// The item of effect `key` takes `values` (put in the list when it is not there); with none, or with every value at its default, the
    /// effect leaves the list. Used by a widget, which is always there and only shows in the list once it changes something.
    pub fn set_widget(&mut self, key: &str, values: Option<&Values>) {
        let mut item = self.items.iter().find(|i| i.key == key).cloned().unwrap_or_else(|| Item::new(key));
        if let Some(v) = values {
            item.set_values(v);
        }
        self.items.retain(|i| i.key != key);
        if values.is_some() && !item.is_default() {
            self.items.push(item);
        }
    }

    /// The item of effect `key` takes `values` and stays in the list even when they are its defaults (an effect whose defaults still DO
    /// something: swap the channels, bring the loudness to -16); none takes it out. Used by a drawer that chooses an action.
    pub fn set_item(&mut self, key: &str, values: Option<&Values>) {
        let mut item = self.items.iter().find(|i| i.key == key).cloned().unwrap_or_else(|| Item::new(key));
        self.items.retain(|i| i.key != key);
        if let Some(v) = values {
            item.set_values(v);
            self.items.push(item);
        }
    }

    /// The same list with the standard effects first, each at its fixed place, and every other effect after them in the order it was
    /// added. A standard effect is listed once. The work view's list keeps this shape.
    pub fn arranged(mut self, standard: &[String]) -> EffectsEntry {
        let mut rows: Vec<Item> = Vec::new();
        for key in standard {
            if let Some(at) = self.items.iter().position(|i| &i.key == key) {
                rows.push(self.items.remove(at));
            }
        }
        // a second copy of a standard effect is dropped; the rest keep their order
        self.items.retain(|i| !standard.contains(&i.key));
        rows.extend(self.items);
        self.items = rows;
        self
    }

    /// The same list without the effects that decide how much of the file is kept after the silence is trimmed (the range, the limit, the
    /// silence at the end): what is left is where a time written in the range's boxes counts from.
    pub fn without_cuts(&self) -> EffectsEntry {
        let mut e = self.clone();
        e.items.retain(|i| !matches!(i.op(), Some(Op::CutRange | Op::CutLength | Op::PadSilence)));
        e
    }

    /// Mark the start and / or the end of the range kept, in seconds counted on what the effects above the range leave: the range
    /// effect is added when the list has none. A start that is not before the end clears the end (0 = the end), and the other way
    /// round, so the range is never one the conversion would turn into nothing.
    pub fn mark_range(&mut self, start: Option<f64>, end: Option<f64>) {
        let at = match self.items.iter().position(|i| i.key == KEEP_RANGE) {
            Some(at) => at,
            None => {
                self.items.push(Item::new(KEEP_RANGE));
                self.items.len() - 1
            }
        };
        let item = &mut self.items[at];
        let values = item.all_values();
        let number = |id: &str| values.get(id).map_or(0.0, V::to_number);
        let (mut from, mut to) = (number("from"), number("to"));
        if let Some(s) = start {
            from = s.max(0.0);
            if to > 0.0 && to <= from {
                to = 0.0;
            }
        }
        if let Some(e) = end {
            to = e.max(0.0);
            if to > 0.0 && from >= to {
                from = 0.0;
            }
        }
        let round = |x: f64| Value::from((x * 1000.0).round() / 1000.0);
        item.values.insert("from".to_owned(), round(from));
        item.values.insert("to".to_owned(), round(to));
    }

    /// Keep the whole file again: the range effect leaves the list.
    pub fn clear_range(&mut self) {
        self.items.retain(|i| i.key != KEEP_RANGE);
    }

    /// The entry a fresh install starts with.
    pub fn standard(name: &str) -> Self {
        EffectsEntry { name: name.to_owned(), ..EffectsEntry::default() }
    }

    pub fn new(name: &str) -> Self {
        EffectsEntry { id: crate::preset::new_id(), name: name.to_owned(), items: Vec::new() }
    }

    /// The same entry without an item that names no effect (what a file of an older build holds in place of one). An effect the catalogue lacks stays: another
    /// ffmpeg may bring it back, and the job says it is not available.
    pub fn normalized(mut self) -> Self {
        self.items.retain(|i| !i.key.is_empty());
        self
    }

    /// What is wrong with the order of the effects: one that runs before another which changes what it works on.
    pub fn order_problems(&self) -> Vec<String> {
        let Some(cat) = catalog() else { return Vec::new() };
        cat.order(&self.items.iter().filter(|i| i.is_ready()).map(|i| i.key.clone()).collect::<Vec<_>>()).problems
    }

}



#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_users_favourites_replace_the_standard_level_except_for_what_the_app_is_wired_to() {
        let cat = catalog().expect("the fixture reads");
        let by_level = standard_keys_by(&cat, "windows", None);
        assert!(by_level.contains(&"try:chorus".to_owned()) && by_level.contains(&"equalizer".to_owned()), "{by_level:?}");
        let chosen = vec!["try:flanger".to_owned()];
        let mine = standard_keys_by(&cat, "windows", Some(&chosen));
        assert!(mine.contains(&"try:flanger".to_owned()), "what he ticked is listed: {mine:?}");
        assert!(!mine.contains(&"try:chorus".to_owned()), "what he did not tick is not: {mine:?}");
        assert!(mine.contains(&"equalizer".to_owned()), "an effect the app is wired to is listed whatever he chose: {mine:?}");
        let none: Vec<String> = Vec::new();
        assert!(!standard_keys_by(&cat, "windows", Some(&none)).is_empty(), "an empty choice still leaves the wired effects");
    }

    #[test]
    fn marks_write_the_range_and_never_leave_one_that_keeps_nothing() {
        test_fixture();
        let mut e = EffectsEntry::new("work");
        e.mark_range(Some(10.0), None);
        assert_eq!(e.items.len(), 1, "the range effect is added by the first mark");
        assert_eq!(e.cut_range().map(|c| (c.from_s, c.to_s)), Some((10.0, 0.0)));
        e.mark_range(None, Some(25.5));
        assert_eq!(e.cut_range().map(|c| (c.from_s, c.to_s)), Some((10.0, 25.5)));
        e.mark_range(Some(30.0), None);
        assert_eq!(e.cut_range().map(|c| (c.from_s, c.to_s)), Some((30.0, 0.0)), "a start past the end clears the end");
        e.mark_range(None, Some(20.0));
        assert_eq!(e.cut_range().map(|c| (c.from_s, c.to_s)), Some((0.0, 20.0)), "an end before the start clears the start");
        e.clear_range();
        assert!(e.items.is_empty() && e.cut_range().is_none());
    }

    #[test]
    fn what_is_cut_after_the_silence_is_not_part_of_where_a_range_counts_from() {
        test_fixture();
        let mut e = EffectsEntry::new("work");
        e.items.push(Item::new("trim-silence"));
        e.mark_range(Some(5.0), Some(9.0));
        e.items.push(Item::new("limit-duration"));
        e.items.push(Item::new("silence-end"));
        let rest = e.without_cuts();
        assert_eq!(rest.items.len(), 1);
        assert_eq!(rest.items[0].key, "trim-silence");
    }

    #[test]
    fn a_language_without_translations_stays_english_and_one_with_them_changes_the_words() {
        let en = Catalog::from_json(include_str!("../testdata/effects.json")).unwrap();
        let same = en.localized("xx");
        assert_eq!(same.categories[0].title, en.categories[0].title);
        let Some((lang, t)) = en.i18n.iter().find(|(_, t)| !t.categories.is_empty()) else { return };
        let (id, text) = t.categories.iter().next().unwrap();
        let Some(title) = &text.title else { return };
        let fr = en.localized(lang);
        let cat = fr.categories.iter().find(|c| &c.id == id).unwrap();
        assert_eq!(&cat.title, title);
        assert!(fr.i18n.is_empty());
    }

    const TRIM_SILENCE: &str = "trim-silence";
    const KEEP_RANGE: &str = "keep-range";
    const LIMIT_DURATION: &str = "limit-duration";
    const VOLUME_LOUDNESS: &str = "volume-loudness";
    const VOLUME_PEAK: &str = "volume-peak";
    const VOLUME_REPLAYGAIN: &str = "volume-replaygain";
    const VOLUME_GAIN: &str = "volume-gain";
    const SILENCE_END: &str = "silence-end";
    const REPLAYGAIN_TAGS: &str = "replaygain-tags";
    const TEMPO: &str = "try:atempo";

    const OUT: &str = "\
[Parsed_silencedetect_0 @ 0000] silence_start: 3
[Parsed_replaygain_2 @ 0000] track_gain = +6.28 dB
[Parsed_replaygain_2 @ 0000] track_peak = 0.088367
[Parsed_silencedetect_0 @ 0000] silence_end: 6.5 | silence_duration: 3.5
[Parsed_volumedetect_1 @ 0000] max_volume: -18.1 dB
[Parsed_ebur128_1 @ 0000] Summary:

  Integrated loudness:
    I:         -22.0 LUFS
    Threshold: -32.1 LUFS

  Loudness range:
    LRA:         9.9 LU

  True peak:
    Peak:      -18.1 dBFS
";

    fn entry(items: Vec<Item>) -> EffectsEntry {
        EffectsEntry { items, ..EffectsEntry::new("t") }
    }

    fn song(duration_s: f64, start: Option<f64>, end: Option<f64>) -> Analysis {
        Analysis { duration_s, song_start_s: start, song_end_s: end, ..Analysis::default() }
    }

    fn silence_end(extra: &[(&str, Value)]) -> Item {
        Item::with(SILENCE_END, extra)
    }

    #[test]
    fn the_measuring_pass_is_read_into_numbers() {
        let a = parse_analysis(OUT, 6.5);
        assert_eq!(a.song_end_s, Some(3.0));
        assert_eq!(a.song_start_s, None);
        assert_eq!(a.loudness_lufs, Some(-22.0));
        assert_eq!(a.true_peak_db, Some(-18.1));
        assert_eq!(a.max_volume_db, Some(-18.1));
        assert_eq!(a.rg_gain_db, Some(6.28));
        assert_eq!(a.rg_peak, Some(0.088367));
    }

    #[test]
    fn a_silence_at_the_start_gives_the_start_of_the_sound() {
        let out = "silence_start: 0\nsilence_end: 1.5 | silence_duration: 1.5\nsilence_start: 4\nsilence_end: 5 | silence_duration: 1\n";
        let a = parse_analysis(out, 5.0);
        assert_eq!(a.song_start_s, Some(1.5));
        assert_eq!(a.song_end_s, Some(4.0));
    }

    #[test]
    fn a_silence_in_the_middle_is_not_the_end_of_the_song() {
        let out = "silence_start: 2\nsilence_end: 4 | silence_duration: 2\n";
        let a = parse_analysis(out, 10.0);
        assert_eq!(a.song_end_s, None, "silence that ends before the file does is part of the song");
        assert_eq!(a.song_start_s, None);
    }

    #[test]
    fn a_silence_that_is_never_closed_runs_to_the_end() {
        let a = parse_analysis("silence_start: 7.25\n", 9.0);
        assert_eq!(a.song_end_s, Some(7.25));
    }

    #[test]
    fn the_list_keeps_the_order_the_user_gave_and_survives_json() {
        let e = entry(vec![Item::new(REPLAYGAIN_TAGS), Item::with("sound-fade", &[("fade_out", json!(1.2))]), Item::new(TRIM_SILENCE), Item::new("")]);
        let e = e.normalized();
        let keys: Vec<&str> = e.items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, [REPLAYGAIN_TAGS, "sound-fade", TRIM_SILENCE], "an item naming no effect is dropped, the rest is not moved");
        let back: EffectsEntry = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        assert_eq!(back, e);
    }

    #[test]
    fn numbers_are_read_inside_their_range() {
        let e = entry(vec![silence_end(&[("seconds", json!(9999))]), Item::with(VOLUME_GAIN, &[("gain", json!(400))])]);
        assert_eq!(e.silence_end().unwrap().seconds, 60.0);
        assert_eq!(e.volume().unwrap().gain_db, 30.0);
        let twice = entry(vec![silence_end(&[("seconds", json!(3))]), silence_end(&[("seconds", json!(9))])]);
        assert_eq!(twice.silence_end().unwrap().seconds, 3.0, "the first of a kind wins");
    }

    #[test]
    fn a_file_saved_with_the_old_kinds_loses_them_and_keeps_the_rest() {
        let old = r#"{ "id": "x", "name": "Old", "items": [
            { "trim_silence": { "start": true } },
            { "key": "sound-fade", "values": { "fade_out": 1.5 } } ] }"#;
        let e: EffectsEntry = serde_json::from_str(old).unwrap();
        let e = e.normalized();
        let keys: Vec<&str> = e.items.iter().map(|i| i.key.as_str()).collect();
        assert_eq!(keys, ["sound-fade"], "an effect of the old shape names no effect now");
        assert_eq!((e.id.as_str(), e.name.as_str()), ("x", "Old"));
    }

    #[test]
    fn every_measured_effect_of_the_catalogue_names_an_operation_the_app_has() {
        let cat = catalog().unwrap();
        let measured: Vec<_> = cat.effects.iter().filter(|(_, e)| e.measure.is_some()).collect();
        assert!(!measured.is_empty());
        for (id, e) in measured {
            let m = e.measure.as_ref().unwrap();
            assert!(Op::named(&m.op).is_some(), "{id}: the app has no operation {}", m.op);
            for control in m.controls.values() {
                assert!(e.controls.iter().any(|c| &c.id == control), "{id}: no control {control}");
            }
        }
    }

    #[test]
    fn the_speed_comes_from_the_filter_the_catalogue_names() {
        let e = entry(vec![Item::with("try:atempo", &[("tempo", json!(2))]), Item::with("sound-fade", &[("fade_out", json!(2))])]);
        assert!((e.tempo_ratio() - 2.0).abs() < 1e-9);
        assert!((entry(vec![Item::with("sound-fade", &[("fade_out", json!(2))])]).tempo_ratio() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn the_silence_at_the_end_is_exactly_the_wanted_length() {
        let e = entry(vec![silence_end(&[])]);
        // Song to 3 s, a 3.5 s tail: cut at 3.05, then 1.95 s appended -> 3 s + 2 s.
        let p = plan(&e, &song(6.5, None, Some(3.0)), None);
        assert_eq!(p.filters[0], "atrim=start=0.000000:end=3.050000");
        assert_eq!(p.filters[2], "apad=pad_dur=1.950000");
        assert!((p.expected_s.unwrap() - 5.0).abs() < 1e-6);
        // A tail of 0.3 s is cut to the margin too.
        assert!((plan(&e, &song(3.3, None, Some(3.0)), None).expected_s.unwrap() - 5.0).abs() < 1e-6);
        // A file with no trailing silence: nothing to cut, the whole gap appended.
        let p = plan(&e, &song(3.0, None, None), None);
        assert_eq!(p.af().unwrap(), "apad=pad_dur=2.000000");
        assert!((p.expected_s.unwrap() - 5.0).abs() < 1e-6);
    }

    #[test]
    fn a_tail_shorter_than_the_margin_is_kept_whole() {
        let e = entry(vec![silence_end(&[("margin", json!(500))])]);
        let p = plan(&e, &song(3.2, None, Some(3.0)), None);
        assert!(p.filters.iter().all(|f| !f.starts_with("atrim")), "{:?}", p.filters);
        assert!((p.expected_s.unwrap() - (3.2 + 1.8)).abs() < 1e-6);
    }

    #[test]
    fn trimming_the_ends_keeps_the_sound_and_a_margin() {
        let e = entry(vec![Item::new(TRIM_SILENCE)]);
        let w = window(&e, &song(10.0, Some(1.5), Some(8.0)));
        assert!((w.start - 1.45).abs() < 1e-9 && (w.end - 8.05).abs() < 1e-9, "{w:?}");
        // Only the start, only the end.
        let only_start = entry(vec![Item::with(TRIM_SILENCE, &[("end", json!(false))])]);
        let w = window(&only_start, &song(10.0, Some(1.5), Some(8.0)));
        assert!((w.end - 10.0).abs() < 1e-9 && (w.start - 1.45).abs() < 1e-9);
        // A file that neither begins nor ends in silence is kept whole.
        let p = plan(&e, &song(10.0, None, None), None);
        assert!(p.af().is_none() && p.expected_s.is_none());
    }

    #[test]
    fn keep_a_range_and_the_limit_work_on_what_is_left_by_the_effects_before_them() {
        let trim = Item::with(TRIM_SILENCE, &[("margin", json!(0))]);
        let range = Item::with(KEEP_RANGE, &[("from", json!(2)), ("to", json!(10))]);
        let mut e = entry(vec![trim, range]);
        // Sound from 1 to 20: the range counts from 1 -> 3 to 11.
        let w = window(&e, &song(25.0, Some(1.0), Some(20.0)));
        assert_eq!((w.start, w.end), (3.0, 11.0));
        e.items.push(Item::with(LIMIT_DURATION, &[("seconds", json!(5))]));
        let w = window(&e, &song(25.0, Some(1.0), Some(20.0)));
        assert_eq!((w.start, w.end), (3.0, 8.0));
        // A range that falls outside what is left changes nothing.
        let out = entry(vec![Item::with(KEEP_RANGE, &[("from", json!(50)), ("to", json!(60))])]);
        let w = window(&out, &song(10.0, None, None));
        assert_eq!((w.start, w.end), (0.0, 10.0));
    }

    #[test]
    fn the_limit_shortens_and_the_silence_is_added_after_it() {
        let e = entry(vec![Item::new(LIMIT_DURATION), silence_end(&[])]);
        // A 4-minute song: 30 s kept, then the 2 s of silence.
        let p = plan(&e, &song(247.0, None, Some(240.0)), None);
        assert!((p.expected_s.unwrap() - 32.0).abs() < 1e-6, "{p:?}");
        assert!(p.filters[0].starts_with("atrim=start=0.000000:end=30.000000"));
    }

    #[test]
    fn the_equaliser_is_a_drawer_and_is_listed_only_while_a_band_is_off_zero() {
        let cat = catalog().expect("the fixture reads");
        let w = widgets(&cat, "windows").into_iter().find(|w| w.key == "equalizer").expect("the equaliser is a widget");
        assert_eq!(w.kind, "tone");
        let mut e = entry(vec![]);
        let mut v = Item::new("equalizer").all_values();
        v.insert("g1k".to_owned(), V::Num(4.0));
        e.set_widget("equalizer", Some(&v));
        assert_eq!(e.items.len(), 1, "a band off zero puts the effect in the list");
        let af = plan(&e, &song(60.0, None, None), None).af().unwrap();
        assert!(af.contains("equalizer=frequency=1000") && af.contains("gain=4"), "{af}");
        v.insert("g1k".to_owned(), V::Num(0.0));
        e.set_widget("equalizer", Some(&v));
        assert!(e.items.is_empty(), "every band back at zero takes it out again");
    }

    #[test]
    fn the_drawers_cover_the_equaliser_the_compressor_the_volume_the_channels_and_the_fade() {
        let cat = catalog().expect("the fixture reads");
        let kinds: Vec<(String, String)> = widgets(&cat, "windows").into_iter().map(|w| (w.key, w.kind)).collect();
        for (key, kind) in [("equalizer", "tone"), ("compressor", "compressor"), ("volume-gain", "level"), ("volume-loudness", "level"), ("volume-peak", "level"), ("volume-replaygain", "level"), ("channels", "channels"), ("sound-fade", "fades")] {
            assert!(kinds.contains(&(key.to_owned(), kind.to_owned())), "{key} is drawn as {kind}: {kinds:?}");
        }
    }

    #[test]
    fn an_effect_chosen_by_a_drawer_stays_in_the_list_at_its_defaults() {
        let mut e = entry(vec![]);
        let defaults = Item::new("channels").all_values();
        e.set_widget("channels", Some(&defaults));
        assert!(e.items.is_empty(), "a widget at its defaults leaves the list");
        e.set_item("channels", Some(&defaults));
        assert_eq!(e.items.len(), 1, "a chosen action stays, even when the way chosen is the default one");
        let af = plan(&e, &song(60.0, None, None), None).af().unwrap();
        assert!(af.contains("stereo|c0=c1|c1=c0"), "{af}");
        e.set_item("channels", None);
        assert!(e.items.is_empty());
    }

    #[test]
    fn fades_ride_on_the_kept_part() {
        let e = entry(vec![Item::new(LIMIT_DURATION), Item::with("sound-fade", &[("fade_in", json!(2)), ("fade_out", json!(2))])]);
        let af = plan(&e, &song(200.0, None, None), None).af().unwrap();
        assert!(af.contains("afade=type=in:start_time=0:duration=2"), "{af}");
        assert!(af.contains("afade=type=out:start_time=28:duration=2"), "{af}");
        // A fade longer than the song is the whole song.
        let short = entry(vec![Item::with("sound-fade", &[("fade_out", json!(9))])]);
        let af = plan(&short, &song(3.0, None, None), None).af().unwrap();
        assert!(af.contains("afade=type=out:start_time=0:duration=3"), "{af}");
    }

    #[test]
    fn each_volume_mode_computes_its_gain_and_only_the_peak_mode_has_no_limiter() {
        let a = Analysis { duration_s: 6.5, loudness_lufs: Some(-22.0), max_volume_db: Some(-12.0), rg_gain_db: Some(3.7), ..Analysis::default() };
        let p = plan(&entry(vec![Item::new(VOLUME_LOUDNESS)]), &a, None);
        assert_eq!(p.applied_gain_db, Some(6.0));
        assert!(p.af().unwrap().contains("volume=6.000dB,alimiter=limit=0.891251:level=0"));
        let p = plan(&entry(vec![Item::new(VOLUME_PEAK)]), &a, None);
        assert_eq!(p.applied_gain_db, Some(11.0));
        assert_eq!(p.af().unwrap(), "volume=11.000dB", "a peak target cannot clip, no limiter");
        let p = plan(&entry(vec![Item::new(VOLUME_REPLAYGAIN)]), &a, None);
        assert_eq!(p.applied_gain_db, Some(3.7));
        let album = Item::with(VOLUME_REPLAYGAIN, &[("album", json!(true))]);
        let p = plan(&entry(vec![album.clone()]), &a, Some(&Album { gain_db: -1.5, peak: 0.9 }));
        assert_eq!(p.applied_gain_db, Some(-1.5), "the album's gain when the album is asked for");
        let p = plan(&entry(vec![album]), &a, None);
        assert_eq!(p.applied_gain_db, Some(3.7), "no album found: the track's own");
        let p = plan(&entry(vec![Item::with(VOLUME_GAIN, &[("gain", json!(-4))])]), &a, None);
        assert_eq!(p.applied_gain_db, Some(-4.0));
    }

    #[test]
    fn the_volume_acts_where_it_is_listed() {
        let a = Analysis { duration_s: 6.5, max_volume_db: Some(-12.0), ..Analysis::default() };
        let above = entry(vec![Item::with("try:highpass", &[("frequency", json!(80))]), Item::new(VOLUME_PEAK)]);
        let below = entry(vec![Item::new(VOLUME_PEAK), Item::with("try:highpass", &[("frequency", json!(80))])]);
        let (up, down) = (plan(&above, &a, None).af().unwrap(), plan(&below, &a, None).af().unwrap());
        assert!(up.starts_with("highpass=frequency=80,volume="), "{up}");
        assert!(down.starts_with("volume=") && down.ends_with("highpass=frequency=80"), "{down}");
    }

    #[test]
    fn replaygain_tags_describe_the_output_file() {
        let mut e = entry(vec![Item::new(REPLAYGAIN_TAGS)]);
        let a = Analysis { duration_s: 6.5, rg_gain_db: Some(6.28), rg_peak: Some(0.088367), ..Analysis::default() };
        let p = plan(&e, &a, None);
        assert_eq!(p.tags[0], ("REPLAYGAIN_TRACK_GAIN".to_owned(), "+6.28 dB".to_owned()));
        assert_eq!(p.tags[1], ("REPLAYGAIN_TRACK_PEAK".to_owned(), "0.088367".to_owned()));
        assert_eq!(p.tags.len(), 2, "no album tags unless asked for");
        // With a gain applied the tag says what is LEFT to do, and the peak moved with it.
        e.items.push(Item::new(VOLUME_LOUDNESS));
        let a = Analysis { loudness_lufs: Some(-22.0), ..a };
        let p = plan(&e, &a, None);
        assert_eq!(p.tags[0].1, "+0.28 dB");
        let peak: f64 = p.tags[1].1.parse().unwrap();
        assert!((peak - 0.088367 * db_to_linear(6.0)).abs() < 1e-5);
        // The album's figures are written beside the track's, with the same gain taken off.
        e.items[0] = Item::with(REPLAYGAIN_TAGS, &[("album", json!(true))]);
        let p = plan(&e, &a, Some(&Album { gain_db: 5.0, peak: 0.2 }));
        assert_eq!(p.tags.len(), 4);
        assert_eq!(p.tags[2], ("REPLAYGAIN_ALBUM_GAIN".to_owned(), "-1.00 dB".to_owned()));
    }

    #[test]
    fn the_meter_runs_over_the_kept_part_only() {
        let e = entry(vec![Item::new(TRIM_SILENCE), Item::new(VOLUME_LOUDNESS)]);
        let a = song(10.0, Some(2.0), Some(8.0));
        let w = window(&e, &a);
        let f = meter_filter(&e, &w, 10.0);
        assert!(f.starts_with("atrim=start=1.950000:end=8.050000,asetpts=PTS-STARTPTS,volumedetect"), "{f}");
        let whole = entry(vec![Item::new(VOLUME_LOUDNESS)]);
        assert_eq!(meter_filter(&whole, &window(&whole, &a), 10.0), METER_FILTER);
        // The meters see what the volume will be set on: through the effects above it, not the raw cut, and not through those below.
        let p = entry(vec![Item::with("try:highpass", &[("frequency", json!(80))]), Item::new(VOLUME_LOUDNESS), Item::with("try:highpass", &[("frequency", json!(90))])]);
        assert_eq!(meter_filter(&p, &window(&p, &a), 10.0), format!("highpass=frequency=80,{METER_FILTER}"));
    }

    #[test]
    fn the_filters_are_written_in_the_order_listed() {
        let mut gains = serde_json::Map::new();
        gains.insert("g31".into(), json!(3));
        gains.insert("g1k".into(), json!(-4));
        let e = entry(vec![
            Item::with("channels", &[("mode", json!("swap"))]),
            Item::new("try:adeclick"),
            Item::with("try:highpass", &[("frequency", json!(40))]),
            Item { key: "equalizer".into(), values: gains.into_iter().collect() },
            Item::with("compressor", &[("threshold", json!(-18)), ("ratio", json!(4))]),
            Item::with(TEMPO, &[("tempo", json!(1.25))]),
        ]);
        let chain = e.try_chain(0..e.items.len(), 100.0).unwrap();
        let text = chain.join(" | ");
        let at = |n: &str| text.find(n).unwrap_or_else(|| panic!("{n} in {text}"));
        assert!(at("pan=") < at("adeclick") && at("adeclick") < at("highpass=frequency=40") && at("highpass") < at("equalizer") && at("equalizer") < at("acompressor") && at("acompressor") < at("atempo"), "{text}");
        assert!(text.contains("pan=args='stereo|c0=c1|c1=c0'"), "{text}");
        assert!(text.contains("threshold=-18dB") && text.contains("ratio=4"), "{text}");
        let reversed: Vec<Item> = e.items.iter().rev().cloned().collect();
        assert_ne!(chain, entry(reversed).try_chain(0..6, 100.0).unwrap(), "the order of the list is the order of the filters");
    }

    #[test]
    fn a_tempo_change_moves_the_length_the_fades_and_the_silence_follow() {
        let e = entry(vec![Item::with(TEMPO, &[("tempo", json!(2))]), Item::with("sound-fade", &[("fade_out", json!(1))]), silence_end(&[])]);
        // 20 s of sound twice as fast = 10 s; the fade-out is the last second of THOSE 10, then 2 s.
        let p = plan(&e, &song(20.0, None, None), None);
        let af = p.af().unwrap();
        assert!(af.contains("atempo=tempo=2"), "{af}");
        assert!(af.contains("afade=type=out:start_time=9:duration=1"), "{af}");
        assert!((p.expected_s.unwrap() - 12.0).abs() < 1e-6, "{p:?}");
    }

    #[test]
    fn an_effect_the_catalogue_lacks_is_named_and_a_missing_catalogue_is_not_a_crash() {
        let e = entry(vec![Item::new("gone")]);
        assert!(e.check().unwrap_err().contains("not available with this ffmpeg"));
        assert!(entry(vec![Item::new(VOLUME_GAIN)]).check().is_ok(), "a measured effect needs no filter of the catalogue");
    }

    #[test]
    fn an_empty_list_changes_nothing_and_needs_no_analysis() {
        let e = EffectsEntry::standard("No effects");
        assert!(!e.needs_analysis() && !e.changes_audio());
        let p = plan(&e, &Analysis { duration_s: 10.0, ..Analysis::default() }, None);
        assert!(p.af().is_none() && p.tags.is_empty() && p.expected_s.is_none());
        // A fixed gain and fades need no measuring at all.
        let g = entry(vec![Item::with("sound-fade", &[("fade_in", json!(2))]), Item::new(VOLUME_GAIN)]);
        assert!(!g.needs_analysis() && g.changes_audio());
    }
}
