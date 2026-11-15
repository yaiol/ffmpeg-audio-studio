//! The player of the converted files: a row's play icon opens it on that file, and when a file ends the next converted one of the
//! same list follows. What is drawn is the catalog's `UisPlayer`; this file holds the playing (the sound and its position).
//! The same pane as Media Downloader's `player.rs` without the picture of a video: a change to one is made to the other.

use crate::{
    catalog::{UisPlayer, UisPlayerLabels},
    i18n,
};
use dioxus::prelude::*;
use engine::player::Sound;
use std::{path::PathBuf, time::Duration};

/// A converted file the player can play.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
}

impl Track {
    /// The converted file at `path`, shown by its file name.
    pub fn of(path: &std::path::Path) -> Track {
        Track { path: path.to_path_buf(), title: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default() }
    }
}

/// What the rows and the pane share: the file on the player (none = no player), whether it is meant to be sounding, and where it is
/// in seconds.
#[derive(Clone, Copy, PartialEq)]
pub struct Player {
    pub track: Signal<Option<Track>>,
    pub playing: Signal<bool>,
    pub position: Signal<f64>,
}

impl Player {
    pub fn new() -> Self {
        Self { track: use_signal(|| None), playing: use_signal(|| false), position: use_signal(|| 0.0) }
    }

    /// The play icon of a row: the file on the player is paused or resumed, any other file takes the player from its start.
    pub fn play(mut self, track: Track) {
        if self.track.peek().as_ref().is_some_and(|t| t.path == track.path) {
            let now = *self.playing.peek();
            self.playing.set(!now);
        } else {
            self.go_to(track);
        }
    }

    /// Put `track` on the player and start it from its beginning.
    fn go_to(mut self, track: Track) {
        self.position.set(0.0);
        self.track.set(Some(track));
        self.playing.set(true);
    }

    /// A file is on the player (the pane is open).
    pub fn is_open(&self) -> bool {
        self.track.read().is_some()
    }

    /// Is this file the one sounding now?
    pub fn is_sounding(&self, path: &std::path::Path) -> bool {
        *self.playing.read() && self.track.read().as_ref().is_some_and(|t| t.path == path)
    }
}

/// The file after `current` in `queue`, and the one before it.
fn neighbours(queue: &[Track], current: Option<&Track>) -> (Option<Track>, Option<Track>) {
    let Some(i) = current.and_then(|c| queue.iter().position(|q| q.path == c.path)) else { return (None, None) };
    (i.checked_sub(1).and_then(|p| queue.get(p).cloned()), queue.get(i + 1).cloned())
}

/// `away`: the view shown is not the one the file belongs to. The pane is then not drawn and the file is paused, and it comes back
/// where it was when its own view is shown again.
#[component]
pub fn PlayerPane(player: Player, queue: ReadSignal<Vec<Track>>, lang: String, away: bool) -> Element {
    let t = |key: &str| i18n::t(&lang, key);
    let Player { mut track, mut playing, mut position } = player;
    let mut sound = use_signal(|| None::<Sound>);
    let mut length = use_signal(|| 0.0_f64);
    let mut volume = use_signal(|| 1.0_f32);
    // Bumped by a seek, so the sound starts again from the new place.
    let mut seeked = use_signal(|| 0u32);
    let mut failure = use_signal(String::new);

    // The other view is shown: the file waits, paused, until its own view is back.
    use_effect(use_reactive!(|away| {
        if away && *playing.peek() {
            playing.set(false);
        }
    }));

    // The file changed: its length.
    use_effect(move || {
        // The tag reader knows most formats; ffmpeg is asked for the rest (a raw AC-3 or AAC stream, a Wave64).
        let long = track().and_then(|t| {
            engine::player::duration(&t.path).or_else(|| {
                let ff = engine::ffmpeg::locate(engine::settings::ffmpeg_dir().as_deref())?;
                let us = engine::ffmpeg::probe(&ff, &t.path).ok()?.duration_us;
                (us > 0).then(|| us as f64 / 1e6)
            })
        });
        length.set(long.unwrap_or(0.0));
    });

    // What is meant to sound against what does: start it from where it is, or stop it where it got to.
    let lang_fx = lang.clone();
    use_effect(move || {
        let (t, want, _) = (track(), playing(), seeked());
        sound.write().take();
        let Some(t) = t else { return };
        if !want {
            return;
        }
        let Some(ff) = engine::ffmpeg::locate(engine::settings::ffmpeg_dir().as_deref()) else {
            failure.set(i18n::t(&lang_fx, "msgStgFfmpegNone"));
            playing.set(false);
            return;
        };
        match Sound::start(&ff, &t.path, *position.peek(), *volume.peek()) {
            Ok(s) => {
                failure.set(String::new());
                sound.set(Some(s));
            }
            Err(e) => {
                engine::log::error("player", &format!("{e}\ninput: {}", t.path.display()));
                failure.set(e);
                playing.set(false);
            }
        }
    });

    // Follow the playing: the position, and the end of the file (which hands the player to the next one).
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            let state = sound.peek().as_ref().map(|s| (s.position(), s.finished()));
            match state {
                Some((p, false)) => position.set(p),
                Some((_, true)) => {
                    let (_, next) = neighbours(&queue.peek(), track.peek().as_ref());
                    match next {
                        Some(n) => player.go_to(n),
                        None => {
                            position.set(0.0);
                            playing.set(false);
                        }
                    }
                }
                None => {}
            }
        }
    });

    let (prev, next) = neighbours(&queue.read(), track.read().as_ref());
    let labels = UisPlayerLabels {
        play: t("tipWorkPlay"),
        pause: t("tipWorkPause"),
        stop: t("tipMainStop"),
        previous: t("tipMainPrevious"),
        next: t("tipMainNext"),
        volume: t("tipWorkVolume"),
        close: t("tipMainPlayerClose"),
    };
    let title = track.read().as_ref().map(|t| t.title.clone()).unwrap_or_default();
    let (has_prev, has_next) = (prev.is_some(), next.is_some());
    let box_class = if away || !player.is_open() { "out-player out-player-away" } else { "out-player" };

    // The box is the app's (its padding and border); what is in it is the catalog's player.
    rsx! {
        div { class: box_class,
            UisPlayer {
                title,
                error: failure(),
                position: position(),
                length: length(),
                volume: volume(),
                playing: playing(),
                has_prev,
                has_next,
                labels,
                on_seek: move |v: f64| {
                    position.set(v);
                    seeked += 1;
                },
                on_toggle: move |_| {
                    if let Some(s) = sound.peek().as_ref() {
                        position.set(s.position());
                    }
                    let now = *playing.peek();
                    playing.set(!now);
                },
                on_stop: move |_| {
                    position.set(0.0);
                    playing.set(false);
                },
                on_prev: move |_| {
                    if let Some(p) = prev.clone() {
                        player.go_to(p);
                    }
                },
                on_next: move |_| {
                    if let Some(n) = next.clone() {
                        player.go_to(n);
                    }
                },
                on_volume: move |v: f32| {
                    volume.set(v);
                    if let Some(s) = sound.peek().as_ref() {
                        s.set_volume(v);
                    }
                },
                on_close: move |_| {
                    playing.set(false);
                    track.set(None);
                },
            }
        }
    }
}
