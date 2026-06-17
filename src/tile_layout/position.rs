#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: usize,
    pub y: usize,
}

impl Position {
    #[must_use]
    pub fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }
}
