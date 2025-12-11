#[derive(Clone, Copy)]
pub enum Spectra6Color {
    Black = 0,
    White = 1,
    Yellow = 2,
    Red = 3,
    Blue = 5,
    Green = 6,
    Clean = 7,
}

pub struct SpectraPacker<T>(pub T);

impl<T> Iterator for SpectraPacker<T>
where
    T: Iterator<Item = Spectra6Color>,
{
    type Item = u8;
    fn next(&mut self) -> Option<Self::Item> {
        let left = self.0.next()?;
        let right = self.0.next().unwrap_or(Spectra6Color::White);
        Some((left as u8) << 4 | (right as u8))
    }
}
