use alloc::vec::Vec;
use core::marker::PhantomData;
use core::ops::{AddAssign, Div, DivAssign, Mul, MulAssign};
use embedded_graphics::pixelcolor::{BinaryColor, RgbColor};

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

pub trait ForwardErrorDiffusionMethod {
    fn get_max_y_target(&self) -> usize;
    fn get_divisor(&self) -> usize;
    fn get_targets(&self) -> impl Iterator<Item = (isize, usize, usize)>;
}

pub struct FloydSteinberg;

impl ForwardErrorDiffusionMethod for FloydSteinberg {
    #[inline(always)]
    fn get_max_y_target(&self) -> usize {
        1
    }
    #[inline(always)]
    fn get_divisor(&self) -> usize {
        16
    }
    #[inline(always)]
    fn get_targets(&self) -> impl Iterator<Item = (isize, usize, usize)> {
        [(1, 0, 7), (-1, 1, 3), (0, 1, 5), (1, 1, 1)].into_iter()
    }
}

pub struct JarvisJudiceAndNinke;

impl ForwardErrorDiffusionMethod for JarvisJudiceAndNinke {
    #[inline(always)]
    fn get_max_y_target(&self) -> usize {
        2
    }
    #[inline(always)]
    fn get_divisor(&self) -> usize {
        48
    }
    #[inline(always)]
    fn get_targets(&self) -> impl Iterator<Item = (isize, usize, usize)> {
        [
            // First row
            (1, 0, 7),
            (2, 0, 5),
            // Second row
            (-2, 1, 3),
            (-1, 1, 5),
            (0, 1, 7),
            (1, 1, 5),
            (2, 1, 3),
            // Third row
            (-2, 2, 1),
            (-1, 2, 3),
            (0, 2, 5),
            (1, 2, 3),
            (2, 2, 1),
        ]
        .into_iter()
    }
}

pub struct Atkinson;
impl ForwardErrorDiffusionMethod for Atkinson {
    #[inline(always)]
    fn get_max_y_target(&self) -> usize {
        2
    }
    #[inline(always)]
    fn get_divisor(&self) -> usize {
        8
    }
    #[inline(always)]
    fn get_targets(&self) -> impl Iterator<Item = (isize, usize, usize)> {
        [
            // First row
            (1, 0, 1),
            (2, 0, 1),
            // Second row
            (-1, 1, 1),
            (0, 1, 1),
            (1, 1, 1),
            // Third row
            (0, 2, 1),
        ]
        .into_iter()
    }
}

pub struct ForwardErrorDiffusion<
    PALETTE: DitherPalette,
    METHOD: ForwardErrorDiffusionMethod,
    I: Iterator<Item = PALETTE::SourceColor>,
> {
    palette: PALETTE,
    method: METHOD,
    source: I,
    width: usize,
    x: usize,
    y: usize,
    diffusion: Vec<PALETTE::QuantizationError>,
}

impl<
    PALETTE: DitherPalette,
    METHOD: ForwardErrorDiffusionMethod,
    I: Iterator<Item = PALETTE::SourceColor>,
> ForwardErrorDiffusion<PALETTE, METHOD, I>
{
    pub fn new(palette: PALETTE, method: METHOD, source: I, width: usize) -> Self {
        let mut diffusion = Vec::new();
        diffusion.resize_with(width * (method.get_max_y_target() + 1), Default::default);
        ForwardErrorDiffusion {
            palette,
            method,
            width,
            x: 0,
            y: 0,
            diffusion,
            source,
        }
    }
}
impl<
    PALETTE: DitherPalette,
    METHOD: ForwardErrorDiffusionMethod,
    I: Iterator<Item = PALETTE::SourceColor>,
> ForwardErrorDiffusion<PALETTE, METHOD, I>
{
    fn get_diffusion_index(&self, x: usize, y: usize) -> usize {
        let y = y % (self.method.get_max_y_target() + 1);
        x + (self.width * y)
    }
}

impl<
    PALETTE: DitherPalette,
    METHOD: ForwardErrorDiffusionMethod,
    I: Iterator<Item = PALETTE::SourceColor>,
> Iterator for ForwardErrorDiffusion<PALETTE, METHOD, I>
{
    type Item = PALETTE::TargetColor;

    fn next(&mut self) -> Option<<Self as Iterator>::Item> {
        let source_color = self.source.next()?;
        let index = self.get_diffusion_index(self.x, self.y);
        let source_error = core::mem::take(&mut self.diffusion[index]);
        let (target_color, error) = self
            .palette
            .get_closest(source_color, source_error / self.method.get_divisor());
        // Spread error over next pixels
        for (dx, dy, mul) in self.method.get_targets() {
            if let (Some(tx), Some(ty)) = (self.x.checked_add_signed(dx), self.y.checked_add(dy))
                && tx < self.width
            {
                let tindex = self.get_diffusion_index(tx, ty);
                self.diffusion[tindex] += error.clone() * mul;
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

fn arr3zip<A, B, C, F: Fn(A, B) -> C>(a: [A; 3], b: [B; 3], f: F) -> [C; 3] {
    let [a0, a1, a2] = a;
    let [b0, b1, b2] = b;
    [f(a0, b0), f(a1, b1), f(a2, b2)]
}

fn rgb_to_arr<C: RgbColor>(c: C) -> [u8; 3] {
    [c.r(), c.g(), c.b()]
}

const fn rgb_max_arr<C: RgbColor>() -> [u8; 3] {
    [C::MAX_R, C::MAX_G, C::MAX_B]
}

pub struct RgbColorToBinaryColor<RGB: RgbColor>(PhantomData<RGB>);

impl<RGB: RgbColor> Default for RgbColorToBinaryColor<RGB> {
    fn default() -> Self {
        Self::new()
    }
}

impl<RGB: RgbColor> RgbColorToBinaryColor<RGB> {
    pub const fn new() -> Self {
        RgbColorToBinaryColor(PhantomData)
    }
}

impl<RGB: RgbColor> DitherPalette for RgbColorToBinaryColor<RGB> {
    type SourceColor = RGB;
    type TargetColor = BinaryColor;
    type QuantizationError = DefaultQuantizationError<i16, 1>;

    fn get_closest(
        &self,
        source: Self::SourceColor,
        error: Self::QuantizationError,
    ) -> (Self::TargetColor, Self::QuantizationError) {
        let source = rgb_to_arr(source);
        let total: i16 = source.into_iter().map(|x| x as i16).sum();
        let total = total + error.0[0];
        let max: i16 = RGB::MAX_R as i16 + RGB::MAX_G as i16 + RGB::MAX_B as i16;
        if total > max / 2 {
            (BinaryColor::On, DefaultQuantizationError([total - max]))
        } else {
            (BinaryColor::Off, DefaultQuantizationError([total]))
        }
    }
}

pub struct RgbColorToPalette<'t, RGB: RgbColor, T>(&'t [(RGB, T)]);

impl<'t, RGB: RgbColor, T> RgbColorToPalette<'t, RGB, T> {
    pub const fn new(palette: &'t [(RGB, T)]) -> Self {
        RgbColorToPalette(palette)
    }
}
impl<'t, RGB: RgbColor, T> DitherPalette for RgbColorToPalette<'t, RGB, T>
where
    T: Clone,
{
    type SourceColor = RGB;
    type TargetColor = T;
    type QuantizationError = DefaultQuantizationError<i16, 3>;

    fn get_closest(
        &self,
        source: Self::SourceColor,
        error: Self::QuantizationError,
    ) -> (Self::TargetColor, Self::QuantizationError) {
        let source = rgb_to_arr(source);
        let source_adjusted: [i16; 3] =
            arr3zip(source, error.0, |source, error| (source as i16) + error);
        let source_adjusted: [i16; 3] =
            arr3zip(source_adjusted, rgb_max_arr::<RGB>(), |source, max| {
                source.clamp(0, max as i16)
            });
        let options = self.0.iter();
        let options = options.map(|(palette_source, palette_target)| {
            let errors: [i16; 3] = arr3zip(source_adjusted, rgb_to_arr(*palette_source), |s, p| {
                s - (p as i16)
            });
            let distance: i32 = errors
                .iter()
                .map(|error| {
                    let error = *error as i32;
                    error * error
                })
                .sum();
            (distance, DefaultQuantizationError(errors), palette_target)
        });
        let (_, error, palette_target) = options.min_by_key(|(distance, _, _)| *distance).unwrap();
        (palette_target.clone(), error)
    }
}
