use image::RgbaImage;

#[derive(Clone, Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Frame {
    /// # Panics
    ///
    /// Panics if either side is zero or the buffer is not four bytes a pixel.
    #[must_use]
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        assert!(
            width > 0 && height > 0,
            "frame must have positive dimensions"
        );
        assert_eq!(
            rgba.len(),
            width as usize * height as usize * 4,
            "rgba buffer size mismatch"
        );
        Self {
            width,
            height,
            rgba,
        }
    }

    #[must_use]
    pub fn from_image(image: RgbaImage) -> Self {
        Self::new(image.width(), image.height(), image.into_raw())
    }

    /// # Panics
    ///
    /// Panics if the buffer does not hold four bytes a pixel.
    #[must_use]
    pub fn to_image(&self) -> RgbaImage {
        RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
            .expect("rgba buffer size mismatch")
    }

    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        let base = (y as usize * self.width as usize + x as usize) * 4;
        [
            self.rgba[base],
            self.rgba[base + 1],
            self.rgba[base + 2],
            self.rgba[base + 3],
        ]
    }
}
