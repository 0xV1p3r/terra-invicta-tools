//! 8-bit images in the two Pillow modes the decoders produce.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Rgb,
    Rgba,
}

impl Mode {
    pub fn channels(self) -> usize {
        match self {
            Self::Rgb => 3,
            Self::Rgba => 4,
        }
    }
}

/// Rows top to bottom, tightly packed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub mode: Mode,
    pub data: Vec<u8>,
}

impl Image {
    pub fn new(width: u32, height: u32, mode: Mode) -> Self {
        Self {
            width,
            height,
            mode,
            data: vec![0; width as usize * height as usize * mode.channels()],
        }
    }

    fn stride(&self) -> usize {
        self.width as usize * self.mode.channels()
    }

    /// Pillow's `crop`: the box may extend past the image, which reads as zeros.
    pub fn crop(&self, x0: i64, y0: i64, x1: i64, y1: i64) -> Image {
        let (w, h) = ((x1 - x0).max(0) as u32, (y1 - y0).max(0) as u32);
        let mut out = Image::new(w, h, self.mode);
        let c = self.mode.channels();
        let (sx0, sx1) = (x0.max(0), x1.min(i64::from(self.width)));
        if sx0 >= sx1 {
            return out;
        }
        let len = (sx1 - sx0) as usize * c;
        let dst_x = (sx0 - x0) as usize * c;
        for y in y0.max(0)..y1.min(i64::from(self.height)) {
            let src = y as usize * self.stride() + sx0 as usize * c;
            let dst = (y - y0) as usize * out.stride() + dst_x;
            out.data[dst..dst + len].copy_from_slice(&self.data[src..src + len]);
        }
        out
    }

    /// [`Image::crop`] that reuses the buffer when the box is the whole image.
    pub fn into_crop(self, x0: i64, y0: i64, x1: i64, y1: i64) -> Image {
        if (x0, y0, x1, y1) == (0, 0, i64::from(self.width), i64::from(self.height)) {
            self
        } else {
            self.crop(x0, y0, x1, y1)
        }
    }

    pub fn flip_vertical(&mut self) {
        let stride = self.stride();
        let h = self.height as usize;
        for y in 0..h / 2 {
            let (top, bottom) = self.data.split_at_mut((h - 1 - y) * stride);
            top[y * stride..(y + 1) * stride].swap_with_slice(&mut bottom[..stride]);
        }
    }

    pub fn flip_horizontal(&mut self) {
        let c = self.mode.channels();
        let stride = self.stride();
        let w = self.width as usize;
        for row in self.data.chunks_exact_mut(stride) {
            for x in 0..w / 2 {
                for k in 0..c {
                    row.swap(x * c + k, (w - 1 - x) * c + k);
                }
            }
        }
    }

    /// Pillow's `transpose(ROTATE_270)`: 90 degrees clockwise.
    pub fn rotate_270(&self) -> Image {
        let c = self.mode.channels();
        let (w, h) = (self.width as usize, self.height as usize);
        let mut out = Image::new(self.height, self.width, self.mode);
        for y in 0..h {
            for x in 0..w {
                // (x, y) -> (h - 1 - y, x)
                let src = (y * w + x) * c;
                let dst = (x * h + (h - 1 - y)) * c;
                out.data[dst..dst + c].copy_from_slice(&self.data[src..src + c]);
            }
        }
        out
    }

    /// The pixels as RGBA (alpha 255 for RGB).
    pub fn to_rgba(&self) -> Vec<u8> {
        match self.mode {
            Mode::Rgba => self.data.clone(),
            Mode::Rgb => self.data.as_chunks::<3>().0.iter().flat_map(|p| [p[0], p[1], p[2], 255]).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn numbered(w: u32, h: u32) -> Image {
        let mut im = Image::new(w, h, Mode::Rgb);
        for (i, p) in im.data.as_chunks_mut::<3>().0.iter_mut().enumerate() {
            p.fill(i as u8);
        }
        im
    }

    #[test]
    fn crop_pads_with_zeros() {
        let im = numbered(3, 2);
        let c = im.crop(1, -1, 4, 1);
        assert_eq!((c.width, c.height), (3, 2));
        let px: Vec<u8> = c.data.as_chunks::<3>().0.iter().map(|p| p[0]).collect();
        assert_eq!(px, [0, 0, 0, 1, 2, 0]);
    }

    #[test]
    fn flips_and_rotation() {
        let mut im = numbered(2, 3);
        im.flip_vertical();
        let px: Vec<u8> = im.data.as_chunks::<3>().0.iter().map(|p| p[0]).collect();
        assert_eq!(px, [4, 5, 2, 3, 0, 1]);
        im.flip_horizontal();
        let px: Vec<u8> = im.data.as_chunks::<3>().0.iter().map(|p| p[0]).collect();
        assert_eq!(px, [5, 4, 3, 2, 1, 0]);
        let r = numbered(2, 3).rotate_270();
        assert_eq!((r.width, r.height), (3, 2));
        let px: Vec<u8> = r.data.as_chunks::<3>().0.iter().map(|p| p[0]).collect();
        // 0 1      4 2 0
        // 2 3  ->  5 3 1
        // 4 5
        assert_eq!(px, [4, 2, 0, 5, 3, 1]);
    }
}
