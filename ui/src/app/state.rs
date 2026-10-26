use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};

use io::aedat4_decoder::Aedat4Decoder;

use crate::cache::checkpoint::Checkpoint;

pub struct AppState {
    checkpoints: Arc<RwLock<HashMap<u64, Checkpoint>>>,
    file_state: Option<Arc<Mutex<Aedat4Decoder>>>,
}

impl AppState {
    pub fn new() -> Arc<Mutex<Self>> {
        Arc::new(Mutex::new(Self {
            checkpoints: Arc::new(RwLock::new(HashMap::new())),
            file_state: None,
        }))
    }

    pub fn set_file_state(&mut self, decoder: Aedat4Decoder) {
        self.checkpoints
            .write()
            .expect("checkpoint cache lock poisoned")
            .clear();
        self.file_state = Some(Arc::new(Mutex::new(decoder)));
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
