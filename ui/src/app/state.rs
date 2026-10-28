use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use io::{
    aedat4_decoder::Aedat4Decoder,
    codec::{Decoder, Packet},
};

use crate::cache::checkpoint::Checkpoint;

pub struct AppState {
    checkpoints: Arc<RwLock<HashMap<u64, Checkpoint>>>,
    file_state: Option<Arc<Mutex<Aedat4Decoder>>>,
}

/// Heuristic for the number of checkpoints to generate
/// This will be varied to find the optimal number of checkpoints
const NUM_CHECKPOINTS: i64 = 20;

impl AppState {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            file_state: None,
        }))
    }

    pub async fn set_file_state(&mut self, mut decoder: Aedat4Decoder) {
        println!("setting file state");
        let checkpoints = self.generate_checkpoints(&mut decoder).await;
        self.file_state = Some(Arc::new(Mutex::new(decoder)));
        self.checkpoints
            .write()
            .expect("checkpoint cache lock poisoned")
            .extend(checkpoints.clone());
        println!("checkpoints: {:?}", checkpoints);
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

        let mut zeroes = Vec::new();
        zeroes.resize(width as usize * height as usize, 0u64);

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
            let mut last_pol_offs = Vec::new();
            last_pol_offs.resize(width as usize * height as usize, 0u64);
            let mut last_pol_ons = Vec::new();
            last_pol_ons.resize(width as usize * height as usize, 0u64);

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
        let state = AppState::new();
        let state = state.lock().unwrap();
        assert!(state.file_state.is_none());
        assert!(state.checkpoints.read().unwrap().is_empty());
    }

    #[tokio::test]
    async fn replaces_the_current_file() {
        let first = decoder(10).await;
        let second = decoder(20).await;
        let state = AppState::new();
        let mut state = state.lock().unwrap();
        state.set_file_state(first);
        let previous = Arc::clone(state.file_state.as_ref().unwrap());

        state.set_file_state(second);

        let current = state.file_state.as_ref().unwrap();
        assert!(!Arc::ptr_eq(&previous, current));
        assert_eq!(current.lock().unwrap().file_data_table.total_events, 20);
    }

    #[tokio::test]
    async fn creates_checkpoints_on_open() {
        panic!("unimplemented");
    }

    #[tokio::test]
    async fn changes_checkpoints_when_opening_new_file() {
        panic!("unimplemented");
    }
}
