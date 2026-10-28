// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
pub mod renderer;

use renderer::Renderer;
use slint::{
    ToSharedString,
    wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu},
};

use std::{
    path::PathBuf,
    result,
    sync::{Arc, Mutex},
};

pub mod visualisations;
use visualisations::timeline;

pub mod app;
use app::state::AppState;

pub mod cache;

use io::{
    aedat4_decoder::{Aedat4, FileDataTable},
    codec::{DecoderFactory, VideoMetadata},
    file_data_table,
};

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
    wire(&editor_window, state);

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
                println!("unknown rendering state");
            }
        })
        .expect("Failed to setup wgpu renderer");

    editor_window.window().request_redraw();

    editor_window.run().expect("window failed to init");
}

/// Wires all callbacks for the UI.
fn wire(ui: &AppWindow, state: Arc<Mutex<AppState>>) {
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
        ui.on_open_file(move || {
            let Some(ui) = ui_weak.upgrade() else {
                return;
            };
            if ui.get_file_loading() {
                return;
            }
            ui.set_file_loading(true);
            ui.set_file_error("".into());

            let ui_weak = ui.as_weak();
            let state = Arc::clone(&state);
            let handle = std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("could not create Tokio runtime");

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
                            ui.set_duration(format!("{duration} µs").into());
                        }
                        Ok(None) => {}
                        Err(error) => ui.set_file_error(error.into()),
                    }
                });
            });
            let _ = handle.join();
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

    state
        .lock()
        .expect("lock failed")
        .set_file_state(*decoder)
        .await;

    Ok(Some((path, metadata, file_data_table)))
}
