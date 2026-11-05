//! What the work view tells its preview pane: the file and the effects, and the marks the user sets in the pane, which the work
//! view writes into the "keep a range" effect of its list.

use engine::{effects::EffectsEntry, ffmpeg_effects::Values};
use std::path::PathBuf;

/// What the user set with the buttons of the preview. The times are in the range effect's own count (seconds on what the
/// effects above it leave), so the work view writes them as they are.
#[derive(Clone, Debug, PartialEq)]
pub enum Mark {
    /// The kept range starts here.
    Start(f64),
    /// The kept range ends here.
    End(f64),
    /// Keep the whole file.
    Clear,
}

/// What the user did with a drawer: the effect it belongs to and the values of its controls now (every one, the way an effect form writes
/// them); none = back to the defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetEdit {
    pub key: String,
    pub values: Option<Values>,
    /// The effect stays in the list even at its defaults (the drawer chose it); otherwise it leaves the list while every value is neutral.
    pub keep: bool,
}

/// What stands in the place of the file list, under the waveform: the files, or one of the drawers (one at a time).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Drawer {
    #[default]
    Files,
    Tone,
    Compressor,
    Level,
    Channels,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    /// The file shown: the first selected one in the work view.
    pub source: Option<PathBuf>,
    /// The work view's effects, heard as they stand.
    pub effects: EffectsEntry,
    pub lang: String,
}
