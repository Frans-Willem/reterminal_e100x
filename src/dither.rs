use alloc::vec::Vec;
use core::ops::{AddAssign, Div, Mul};

pub trait DitherPalette {
    type SourceColor;
    type TargetColor;
    type QuantizationError: Default
        + Clone
        + Mul<usize, Output = Self::QuantizationError>
        + AddAssign<Self::QuantizationError>
        + Div<usize>;

    fn get_closest(
        &self,
        source: Self::SourceColor,
        diffused_error: <Self::QuantizationError as Div<usize>>::Output,
    ) -> (Self::TargetColor, Self::QuantizationError);
}

pub struct FloydSteinberg<D: DitherPalette, I: Iterator<Item = D::SourceColor>> {
    palette: D,
    source: I,
    width: usize,
    x: usize,
    y: usize,
    diffusion: Vec<[D::QuantizationError; 2]>,
}

impl<D: DitherPalette, I: Iterator<Item = D::SourceColor>> FloydSteinberg<D, I> {
    pub fn new(palette: D, source: I, width: usize) -> Self {
        let mut diffusion = Vec::new();
        diffusion.resize_with(width, Default::default);
        FloydSteinberg {
            palette,
            width,
            x: 0,
            y: 0,
            diffusion,
            source,
        }
    }
}

impl<D: DitherPalette, I: Iterator<Item = D::SourceColor>> Iterator for FloydSteinberg<D, I> {
    type Item = D::TargetColor;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        let source_color = self.source.next()?;
        let source_error = core::mem::take(&mut self.diffusion[self.x][self.y % 2]);
        let (target_color, error) = self.palette.get_closest(source_color, source_error / 16);
        // Spread error over next pixels
        for (dx, dy, mul) in [(1, 0, 7), (-1 as isize, 1, 3), (0, 1, 5), (1, 1, 1)] {
            if let (Some(tx), Some(ty)) = (self.x.checked_add_signed(dx), self.y.checked_add(dy)) {
                if tx < self.width {
                    self.diffusion[tx][ty % 2] += error.clone() * mul;
                }
            }
        }
        // Adjust pointer for next pixel
        self.x += 1;
        while self.x >= self.width {
            self.x -= self.width;
            self.y += 1;
        }
        Some(target_color)
    }
}

pub struct RgbaToBool;
#[derive(Clone)]
pub struct GrayscaleQuantizationError(i32);

impl DitherPalette for RgbaToBool {
    type SourceColor = [u8; 4];
    type TargetColor = bool;
    type QuantizationError = GrayscaleQuantizationError;

    fn get_closest(
        &self,
        source: Self::SourceColor,
        error: i32,
    ) -> (Self::TargetColor, Self::QuantizationError) {
        let gray = (source[0] as i32 + source[1] as i32 + source[2] as i32) / 3;
        let gray = gray + error;
        //let gray = gray.clamp(0, 255);
        if gray > 127 {
            (true, GrayscaleQuantizationError(gray - 255))
        } else {
            (false, GrayscaleQuantizationError(gray))
        }
    }
}

impl Default for GrayscaleQuantizationError {
    fn default() -> Self {
        GrayscaleQuantizationError(0)
    }
}

impl AddAssign for GrayscaleQuantizationError {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Mul<usize> for GrayscaleQuantizationError {
    type Output = Self;

    fn mul(self, rhs: usize) -> Self {
        Self(self.0 * (rhs as i32))
    }
}

impl Div<usize> for GrayscaleQuantizationError {
    type Output = i32;

    fn div(self, rhs: usize) -> i32 {
        self.0 / (rhs as i32)
    }
}
