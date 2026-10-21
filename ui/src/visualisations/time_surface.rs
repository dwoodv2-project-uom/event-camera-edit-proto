pub enum DecayMode {
    Linear,
    Exponential,
}


pub fn time_surface_at_t(width: usize, height: usize, t: f64, mode: DecayMode, decay_constant: f64) ->  {
    /*
     let buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);

    for y in 0..height {
        for x in 0..width {
            let ts_last = get_ts_last_for_pixel(x, y);
            let intensity = floor(match mode {
                // TODO: check maths
                // 255 * (1 - k * (delta t))
                DecayMode::Linear => 255.0 * (1.0 - decay_constant * (t - ts_last)),
                // 255 * (e^(-delta t / decay_constant))
                DecayMode::Exponential => 255.0 * exp(-(t - ts_last) / decay_constant),
            });
            let pixel = Rgb8Pixel::new(intensity as u8, intensity as u8, intensity as u8);
            buffer.set_pixel(x, y, pixel);
        }
    }
     */
}
