// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
pub mod renderer;

use chrono::{DateTime, offset};
use renderer::Renderer;
use slint::{
    ToSharedString,
    wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu},
};
use tokio::sync::Mutex;

use std::{path::PathBuf, sync::Arc, time::SystemTime};

pub mod visualisations;
use visualisations::timeline;

pub mod app;
use app::state::AppState;

pub mod cache;

use io::{
    aedat4_decoder::{Aedat4, FileDataTable},
    codec::{DecoderFactory, VideoMetadata},
};

use crate::app::playback;

use playback::{Playback, PlaybackEvent};

slint::include_modules!();

pub fn main() {
    // Bootstrap wgpu for GPU accelarated 2D/3D rendering

    let mut wgpu_settings = WGPUSettings::default();
    wgpu_settings.device_required_features = wgpu::Features::IMMEDIATES;
    wgpu_settings.device_required_limits.max_immediate_size = 16;

    slint::BackendSelector::new()
        .require_wgpu_30(WGPUConfiguration::Automatic(wgpu_settings))
        .select()
        .expect("Unable to create backend with WGPU based renderer");

    let editor_window = AppWindow::new().expect("failed to create app window");
    let state = AppState::new();

    let runtime = Arc::new(
        tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .expect("could not create Tokio runtime"),
    );

    wire(&editor_window, state, runtime);

    let editor_weak = editor_window.as_weak();
    let mut renderer = None;

    editor_window
        .window()
        // Setup rendering pipeline for test shader
        .set_rendering_notifier(move |state, graphics_api| match state {
            slint::RenderingState::RenderingSetup => {
                if let slint::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api {
                    renderer = Some(Renderer::new(device, queue));
                    // Set UI elements in debug menu
                    if let Some(editor_window) = editor_weak.upgrade() {
                        editor_window.set_render_backend("renderer-femtovg-wgpu".into());
                        editor_window.set_device(device.adapter_info().name.into());
                        editor_window.set_device_capabilities(device.features().to_shared_string());
                    }
                }
            }
            slint::RenderingState::BeforeRendering => {
                if let Some(renderer) = &mut renderer {
                    let texture = renderer.render();
                    if let Ok(image) = slint::Image::try_from(texture)
                        && let Some(editor_window) = editor_weak.upgrade()
                    {
                        editor_window.set_texture(image);
                    }
                }
            }
            slint::RenderingState::AfterRendering => {}
            slint::RenderingState::RenderingTeardown => {
                drop(renderer.take());
            }
            _ => {
                eprintln!("unknown rendering state");
            }
        })
        .expect("Failed to setup wgpu renderer");

    editor_window.window().request_redraw();

    editor_window.run().expect("window failed to init");
}

/// Wires all callbacks for the UI.
fn wire(ui: &AppWindow, state: Arc<Mutex<AppState>>, runtime: Arc<tokio::runtime::Runtime>) {
    let ui_weak = ui.as_weak();
    // Timeline rendering
    // TODO: optimise this so on_render_timeline is only called when the timeline data changes (e.g. scale)
    {
        ui.on_render_timeline(|width, height| {
            if width < 100.0 || height < 100.0 {
                return slint::Image::default();
            }

            timeline::render(width as u32, height as u32)
        });
    }
    // File opening logic
    {
        let state_for_open_file = Arc::clone(&state);
        let runtime_for_open_file = Arc::clone(&runtime);
        ui.on_open_file(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if ui.get_file_loading() {
                return;
            }
            ui.set_file_loading(true);
            ui.set_file_error("".into());
            let state = Arc::clone(&state_for_open_file);
            let runtime = Arc::clone(&runtime_for_open_file);

            let ui_weak = ui.as_weak();
            let handle = std::thread::spawn(move || {
                let result = runtime.block_on(pick_and_open_file(state));
                let _ = ui_weak.upgrade_in_event_loop(move |ui| {
                    ui.set_file_loading(false);
                    match result {
                        Ok(Some((path, metadata, file_data_table))) => {
                            let sensor = metadata
                                .as_ref()
                                .and_then(|metadata| {
                                    metadata
                                        .output_modules
                                        .iter()
                                        .find_map(|module| module.output_info.source.as_deref())
                                })
                                .unwrap_or("Unknown")
                                .to_owned();
                            let events = file_data_table.total_events.to_string();
                            let duration = file_data_table.ts_end - file_data_table.ts_start;
                            ui.set_filename(path.to_string_lossy().as_ref().into());
                            ui.set_sensor(sensor.into());
                            ui.set_events(events.into());
                            ui.set_duration(format!("{} s", duration as f64 * 1e-6).into());
                            ui.set_current_timestamp(file_data_table.ts_start as i32);
                            let iso_formatted =
                                DateTime::from_timestamp_micros(file_data_table.ts_start as i64)
                                    .unwrap()
                                    .to_rfc3339();
                            ui.set_current_timestamp_formatted((&iso_formatted).into());
                            ui.set_offset_s(0.0);
                        }
                        Ok(None) => {}
                        Err(error) => ui.set_file_error(error.into()),
                    }
                });
            });
            let _ = handle.join();
        });
    }
    // Skip forward event
    {
        let state = Arc::clone(&state);
        let runtime = Arc::clone(&runtime);
        let ui_weak = ui.as_weak();
        ui.on_skip_forward(move || {
            let state = Arc::clone(&state);
            let ui_weak = ui_weak.clone();
            runtime.spawn(async move {
                let mut app_state = state.lock().await;
                app_state
                    .handle_playback_event(PlaybackEvent::SkipForwardEvent)
                    .await;
                let new_ts = app_state.playback_state.lock().await.current_timestamp;
                let start_ts = app_state.playback_state.lock().await.start_timestamp;
                let offset = new_ts - start_ts as u64;
                update_timestamp(ui_weak, new_ts, offset);
            });
        });
    }
    // Skip backward event
    {
        let state_for_skip_backward = Arc::clone(&state);
        let runtime = Arc::clone(&runtime);
        let ui_weak = ui.as_weak();
        ui.on_skip_backward(move || {
            let state = Arc::clone(&state_for_skip_backward);
            let ui_weak = ui_weak.clone();
            runtime.spawn(async move {
                let mut app_state = state.lock().await;
                app_state
                    .handle_playback_event(PlaybackEvent::SkipBackwardEvent)
                    .await;
                let new_ts = app_state.playback_state.lock().await.current_timestamp;
                let start_ts = app_state.playback_state.lock().await.start_timestamp;
                let offset = new_ts - start_ts as u64;
                update_timestamp(ui_weak, new_ts, offset);
            });
        });
    }
    // Pause/play event
    {
        let state = Arc::clone(&state);
        let runtime = Arc::clone(&runtime);
        ui.on_pause_play(move |is_paused| {
            let state = Arc::clone(&state);
            runtime.spawn(async move {
                let mut app_state = state.lock().await;
                if is_paused {
                    app_state
                        .handle_playback_event(PlaybackEvent::PressPauseEvent)
                        .await;
                } else {
                    app_state
                        .handle_playback_event(PlaybackEvent::PressPlayEvent)
                        .await;
                }
            });
        });
    }
    // Skip to start/end event
    {
        let state = Arc::clone(&state);
        let runtime = Arc::clone(&runtime);
        let ui_weak = ui.as_weak();
        ui.on_skip_start_end(move |is_start| {
            let state = Arc::clone(&state);
            let runtime = Arc::clone(&runtime);
            let ui_weak = ui_weak.clone();
            runtime.spawn(async move {
                let mut app_state = state.lock().await;
                let start_ts = app_state.playback_state.lock().await.start_timestamp;
                let end_ts = app_state.playback_state.lock().await.end_timestamp;

                if is_start {
                    app_state
                        .handle_playback_event(PlaybackEvent::SeekEvent {
                            timestamp: start_ts,
                        })
                        .await;
                } else {
                    app_state
                        .handle_playback_event(PlaybackEvent::SeekEvent { timestamp: end_ts })
                        .await;
                }
                let new_ts = if is_start { start_ts } else { end_ts };
                let offset = new_ts - start_ts;
                update_timestamp(ui_weak, new_ts, offset);
            });
        });
    }
    {}
    // Tick event which advances playback every 1/FPS * playback speed secondss
    {
        // Run two threads in paralell, one which ticks and one which updates the UI.
        let (timestamp_tx, mut timestamp_rx) = tokio::sync::watch::channel((0u64, 0u64));

        // Forward timestamps to the Slint event loop
        let ui_weak = ui.as_weak();
        runtime.spawn(async move {
            while timestamp_rx.changed().await.is_ok() {
                let (new_ts, offset) = *timestamp_rx.borrow_and_update();
                let ui_weak = ui_weak.clone();
                update_timestamp(ui_weak, new_ts, offset);
            }
        });

        // Start tick thread, this idles when the playback state is not Play
        let state = Arc::clone(&state);
        runtime.spawn(async move {
            let playback = Arc::clone(&state.lock().await.playback_state);
            Playback::tick(playback, timestamp_tx).await;
        });
    }
}

async fn pick_and_open_file(
    state: Arc<Mutex<AppState>>,
) -> Result<Option<(PathBuf, Option<VideoMetadata>, FileDataTable)>, String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter(".aedat4", &["aedat4"])
        .pick_file()
    else {
        return Ok(None);
    };

    let path_for_open = path.clone();

    let file = tokio::fs::File::open(&path_for_open)
        .await
        .map_err(|err| format!("could not open file: {} - {err}", path_for_open.display()))?;

    let decoder = Aedat4
        .open(file)
        .await
        .map_err(|_err| format!("could not decode file"))?;

    let metadata = decoder.metadata.clone();

    let file_data_table = decoder.file_data_table.clone();

    state.lock().await.set_file_state(*decoder).await;

    Ok(Some((path, metadata, file_data_table)))
}

fn update_timestamp(ui_weak: slint::Weak<AppWindow>, timestamp: u64, offset: u64) {
    slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            let iso_formatted = DateTime::from_timestamp_micros(timestamp as i64)
                .unwrap()
                .to_rfc3339();
            println!("timestamp: {}", timestamp);
            println!("iso_formatted: {}", iso_formatted);
            println!("timestamp_s: {}", timestamp as f32 / 1e6);

            ui.set_current_timestamp(timestamp as i32);
            ui.set_current_timestamp_formatted((&iso_formatted).into());
            ui.set_offset_s(offset as f32 / 1e6);
        }
    });
}
