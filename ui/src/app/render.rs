use io::codec::{Decoder, Packet};
use slint::{Rgb8Pixel, SharedPixelBuffer};

use crate::{app::state::AppState, visualisations::{event_frame::event_frame, time_surface::{DecayMode, time_surface_at_t}, timeline}};

/// Provides information needed to plug into the render function of a certain visualisation
/// E.g. a time surface needs timestamp of last polarity on event
pub struct RenderCursor {
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
}

pub enum VisualisationMode {
    TimeSurface,
    EventFrame,
    Timeline,
}

pub struct VisualisationParams {
    pub time_surface_params: TimeSurfaceParams,
    pub event_frame_params: EventFrameParams,
}

pub struct TimeSurfaceParams {
    pub decay_constant: f64,
    pub decay_mode: DecayMode,
}

pub struct EventFrameParams {
    pub delta_t: u32,
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
                event_frame_params: EventFrameParams {
                    delta_t: 100,
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
    PreviewWindow(u32),
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

        // Return black image(s) if we're at the start of the playback
        if timestamp == state.playback_state.lock().await.start_timestamp {
            let black_image = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
            state.render_tx.send(ImageUpdate { target: RenderTarget::PreviewWindow(0), texture: Some(black_image.clone()), svg: None }).await.unwrap();
            state.render_tx.send(ImageUpdate { target: RenderTarget::PreviewWindow(1), texture: Some(black_image), svg: None }).await.unwrap();
            return;
        }

        match vizualisation_type {
            VisualisationMode::TimeSurface => {
                if timestamp == state.playback_state.lock().await.start_timestamp {
                    let black_image = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
                    state.render_tx.send(ImageUpdate { target: RenderTarget::PreviewWindow(0), texture: Some(black_image), svg: None }).await.unwrap();
                    return;
                }
                #[cfg(not(feature = "benchmarking"))]
                {
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
                    state.render_tx.send( ImageUpdate { target: RenderTarget::PreviewWindow(0), texture: Some(rendered_image), svg: None }).await.expect("failed to send rendered image");
                }
                #[cfg(feature = "benchmarking")]
                // while benchmarking, checkpoints can be disabled for testing
                {
                    let left_most_checkpoint = state.get_left_most_checkpoint(timestamp).await;
                    let decoder = state.get_decoder().await.expect("no decoder available, is the file open?");
                    let mut decoder = decoder.lock().await;

                    let (mut last_pol_on, start_ts) = match left_most_checkpoint {
                        Some((checkpoint_ts, checkpoint)) => (
                            checkpoint.last_polarity_on.clone(),
                            checkpoint_ts as i64,
                        ),
                        None => {
                            // With no checkpoint, start from the beginning of the recording.
                            (
                                vec![0u64; width as usize * height as usize],
                                decoder.file_data_table.ts_start,
                            )
                        }
                    };

                    let packets_from_check_to_ts = decoder
                        .packets_by_ts_range(start_ts, timestamp as i64)
                        .await
                        .unwrap()
                        .unwrap();
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
                    state.render_tx.send( ImageUpdate { target: RenderTarget::PreviewWindow(0), texture: Some(rendered_image), svg: None }).await.expect("failed to send rendered image");
                }

            }
            VisualisationMode::EventFrame => {
                    let decoder = state.get_decoder().await.expect("no decoder available, is the file open?");
                    let mut decoder = decoder.lock().await;

                    let mut polarities = vec![0; (width * height) as usize];

                    // find left most (polarity offs and ons) checkpoint from t and read from [t-checkpoint t, t]
                    let left_most_checkpoint = state.get_left_most_checkpoint(timestamp - self.params.event_frame_params.delta_t as u64).await;

                    let checkpoint_ts = left_most_checkpoint.0;

                    let delta_t = self.params.event_frame_params.delta_t as u64 * 1000;

                    let packets = if checkpoint_ts > timestamp - delta_t as u64 {
                        // Checkpoint is within delta T so we can read less (from C) using caching
                        // t-dt -------- C------------t

                        let time_from_checkpoint_to_t = timestamp - left_most_checkpoint.0;

                        let last_polarity_off = left_most_checkpoint.1.last_polarity_off.clone().iter().enumerate().map(|(i, &ts)| {
                            let within_delta_t = ts + (left_most_checkpoint.0.saturating_sub(ts)) + time_from_checkpoint_to_t <= self.params.event_frame_params.delta_t as u64;
                            if ts == 0 || !within_delta_t { // too far or no timestamp
                                0
                            } else {
                                polarities[i] = -1;
                                ts
                            }
                        }).collect::<Vec<_>>();

                        let _last_polarity_on = left_most_checkpoint.1.last_polarity_on.clone().iter().enumerate().map(|(i, &ts)| {
                            let within_delta_t = ts + (left_most_checkpoint.0.saturating_sub(ts)) + time_from_checkpoint_to_t <= self.params.event_frame_params.delta_t as u64;
                            if ts == 0 || !within_delta_t {
                                0
                            } else {
                                if ts > last_polarity_off[i] { // only overwrite polarity if it is newer
                                    polarities[i] = 1;
                                }
                                ts
                            }
                        }).collect::<Vec<_>>();

                        decoder
                            .packets_by_ts_range(left_most_checkpoint.0 as i64, timestamp as i64)
                            .await
                            .unwrap()
                            .unwrap()
                    } else {
                        println!("reading from delta t");
                        decoder
                            .packets_by_ts_range(timestamp as i64 - delta_t as i64, timestamp as i64)
                            .await
                            .unwrap()
                            .unwrap()
                    };

                    println!("found {} packets", packets.len());

                    let event_packets = packets.into_iter().filter_map(|packet| match packet {
                        Packet::EventPacket(content) => Some(content),
                        _ => None,
                    });

                    for event_packet in event_packets {
                        let events = event_packet.as_events().unwrap_or_default();
                        for event in events.iter().flatten() {
                            let idx = (event.y() as usize) * width as usize + event.x() as usize;
                            match event.polarity() {
                                true => polarities[idx] = 1,
                                false => polarities[idx] = -1,
                            }
                        }
                    }

                    let rendered_image = event_frame(width, height, polarities);
                    state.render_tx.send(ImageUpdate { target: RenderTarget::PreviewWindow(1), svg: None, texture: Some(rendered_image) }).await.expect("failed to send rendered image");
            }
            VisualisationMode::Timeline => {
                println!("rendering timeline");
                let rendered_svg = timeline::render(
                    width,
                    height,
                    state.max_event_count,
                    &state.event_histogram,
                );
                state.render_tx.send( ImageUpdate { target: RenderTarget::Timeline, svg: Some(rendered_svg), texture: None }).await.expect("failed to send rendered image");
            }
        }
    }
}
