use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    fmt::Display,
    hash::Hash,
};

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

pub struct HeightExpandableMap<T: Clone> {
    map: Vec<Option<T>>,
    width: usize,
    height: usize,
}

pub enum PopLesserOption<T> {
    Some(T),
    NoLesser,
    Empty,
}

pub struct SortedContainer<T, K: Ord + Clone> {
    data: BTreeMap<K, VecDeque<T>>,
}

impl<T, K: Ord + Clone> SortedContainer<T, K> {
    pub fn new() -> Self {
        Self {
            data: BTreeMap::new(),
        }
    }

    pub fn insert(&mut self, key: K, value: T) {
        self.data.entry(key).or_default().push_back(value);
    }

    pub fn pop_lesser(&mut self, key: K) -> PopLesserOption<T> {
        if self.data.is_empty() {
            return PopLesserOption::Empty;
        }
        let (key, item, flag) = {
            let Some((k, items)) = self.data.range_mut(..=key).next_back() else {
                return PopLesserOption::NoLesser;
            };
            let item = items.pop_front().expect("impossible");
            (k.clone(), item, items.is_empty())
        };
        if flag {
            self.data.remove(&key);
        }
        PopLesserOption::Some(item)
    }

    // pub fn pop_greater(&mut self, key: K) -> Option<T> {
    //     let (key, item, flag) = {
    //         let (key, items) = self.data.range_mut(key..).next()?;
    //         let item = items.pop()?;
    //         (key.clone(), item, items.is_empty())
    //     };
    //     if flag {
    //         self.data.remove(&key);
    //     }
    //     Some(item)
    // }
}

impl<T: Clone> HeightExpandableMap<T> {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            map: vec![None; width * height],
            width,
            height,
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    fn resize(&mut self, height: usize) {
        self.map.resize(self.width * height, None);
        self.height = height;
    }

    pub fn get(&self, p: Position) -> Option<&T> {
        self.map
            .get(p.y * self.width + p.x)
            .and_then(|v| v.as_ref())
    }

    pub fn set(&mut self, p: Position, value: T) {
        if p.y >= self.height {
            self.resize(p.y + 1);
        }
        self.map[p.y * self.width + p.x] = Some(value);
    }
    pub fn set_block(&mut self, p1: Position, p2: Position, value: T) {
        for i in p1.y..=p2.y {
            for j in p1.x..=p2.x {
                self.set(Position { x: j, y: i }, value.clone());
            }
        }
    }
}

pub struct TileLayoutMesh<T> {
    tiles: Vec<(Tile<T>, Position)>,
    width: usize,
    height: usize,
}

impl<T: Display> TileLayoutMesh<T> {
    pub fn from_tiles(tiles: Vec<Tile<T>>, width: usize) -> Self {
        let mut tiles = tiles;
        tiles.sort_by_key(|t| std::cmp::Reverse(t.size()));
        let tiles_id_map = tiles.into_iter().enumerate().collect::<BTreeMap<_, _>>();

        let mut map = HeightExpandableMap::<usize>::new(width, 0);
        let mut width_id_map = tiles_id_map.iter().fold(
            SortedContainer::<usize, usize>::new(),
            |mut acc, (id, v)| {
                acc.insert(v.width, *id);
                acc
            },
        );

        let mut y = 0;
        'outer: loop {
            for x in 0..width {
                if map.get(Position::new(x, y)).is_some() {
                    continue;
                }
                let mut space = 1;
                for i in (x + 1)..width {
                    if map.get(Position::new(i, y)).is_some() {
                        break;
                    }
                    space += 1;
                }
                let tile_id = match width_id_map.pop_lesser(space) {
                    PopLesserOption::Some(tile) => tile,
                    PopLesserOption::Empty => {
                        break 'outer;
                    }
                    PopLesserOption::NoLesser => {
                        continue;
                    }
                };
                let tile = tiles_id_map.get(&tile_id).expect("impossible");
                map.set_block(
                    Position::new(x, y),
                    Position::new(x + tile.width - 1, y + tile.height - 1),
                    tile_id,
                );
            }
            y += 1;
        }
        for y in 0..map.height() {
            for x in 0..map.width() {
                print!(
                    "{}",
                    map.get(Position::new(x, y))
                        .map(|v| to_chinese_character_ordered(*v))
                        .unwrap_or('　')
                );
            }
            print!("\n");
        }

        todo!()
    }
}

fn to_chinese_character_ordered(i: usize) -> char {
    #[rustfmt::skip]
    static CHARS: [char; 96] = [
        '甲', '乙', '丙', '丁', '戊', '己', '庚', '辛', '壬', '癸',
        '子', '丑', '寅', '卯', '辰', '巳', '午', '未', '申', '酉', '戌', '亥',
        '一', '二', '三', '四', '五', '六', '七', '八', '九', '十',
        '壹', '贰', '叁', '肆', '伍', '陆', '柒', '捌', '玖', '拾',
        '天', '地', '玄', '黄', '宇', '宙', '洪', '荒',
        '乾', '兑', '离', '震', '巽', '坎', '艮', '坤',
        '金', '木', '水', '火', '土',
        '角', '亢', '氐', '房', '心', '尾', '箕', '斗', '牛', '女', '虚',
        '危', '室', '壁', '奎', '娄', '胃', '昴', '毕', '觜', '参', '井',
        '鬼', '柳', '星', '张', '翼', '轸', '宫', '商', '角', '徵', '羽',
    ];
    CHARS[i]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test() {
        let tiles = [
            (7, 11),
            (6, 9),
            (6, 6),
            (4, 7),
            (3, 5),
            (2, 4),
            (1, 2),
            (1, 2),
            (1, 2),
            (1, 2),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (w, h))| Tile::new((b'A' + i as u8) as char, w, h))
        .collect::<Vec<Tile<char>>>();
        let _ = TileLayoutMesh::from_tiles(tiles, 12);
    }
}
