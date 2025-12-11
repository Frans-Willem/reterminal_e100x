use alloc::vec::Vec;
use core::ops::{AddAssign, Div, DivAssign, Mul, MulAssign};

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

#[derive(Clone)]
pub struct DefaultQuantizationError<T, const CHANNELS: usize>([T; CHANNELS]);

impl<T, const CHANNELS: usize> Default for DefaultQuantizationError<T, CHANNELS>
where
    [T; CHANNELS]: Default,
{
    fn default() -> Self {
        DefaultQuantizationError(Default::default())
    }
}

impl<T, const CHANNELS: usize> AddAssign for DefaultQuantizationError<T, CHANNELS>
where
    T: AddAssign,
    T: Copy,
{
    fn add_assign(&mut self, rhs: Self) {
        for i in 0..CHANNELS {
            self.0[i] += rhs.0[i];
        }
    }
}

impl<T, const CHANNELS: usize> Mul<usize> for DefaultQuantizationError<T, CHANNELS>
where
    T: MulAssign,
    T: Copy,
    T: TryFrom<usize>,
    T: Default,
{
    type Output = Self;

    fn mul(mut self, rhs: usize) -> Self {
        for i in 0..CHANNELS {
            self.0[i] *= rhs.try_into().unwrap_or(Default::default());
        }
        self
    }
}

impl<T, const CHANNELS: usize> Div<usize> for DefaultQuantizationError<T, CHANNELS>
where
    T: DivAssign,
    T: Copy,
    T: TryFrom<usize>,
    T: Default,
{
    type Output = Self;

    fn div(mut self, rhs: usize) -> Self {
        for i in 0..CHANNELS {
            self.0[i] /= rhs.try_into().unwrap_or(Default::default());
        }
        self
    }
}

pub struct RgbaToBool;

impl DitherPalette for RgbaToBool {
    type SourceColor = [u8; 4];
    type TargetColor = bool;
    type QuantizationError = DefaultQuantizationError<i16, 1>;

    fn get_closest(
        &self,
        source: Self::SourceColor,
        error: Self::QuantizationError,
    ) -> (Self::TargetColor, Self::QuantizationError) {
        let gray = (source[0] as i16 + source[1] as i16 + source[2] as i16) / 3;
        let gray = gray + error.0[0];
        if gray > 127 {
            (true, DefaultQuantizationError([gray - 255]))
        } else {
            (false, DefaultQuantizationError([gray]))
        }
    }
}

pub struct RgbaToPalette<'t, T>(pub &'t [([u8; 3], T)]);

impl<'t, T> DitherPalette for RgbaToPalette<'t, T>
where
    T: Clone,
{
    type SourceColor = [u8; 4];
    type TargetColor = T;
    type QuantizationError = DefaultQuantizationError<i16, 3>;

    fn get_closest(
        &self,
        source: Self::SourceColor,
        error: Self::QuantizationError,
    ) -> (Self::TargetColor, Self::QuantizationError) {
        let source_adjusted: [i16; 3] = [
            source[0] as i16 + error.0[0],
            source[1] as i16 + error.0[1],
            source[2] as i16 + error.0[2],
        ];
        let source_adjusted = [
            source_adjusted[0].clamp(0, 255),
            source_adjusted[1].clamp(0, 255),
            source_adjusted[2].clamp(0, 255),
        ];
        let options = self.0.iter();
        let options = options.map(|(palette_source, palette_target)| {
            let errors: [i16; 3] = [
                source_adjusted[0] - palette_source[0] as i16,
                source_adjusted[1] - palette_source[1] as i16,
                source_adjusted[2] - palette_source[2] as i16,
            ];
            let distance = errors[0] as i32 * errors[0] as i32
                + errors[1] as i32 * errors[1] as i32
                + errors[2] as i32 * errors[2] as i32;
            (distance, DefaultQuantizationError(errors), palette_target)
        });
        let (distance, error, palette_target) = options
            .min_by_key(|(distance, error, palette_target)| *distance)
            .unwrap();
        (palette_target.clone(), error)
    }
}
