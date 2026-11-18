#[path = "../utils/lib.rs"]
mod utils;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use event_camera_editor_ui::{app::playback::PlaybackEvent, visualisations::time_surface::{DecayMode, time_surface_at_t}};
use itertools::{Itertools, iproduct};
use std::{collections::HashSet, env, fmt::format, hint::black_box, path::PathBuf};
use tokio::runtime::Builder;

/// Benchmarks time-surface rendering at several timestamps from one recording.
/// Setup and checkpoint generation happen before Criterion starts timing.
fn bench_timesurface_with_playback(c: &mut Criterion) {
    let input = env::var_os("BENCH_FILE")
        .map(PathBuf::from)
        .expect("set BENCH_FILE to an AEDAT4 recording");

    let timestamp_samples = env::var_os("TIMESTAMP_SAMPLES")
        .map(|n| {
            n.into_string().unwrap_or_default().parse::<usize>().unwrap_or({
                eprintln!("TIMESTAMP_SAMPLES not set, defaulting to 10");
                10
            })
        }).unwrap_or({
            eprintln!("TIMESTAMP_SAMPLES not set, defaulting to 10");
            10
        });

    let runtime = Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("failed to create Tokio runtime");
    let (state, mut render_rx) = runtime.block_on(utils::setup_state(input));

    let (ts_start, ts_end) = runtime.block_on(async {
        let state = state.lock().await;
        let playback = state.playback_state.lock().await;
        let checkpoints = state.checkpoints.read().unwrap();
        println!("has {} checkpoints at timestamps {:?}", checkpoints.keys().len(), checkpoints.keys());
        (playback.start_timestamp, playback.end_timestamp)
    });

    assert!(ts_end > ts_start, "ts end and ts start are not valid");
    assert!(ts_start != ts_end, "file is zero length");

    let timestamps: Vec<u64> = vec![0; timestamp_samples];
    let step = (ts_end - ts_start) / timestamp_samples as u64;
    assert!(step > 0, "we do not have enough time range to generate {timestamp_samples} timestamps");

    // mid points between checkpoints
    let timestamps: Vec<u64> = timestamps.iter().enumerate().map(|(i, _)| ts_start + (step * i as u64 + step / 2) as u64).collect();

    // setup_state renders the initial frame while opening the recording.
    runtime.block_on(async {
        render_rx
            .recv()
            .await
            .expect("initial render was not sent");
    });

    let mut group = c.benchmark_group("time_surface");

    for timestamp in timestamps.iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(timestamp),
            timestamp,
            |b, &timestamp| {
                b.iter(|| {
                    runtime.block_on(async {
                        let mut state = state.lock().await;
                        state
                            .handle_playback_event(PlaybackEvent::SeekEvent { timestamp })
                            .await;
                        drop(state);
                        black_box(render_rx
                            .recv()
                            .await
                            .expect("didnt receive render"));
                    });
                });
            },
        );
    }
    group.finish();
}

fn benchmark_raw_time_surface_fn(c: &mut Criterion) {
    let widths: Vec<u32> = vec![100, 200, 400, 800, 1600, 3200];
    let heights: Vec<u32> = vec![100, 200, 400, 800, 1600, 3200];
    let decay_modes: Vec<DecayMode> = vec![DecayMode::Linear, DecayMode::Exponential];
    let decay_rates: Vec<u64> = vec![5_000_000, 10_000_000, 20_000_000, 40_000_000, 80_000_000, 160_000_000, 320_000_000];
    let mut group = c.benchmark_group("raw_time_surface");


    for (width, height, decay_mode, decay_rate) in iproduct!(widths.iter(), heights.iter(), decay_modes.iter(), decay_rates.iter()).unique() {
        group.bench_with_input(
            BenchmarkId::new("time_surface", &format!("{}x{} decay_mode={:?} decay_rate={} ms", width, height, decay_mode, decay_rate / 1000)),
            &(width, height, decay_mode, decay_rate),
            |b, &(width, height, decay_mode, decay_rate)| {
                b.iter(|| {
                    let t: i64 = 200;
                    // ts_last = [0,.., 99, 1, ..., 99]
                    let ts_last_array: Vec<u64> = (0..(*width * *height) as usize)
                        .map(|i| (i % 100) as u64)
                        .collect();
                    black_box(time_surface_at_t(*width, *height, t, ts_last_array, decay_mode.clone(), *decay_rate as f64));
                });
            },
        );
    }

}

criterion_group!(benches, bench_timesurface_with_playback, benchmark_raw_time_surface_fn);
criterion_main!(benches);
