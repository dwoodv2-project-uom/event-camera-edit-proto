use slint::{Rgb8Pixel, SharedPixelBuffer};


///  Renders an event frame visualisation of the camera's event stream.
///  `event_polarities` is a list of polarities (1 for positive, -1 for negative, 0 for nothing) for each pixel.
pub fn event_frame(width: u32, height: u32, event_polarities: Vec<i32>) -> SharedPixelBuffer<Rgb8Pixel> {
    let mut buffer = SharedPixelBuffer::<Rgb8Pixel>::new(width, height);
    let pixels = buffer.make_mut_slice();

    for y in 0..height {
        for x in 0..width {
            let polarity = event_polarities[(y * width + x) as usize];
            let colour = match polarity {
                // Grey for nothing happened (from t-delta to t at least)
                // White for (positive polarity)
                // Black for (negative polarity)
                0 => Rgb8Pixel::new(128, 128, 128),
                1 => Rgb8Pixel::new(255, 255, 255),
                -1 => Rgb8Pixel::new(0, 0, 0),
                _ => {
                    eprintln!("how did we get here? polarity: {}", polarity);
                    Rgb8Pixel::new(128, 128, 128)
                }
            };
            pixels[(y * width + x) as usize] = colour;
        }
    }
    buffer
}

mod tests {
    use crate::visualisations::event_frame::event_frame;

    #[test]
    fn test_event_frame() {
        let polarities = vec![0, -1, 1, 0];

        let event_frame = event_frame(2, 2, polarities);
        assert_eq!(event_frame.width(), 2);
        assert_eq!(event_frame.height(), 2);
        let bytes = event_frame.as_slice();

        // grey
        assert_eq!(bytes[0].r, 128);
        assert_eq!(bytes[0].g, 128);
        assert_eq!(bytes[0].b, 128);

        // black
        assert_eq!(bytes[1].r, 0);
        assert_eq!(bytes[1].g, 0);
        assert_eq!(bytes[1].b, 0);

        // white
        assert_eq!(bytes[2].r, 255);
        assert_eq!(bytes[2].g, 255);
        assert_eq!(bytes[2].b, 255);

        // grey
        assert_eq!(bytes[3].r, 128);
        assert_eq!(bytes[3].g, 128);
        assert_eq!(bytes[3].b, 128);



    }
}
