use std::time::Duration;


use plotters::prelude::*;
use rand::{RngExt, SeedableRng};


// Bucket size for the event frequency histogram
pub const BUCKET_SIZE: i64 = 1_000; // 1 ms

pub fn dummy_data(limit: i32) -> Vec<(i32, i32)> {
    let mut data = Vec::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    for i in 0..limit {
        data.push((i, rng.random_range(0..100)));
    }
    data
}

pub fn render(width: u32, height: u32, y_max: i32, histogram: &[i32]) -> String {
    let x_max = histogram.len().max(1) as i64 * BUCKET_SIZE;
    let y_max = y_max.max(1);
    let mut svg_buffer = String::new();
    println!("rendering timeline");

    {
        let backend = SVGBackend::with_string(&mut svg_buffer, (width, height));

        let drawing_area = backend.into_drawing_area();

        drawing_area.fill(&WHITE).expect("failed to clear chart");

        let mut chart = ChartBuilder::on(&drawing_area)
            .margin(12)
            .set_label_area_size(LabelAreaPosition::Top, 30)
            .set_label_area_size(LabelAreaPosition::Bottom, 8)
            //.x_label_area_size(30)
            .y_label_area_size(60)
            .build_cartesian_2d(0i64..x_max, 0..y_max)
            .expect("failed to build chart");

        chart
            .configure_mesh()
            .disable_y_axis()
            .x_label_formatter(&|x| {
                let duration = Duration::from_micros(*x as u64);

                let total_ms = duration.as_millis();
                let total_seconds = total_ms / 1000;

                let minutes = total_seconds / 60;
                let seconds = total_seconds % 60;
                let milliseconds = total_ms % 1000;

                format!("{:02}:{:02}.{:03}", minutes, seconds, milliseconds)
            })
            .draw()
            .expect("failed to draw chart axes");

        chart
            .draw_series(histogram.iter().enumerate().map(|(index, &count)| {
                let start = index as i64 * BUCKET_SIZE;
                Rectangle::new([(start, 0), (start + BUCKET_SIZE, count)], BLUE.filled())
            }))
            .expect("failed to draw timeline");

        drawing_area.present().expect("failed to present chart");
    }

    svg_buffer
}
