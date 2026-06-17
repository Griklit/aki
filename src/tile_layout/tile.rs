#[derive(Debug)]
pub struct Tile<T> {
    pub item: T,
    pub width: usize,
    pub height: usize,
}

impl<T> Tile<T> {
    #[must_use]
    pub fn new(item: T, width: usize, height: usize) -> Self {
        Self {
            item,
            width,
            height,
        }
    }

    pub fn size(&self) -> usize {
        self.width * self.height
    }
}
