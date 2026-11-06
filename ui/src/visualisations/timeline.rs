use std::time::Duration;

use chrono::DateTime;
use plotters::prelude::*;
use rand::{RngExt, SeedableRng};
use slint::Image;
use tokio::time::error::Elapsed;

pub fn dummy_data(limit: i32) -> Vec<(i32, i32)> {
    let mut data = Vec::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    for i in 0..limit {
        data.push((i, rng.random_range(0..100)));
    }
    data
}

pub fn render(width: u32, height: u32, x_max: i32, y_max: i32, y_min: i32, data: Vec<(i32, i32)>) -> String {
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
            .build_cartesian_2d(0..x_max, y_min..y_max)
            .expect("failed to build chart");

        chart
            .configure_mesh()
            .y_desc("Events")
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
            .draw_series(LineSeries::new(data, &BLUE))
            .expect("failed to draw timeline");

        drawing_area.present().expect("failed to present chart");
    }

    svg_buffer
}
