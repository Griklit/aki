use super::Size;

#[derive(Debug)]
pub struct RawTile<T> {
    pub item: T,
    // 原始比例
    pub size: Size,
}

impl<T> RawTile<T> {
    #[must_use]
    #[inline]
    pub fn new(item: T, size: Size) -> Self {
        Self { item, size }
    }
}

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

    /// 将原始瓦片根据网格尺寸尽可能转换为目标瓦片，不保证 target_area 一定等于 w * h
    pub fn from_raw(raw: RawTile<T>, grid_size: Size, target_area: f64) -> Self {
        // 只关心比例，不关心原始尺寸，故而此处不进行归一化
        let raw_h_grided = raw.size.h as f64 / grid_size.h as f64;
        let raw_w_grided = raw.size.w as f64 / grid_size.w as f64;
        let ratio_grided = raw_w_grided / raw_h_grided;
        let w = ((target_area * ratio_grided).sqrt()).round().max(1.0);
        let h = ((target_area / ratio_grided).sqrt()).round().max(1.0);
        Self::new(raw.item, Size::new(w as usize, h as usize))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_raw() {
        let raws = [
            (2 ^ 8 * 3, 2 ^ 8 * 3),
            (2 ^ 8 * 3, 2 ^ 8 * 4),
            (2 ^ 8 * 3, 2 ^ 8 * 5),
            (2 ^ 8 * 3, 2 ^ 8 * 6),
            (17, 16),
            (51, 42),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (w, h))| RawTile::new(i, Size::new(w, h)))
        .collect::<Vec<_>>();
        for raw in raws {
            let tile = Tile::from_raw(raw, Size::new(1, 1), 12.0);
            dbg!(tile);
        }
    }
}
