use slint::{Image, Rgb8Pixel, SharedPixelBuffer};

pub enum DecayMode {
    Linear,
    Exponential,
}

pub fn time_surface_at_t(
    width: u32,
    height: u32,
    t: i64,
    ts_last_array: &[i64],
    mode: DecayMode,
    decay_constant: f64,
) -> Image {
    assert!(width * height == ts_last_array.len() as u32);
    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    let pixels = buffer.make_mut_slice();

    // TODO: re-write this using a WGSL shader, it might be faster
    for y in 0..height {
        for x in 0..width {
            let ts_last = ts_last_array[(y * width + x) as usize];
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_surface_at_t_long() {
        let width = 100;
        let height = 100;
        let t = 12345678;
        let ts_last_array = vec![0; (width * height) as usize];
        let mode = DecayMode::Exponential;
        let decay_constant = 25.0;
        let image =
            time_surface_at_t(width, height, t, &ts_last_array, mode, decay_constant).to_rgb8();

        assert!(image.is_some());

        let rgba_8 = image.unwrap();
        assert_eq!(rgba_8.width(), width);
        assert_eq!(rgba_8.height(), height);
        // should have decayed
        assert!(rgba_8.as_bytes().iter().all(|&p| p == 0));
    }

    #[test]
    fn test_time_surface_at_t() {
        let width = 100;
        let height = 100;
        let t = 12345678;
        let ts_last_array = vec![12345677; (width * height) as usize];
        let mode = DecayMode::Exponential;
        let decay_constant = 25.0;
        let image =
            time_surface_at_t(width, height, t, &ts_last_array, mode, decay_constant).to_rgb8();

        assert!(image.is_some());

        let rgba_8 = image.unwrap();
        assert_eq!(rgba_8.width(), width);
        assert_eq!(rgba_8.height(), height);
        // should be relatively bright
        assert!(rgba_8.as_bytes().iter().all(|&p| {
            p == ((-1.0 / decay_constant).exp() * 255.0)
                .floor()
                .clamp(0.0, 255.0) as u8
        }));
    }
}
