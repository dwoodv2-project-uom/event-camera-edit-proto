use std::{
    collections::HashMap, sync::{Arc, RwLock},
};

use io::{
    aedat4_decoder::Aedat4Decoder,
    codec::{Decoder, Packet},
};

// TODO: refactor to std::sync::Mutex, it is much faster than tokio::sync::Mutex
use tokio::sync::{Mutex};

use crate::{
    app::{playback::{Playback, PlaybackEffect, PlaybackEvent}, render::{ImageUpdate, Renderer, VisualisationMode, VisualisationState}}, cache::checkpoint::Checkpoint,
};

pub struct AppState {
    checkpoints: Arc<RwLock<HashMap<u64, Checkpoint>>>,
    pub file_state: Option<Arc<Mutex<Aedat4Decoder>>>,
    pub playback_state: Arc<Mutex<Playback>>,
    pub visualisation_state: Arc<Mutex<VisualisationState>>,
    pub render_tx: tokio::sync::mpsc::Sender<ImageUpdate>,
}

/// Heuristic for the number of checkpoints to generate
/// This will be varied to find the optimal number of checkpoints
const NUM_CHECKPOINTS: i64 = 20;

impl AppState {
    pub fn new(tx: tokio::sync::mpsc::Sender<ImageUpdate>) -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            file_state: None,
            playback_state: Arc::new(Mutex::new(Playback::new())),
            visualisation_state: Arc::new(Mutex::new(VisualisationState::default())),
            render_tx: tx,
        }))
    }

    pub async fn set_file_state(&mut self, mut decoder: Aedat4Decoder) {
        let checkpoints = self.generate_checkpoints(&mut decoder).await;
        let fdt_view = &decoder.file_data_table.clone();
        self.file_state = Some(Arc::new(Mutex::new(decoder)));
        self.checkpoints
            .write()
            .expect("checkpoint cache lock poisoned")
            .extend(checkpoints.clone());
        self.handle_playback_event(PlaybackEvent::OpenFileEvent { start_timestamp: fdt_view.ts_start as u64, end_timestamp: fdt_view.ts_end as u64}).await;
    }

    pub async fn get_decoder(&self) -> Option<Arc<Mutex<Aedat4Decoder>>> {
        self.file_state.clone()
    }

    pub async fn handle_playback_event(&mut self, playback: PlaybackEvent) -> Option<slint::Image> {
        let effect = {
            let mut playback_state = self.playback_state.lock().await;
            playback_state.handle_event(playback)
        };
        match effect {
            Some(PlaybackEffect::RenderAt { timestamp }) => {
                let width = {
                    let decoder = self.get_decoder().await?;
                    let decoder = decoder.lock().await;
                    decoder.metadata
                        .as_ref()?
                        .output_modules[0]
                        .output_info
                        .size_x
                        .unwrap_or(0)
                };

                let height = {
                    let decoder = self.get_decoder().await?;
                    let decoder = decoder.lock().await;
                    decoder.metadata
                        .as_ref()?
                        .output_modules[0]
                        .output_info
                        .size_y
                        .unwrap_or(0)
                };

                let visualisation = self.visualisation_state.lock().await;
                visualisation.render(VisualisationMode::TimeSurface, timestamp, width, height, self).await;

                None
            }
            None => None,
        }
    }

    async fn generate_checkpoints(
        &mut self,
        decoder: &mut Aedat4Decoder,
    ) -> Vec<(u64, Checkpoint)> {
        let mut checkpoints = Vec::new();
        let min_ts = decoder.file_data_table.ts_start as u64;
        let max_ts = decoder.file_data_table.ts_end as u64;
        // TODO: find out better partitioning methods
        // E.g. via event density
        let step = (max_ts - min_ts) / NUM_CHECKPOINTS as u64;
        let metadata = decoder.metadata.clone().unwrap();
        let width = metadata.output_modules[0].output_info.size_x.unwrap_or(0);
        let height = metadata.output_modules[0].output_info.size_y.unwrap_or(0);

        let mut zeroes = vec![0; width as usize * height as usize];

        // first checkpoint is at the start of the file, so last_polarity_off/on
        // are all zero
        checkpoints.push((
            min_ts,
            Checkpoint {
                last_polarity_off: zeroes.clone(),
                last_polarity_on: zeroes.clone(),
            },
        ));

        for i in 1..NUM_CHECKPOINTS {
            let ts = min_ts + step * i as u64;
            let last_checkpoint = &checkpoints[(i - 1) as usize];
            // we just overwrite the last_polarity_off/on with the new values using the checkpoints we have made
            // this means we do not need to read the entire file up to 0 -> ts + step for each checkpoint
            let mut last_pol_offs = last_checkpoint.1.last_polarity_off.clone();
            let mut last_pol_ons = last_checkpoint.1.last_polarity_on.clone();

            // Time surface checkpoints:
            // make sure step - 1 to prevent overlap with next checkpoint
            let packets: Vec<Packet> = decoder
                .packets_by_ts_range(ts as i64, (ts + step - 1) as i64)
                .await
                .unwrap()
                .unwrap();

            let event_packets = packets.into_iter().filter_map(|packet| match packet {
                Packet::EventPacket(content) => Some(content),
                _ => None,
            });

            for event_packet in event_packets {
                let events = event_packet.as_events().unwrap_or_default();
                for event in events.iter().flatten() {
                    let idx = (event.y() as usize) * width as usize + event.x() as usize;
                    // update array with the new state of the last polarity off/on event
                    if event.polarity() {
                        last_pol_ons[idx] = event.ts() as u64;
                    } else {
                        last_pol_offs[idx] = event.ts() as u64;
                    }
                }
            }
            checkpoints.push((
                min_ts,
                Checkpoint {
                    last_polarity_off: last_pol_offs,
                    last_polarity_on: last_pol_ons,
                },
            ));
        }
        checkpoints
    }

    /// Get the leftmost checkpoint for a given timestamp.
    pub async fn get_left_most_checkpoint(&self, ts: u64) -> (u64, Checkpoint) {
        let checkpoints = self.checkpoints.read().unwrap();

        checkpoints
            .iter()
            .filter(|(timestamp, _)| **timestamp <= ts)
            .max_by_key(|(timestamp, _)| *timestamp)
            .map(|(&timestamp, checkpoint)| (timestamp, checkpoint.clone()))
            .unwrap()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use io::aedat4_decoder::FileDataTable;
    use tokio::{fs::File, io::BufReader};

    async fn decoder(total_events: i64) -> Aedat4Decoder {
        // TODO: replace with actual file as this just wires in "a file" to get the tests to pass
        let file = File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
            .await
            .expect("test file should be readable");
        Aedat4Decoder {
            file_data_table: FileDataTable {
                total_events,
                total_frames: 0,
                total_imu: 0,
                total_trig: 0,
                ts_start: 0,
                ts_end: 0,
                events: Vec::new(),
                frames: Vec::new(),
                imus: Vec::new(),
                trigs: Vec::new(),
            },
            buffered_reader: BufReader::new(file),
            metadata: None,
        }
    }
    #[test]
    fn starts_without_a_file() {
        let state = AppState::new(tokio::sync::mpsc::channel(10).0);
        let Ok(state) = state.try_lock() else {
            return
        };
        assert!(state.file_state.is_none());
        assert!(state.checkpoints.read().unwrap().is_empty());
    }

    #[tokio::test]
    async fn replaces_the_current_file() {
        let first = decoder(10).await;
        let second = decoder(20).await;
        let state = AppState::new(tokio::sync::mpsc::channel(10).0);
        let Ok(mut state) = state.try_lock() else {
            return
        };
        state.set_file_state(first).await;
        let previous = Arc::clone(state.file_state.as_ref().unwrap());

        state.set_file_state(second).await;

        let current = state.file_state.as_ref().unwrap();
        assert!(!Arc::ptr_eq(&previous, current));
        let Ok(current) = current.try_lock() else {
            return
        };
        assert_eq!(current.file_data_table.total_events, 20);
    }

    #[tokio::test]
    async fn creates_checkpoints_on_open() {
        panic!("unimplemented");
    }

    #[tokio::test]
    async fn changes_checkpoints_when_opening_new_file() {
        panic!("unimplemented");
    }

    #[tokio::test]
    async fn test_get_left_checkpoint() {
        let state = AppState::new(tokio::sync::mpsc::channel(10).0);
        let app = state.lock().await;

        {
            let mut playback = app.playback_state.lock().await;
            playback.start_timestamp = 0;
            playback.end_timestamp = 200;
        }

        let expected = Checkpoint {
            last_polarity_off: vec![1, 2, 3],
            last_polarity_on: vec![4, 5, 6],
        };

        {
            let mut cps = app.checkpoints.write().expect("checkpoint cache lock poisoned");
            cps.clear();
            cps.insert(
                0,
                Checkpoint {
                    last_polarity_off: vec![0],
                    last_polarity_on: vec![0],
                },
            );
            cps.insert(
                10,
                expected.clone(),
            );
            cps.insert(
                20,
                Checkpoint {
                    last_polarity_off: vec![20],
                    last_polarity_on: vec![20],
                },
            );
        }

        let (checkpoint_ts, got) = app.get_left_most_checkpoint(17).await; // should be the one with timestamp=10

        assert_eq!(got.last_polarity_off, expected.last_polarity_off);
        assert_eq!(got.last_polarity_on, expected.last_polarity_on);
        assert_eq!(checkpoint_ts, 10);
    }
}
