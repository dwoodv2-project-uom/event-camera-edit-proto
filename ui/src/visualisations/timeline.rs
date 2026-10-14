use chrono::DateTime;
use plotters::prelude::*;
use rand::{RngExt, SeedableRng};
use slint::Image;

pub fn dummy_data(limit: i32) -> Vec<(i32, i32)> {
    let mut data = Vec::new();
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    for i in 0..limit {
        data.push((i, rng.random_range(0..100)));
    }
    data
}
pub fn render(width: u32, height: u32) -> Image {
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
            .build_cartesian_2d(0..100, 0..100)
            .expect("failed to build chart");

        chart
            .configure_mesh()
            .y_desc("Events")
            .x_label_formatter(&|x| DateTime::from_timestamp(*x as i64, 0).unwrap().to_string())
            .draw()
            .expect("failed to draw chart axes");

        chart
            .draw_series(LineSeries::new(dummy_data(100), &BLUE))
            .expect("failed to draw timeline");

        drawing_area.present().expect("failed to present chart");
    }

    Image::load_from_svg_data(svg_buffer.as_bytes()).expect("failed to load chart SVG")
}
