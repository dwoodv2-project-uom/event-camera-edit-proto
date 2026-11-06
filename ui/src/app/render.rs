use io::codec::{Decoder, Packet};
use slint::{Rgb8Pixel, SharedPixelBuffer};

use crate::{app::state::AppState, visualisations::{time_surface::{DecayMode, time_surface_at_t}, timeline}};

/// Provides information needed to plug into the render function of a certain visualisation
/// E.g. a time surface needs timestamp of last polarity on event
pub struct RenderCursor {
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
}

pub enum VisualisationMode {
    TimeSurface,
    Timeline,
}

pub struct VisualisationParams {
    pub time_surface_params: TimeSurfaceParams,
}

pub struct TimeSurfaceParams {
    pub decay_constant: f64,
    pub decay_mode: DecayMode,
}

impl Default for VisualisationState {
    fn default() -> Self {
        Self {
            mode: VisualisationMode::TimeSurface,
            params: VisualisationParams {
                time_surface_params: TimeSurfaceParams {
                    decay_constant: 50.0,
                    decay_mode: DecayMode::Exponential,
                },
            },
        }
    }
}

pub struct VisualisationState {
    pub mode: VisualisationMode,
    pub params: VisualisationParams,
}

pub enum RenderTarget {
    PreviewWindow,
    Timeline
}

pub struct ImageUpdate {
    pub target: RenderTarget,
    pub texture: Option<slint::SharedPixelBuffer<slint::Rgb8Pixel>>,
    pub svg: Option<String>,
}

pub trait Renderer {
    fn set_time_surface_decay_mode(&mut self, mode: DecayMode);

    fn set_time_surface_decay_constant(&mut self, decay: f64);

    async fn render(&self, vizualisation_type: VisualisationMode, timestamp: u64, width: u32, height: u32, state: &AppState);
}


/// Render function for a specific visualisation mode.
/// Note: this may include I/O operations and should not be called frequently.
impl Renderer for VisualisationState {

    fn set_time_surface_decay_mode(&mut self, mode: DecayMode) {
        self.params.time_surface_params.decay_mode = mode;
    }

    fn set_time_surface_decay_constant(&mut self, decay: f64) {
        self.params.time_surface_params.decay_constant = decay;
    }

    async fn render(&self, vizualisation_type: VisualisationMode, timestamp: u64, width: u32, height: u32, state: &AppState) {

        match vizualisation_type {
            VisualisationMode::TimeSurface => {
                // Return black image if we're at the start of the playback
                if timestamp == state.playback_state.lock().await.start_timestamp {
                    let black_image = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
                    state.render_tx.send(ImageUpdate { target: RenderTarget::PreviewWindow, texture: Some(black_image), svg: None }).await.unwrap();
                    return;
                }
                // Using cache, we apply the packets from the left most checkpoint up until the timestamp

                let (checkpoint_ts, left_most_checkpoint) = state.get_left_most_checkpoint(timestamp).await;

                let mut last_pol_on = left_most_checkpoint.last_polarity_on.clone();

                let decoder = state.get_decoder().await.expect("no decoder available, is the file open?");
                let mut decoder = decoder.lock().await;

                let packets_from_check_to_ts = decoder.packets_by_ts_range(checkpoint_ts as i64, timestamp as i64).await.unwrap().unwrap();
                let event_packets = packets_from_check_to_ts.into_iter().filter_map(|packet| match packet {
                    Packet::EventPacket(content) => Some(content),
                    _ => None,
                });

                for event_packet in event_packets {
                    let events = event_packet.as_events().unwrap_or_default();
                    for event in events.iter().flatten() {
                        let idx = (event.y() as usize) * width as usize + event.x() as usize;
                        if event.polarity() {
                            last_pol_on[idx] = event.ts() as u64;
                        }
                    }
                }

                // Finally, render the time surface and send it to the consumer thread
                let rendered_image = time_surface_at_t(width, height, timestamp as i64, last_pol_on.to_vec(), self.params.time_surface_params.decay_mode.clone(), self.params.time_surface_params.decay_constant);
                state.render_tx.send( ImageUpdate { target: RenderTarget::PreviewWindow, texture: Some(rendered_image), svg: None }).await.expect("failed to send rendered image");
            }
            VisualisationMode::Timeline => {
                println!("rendering timeline");
                let file_data_table = state.file_state.as_ref().unwrap().lock().await.file_data_table.clone();

                let mut event_freq = Vec::<(i32, i32)>::new();
                let mut y_min = i32::MAX;
                let mut y_max = 0;
                let x_max = file_data_table.ts_end as i32 - file_data_table.ts_start as i32;
                let ts_start = file_data_table.ts_start as i32;

                for packet in file_data_table.events {
                    let ts = packet.metadata.ts_start;
                    let offset = ts as i32 - ts_start;
                    let count = packet.metadata.num_elements as i32;
                    if count > y_max {
                        y_max = count;
                    } else if count < y_min {
                        y_min = count;
                    }
                    event_freq.push((offset as i32, count as i32));
                }

                let rendered_svg = timeline::render(width, height, x_max, y_max, y_min, event_freq);
                state.render_tx.send( ImageUpdate { target: RenderTarget::Timeline, svg: Some(rendered_svg), texture: None }).await.expect("failed to send rendered image");
            }
        }
    }
}
/*

*/
