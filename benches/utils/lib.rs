use std::path::PathBuf;
use std::sync::Arc;

use event_camera_editor_ui::AppState;
use event_camera_editor_ui::ImageUpdate;
use io::aedat4_decoder::Aedat4;
use io::codec::DecoderFactory;
use tokio::sync::Mutex;

/// Setup app state for benchmarking the app's performance
pub async fn setup_state(file: PathBuf) -> (Arc<Mutex<AppState>>, tokio::sync::mpsc::Receiver<ImageUpdate>) {
    let (render_tx, render_rx) = tokio::sync::mpsc::channel::<ImageUpdate>(16);
    let state = AppState::new(render_tx);
    let file = tokio::fs::File::open(&file)
        .await
        .map_err(|err| format!("could not open file: {} - {err}", file.display())).unwrap();

    let decoder = Aedat4
        .open(file)
        .await
        .map_err(|_err| "could not decode file".to_string()).unwrap();

    state.lock().await.set_file_state(*decoder).await;

    (state, render_rx)
}
