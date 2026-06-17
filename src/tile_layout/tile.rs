use super::Size;

#[derive(Debug)]
pub struct Tile<T> {
    pub item: T,
    pub size: Size,
}

impl<T> Tile<T> {
    #[must_use]
    pub fn new(item: T, size: Size) -> Self {
        Self { item, size }
    }

    pub fn from_raw(raw: &RawTile<T>, grid_size: Size) -> Self {
        todo!()
    }
}

#[derive(Debug)]
pub struct RawTile<T> {
    pub item: T,
    // 原始比例
    pub size: Size,
    // 权重
    pub weight: u32,
}
