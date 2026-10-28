use slint::{Image, Rgb8Pixel, SharedPixelBuffer};

pub enum DecayMode {
    Linear,
    Exponential,
}

pub fn time_surface_at_t(
    width: u32,
    height: u32,
    t: i64,
    mode: DecayMode,
    decay_constant: f64,
) -> Image {
    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    let pixels = buffer.make_mut_slice();

    for y in 0..height {
        for x in 0..width {
            let ts_last = get_ts_last_for_pixel(x, y, t);
            let delta_t = (t - ts_last) as f64;
            let intensity = match mode {
                // 255 * (1 - k * (delta t))
                DecayMode::Linear => 255.0 * (1.0 - decay_constant * delta_t),
                // 255 * (e^(-delta t / decay_constant))
                DecayMode::Exponential => (-delta_t / decay_constant).exp() * 255.0,
            }
            .floor()
            .clamp(0.0, 255.0) as u8;
            // TODO: add different colour maps rather than just monochrome
            pixels[(y * width + x) as usize] = Rgb8Pixel::new(intensity, intensity, intensity);
        }
    }
    Image::from_rgb8(buffer)
}

pub fn get_ts_last_for_pixel(x: u32, y: u32, t: i64) -> i64 {
    42
}

#[deprecated(
    since = "0.0.1",
    note = "this should only be used for benchmarking, this is slower!"
)]
pub fn get_ts_last_for_pixel_uncached(x: u32, y: u32, t: i64) -> i64 {
    42
}
