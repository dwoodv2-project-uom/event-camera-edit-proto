use std::{cmp::{max, min}, sync::Arc, time::Duration};
use tokio::{
    sync::{Mutex, watch},
    time::{MissedTickBehavior, interval},
};

#[derive(PartialEq, Eq, Debug)]
pub enum PlaybackStatus {
    NoFile,
    Play,
    Pause,
    Seek,
}

pub enum PlaybackEvent {
    OpenFileEvent {
        start_timestamp: u64,
        end_timestamp: u64,
    },
    PressPlayEvent,
    PressPauseEvent,
    EofEvent,
    SeekEvent {
        timestamp: u64,
    },
    StopSeekEvent,
    SkipForwardEvent,
    SkipBackwardEvent,
}

#[derive(Debug)]
pub struct Playback {
    pub state: PlaybackStatus,
    pub current_timestamp: u64,
    pub start_timestamp: u64,
    pub end_timestamp: u64,
    pub duration: u64,
    pub speed: f64,
    pub fps: f64,
    //pub visualisation: VisualisationState,
}

pub enum PlaybackEffect {
    RenderAt { timestamp: u64 },
}

/// Playback is a finite state machine dictated by a PlaybackEvent.
/// The desired FPS is 60 but it might be lower until we optimize.
///                                                    |<---------|
/// [Pause] ----- PressPlayEvent ------>           [Play]      TickEvent
///   ^  ^                                         | | |--------->|
///   |  | --EofEvent, PressPauseEvent------------ | |
///   |  | ----- SeekEvent ---> [Seek]<--SeekEvent---|
///   |-----------StopSeekEvent --|
impl Default for Playback {
    fn default() -> Self {
        Self::new()
    }
}

impl Playback {
    pub fn new() -> Self {
        Self {
            state: PlaybackStatus::NoFile,
            current_timestamp: 0,
            start_timestamp: 0,
            end_timestamp: 0,
            duration: 0,
            speed: 1.0,
            //visualisation: PreviewState::default(),
            fps: 60.0, // 60 FPS is target for our project
        }
    }

    /// Handles a event from the UI and changes the state of the playback.
    /// Returns a `PlaybackEffect` if applicable which is handled by the renderer
    pub fn handle_event(&mut self, event: PlaybackEvent) -> Option<PlaybackEffect> {
        match event {
            PlaybackEvent::OpenFileEvent {
                start_timestamp,
                end_timestamp,
            } => {
                self.state = PlaybackStatus::Pause;
                self.start_timestamp = start_timestamp;
                self.end_timestamp = end_timestamp;
                self.duration = end_timestamp - start_timestamp;
                self.current_timestamp = start_timestamp;
                Some(PlaybackEffect::RenderAt {
                    timestamp: start_timestamp,
                })
            }
            PlaybackEvent::PressPlayEvent => {
                if self.current_timestamp >= self.end_timestamp {
                    self.current_timestamp = self.start_timestamp;
                }
                self.state = PlaybackStatus::Play;
                Some(PlaybackEffect::RenderAt {
                    timestamp: self.current_timestamp,
                })
            }
            PlaybackEvent::PressPauseEvent => {
                self.state = PlaybackStatus::Pause;
                None // stop rendering when paused
            }
            PlaybackEvent::SkipForwardEvent => {
                self.state = PlaybackStatus::Pause;
                self.current_timestamp = min(self.current_timestamp + 100000, self.end_timestamp); // 100ms for now
                Some(PlaybackEffect::RenderAt {
                    timestamp: self.current_timestamp,
                })
            }
            PlaybackEvent::SkipBackwardEvent => {
                self.state = PlaybackStatus::Pause;
                self.current_timestamp = max(self.current_timestamp - 100000, self.start_timestamp);
                Some(PlaybackEffect::RenderAt {
                    timestamp: self.current_timestamp,
                })
            }
            PlaybackEvent::SeekEvent { timestamp } => {
                self.current_timestamp = timestamp;
                Some(PlaybackEffect::RenderAt {
                    timestamp: self.current_timestamp,
                })
            }
            PlaybackEvent::EofEvent => {
                self.state = PlaybackStatus::Pause;
                self.current_timestamp = self.end_timestamp;
                None // stop rendering when EOF
            }
            _ => None,
        }
    }

    /// Ticker which activates every frame to advance the timestamp and send it to the UI.
    /// Idles when not playing.
    pub async fn tick(playback: Arc<Mutex<Self>>, timestamp_tx: watch::Sender<(u64, u64)>) {
        let (period, step) = {
            let playback = playback.lock().await;
            assert!(playback.fps > 0.0, "fps must be greater than 0.0");
            (
                Duration::from_secs_f64(1.0 / playback.fps),
                (1_000_000.0 / playback.fps).round() as u64,
            )
        };

        let mut ticker = interval(period);
        // after a pause, skip missed frames instead of bursting to catch up.
        ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

        loop {
            ticker.tick().await;

            let timestamp = {
                let mut playback = playback.lock().await;
                if playback.state != PlaybackStatus::Play {
                    None
                } else {
                    let next = playback.current_timestamp + step;
                    if next >= playback.end_timestamp {
                        playback.handle_event(PlaybackEvent::EofEvent);
                    } else {
                        playback.current_timestamp = next;
                    }
                    Some(playback.current_timestamp)
                }
            };

            let offset = {
                if let Some(timestamp) = timestamp {
                    let playback = playback.lock().await;
                    Some(timestamp.saturating_sub(playback.start_timestamp))
                } else {
                    None
                }
            };

            if let Some(timestamp) = timestamp && let Some(offset) = offset {
                // send update tx to thread which updates the UI
                if timestamp_tx.send((timestamp, offset)).is_err() {
                    return;
                }
            }
        }
    }
}
