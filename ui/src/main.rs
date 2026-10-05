// Prevent console window in addition to Slint window in Windows release builds when, e.g., starting the app via file manager. Ignored on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use slint::{
    ToSharedString,
    wgpu_30::{WGPUConfiguration, WGPUSettings, wgpu},
};
pub mod renderer;
use renderer::Renderer;

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
    let editor_weak = editor_window.as_weak();
    let mut renderer = None;

    editor_window
        .window()
        // Setup rendering pipeline for test shader
        .set_rendering_notifier(move |state, graphics_api| match state {
            slint::RenderingState::RenderingSetup => {
                if let slint::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api {
                    renderer = Some(Renderer::new(&device, &queue));
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
                    if let Ok(image) = slint::Image::try_from(texture) {
                        if let Some(editor_window) = editor_weak.upgrade() {
                            editor_window.set_texture(image);
                        }
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
