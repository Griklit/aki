use super::{HeightExpandableMap, Position, Tile};
use std::{
    collections::{BTreeMap, VecDeque},
    fmt::Display,
};

enum PopLesserOption<T> {
    Some(T),
    NoLesser,
    Empty,
}

struct SortedContainer<T, K: Ord + Clone> {
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
}

#[derive(Debug)]
pub struct TileLayoutMesh<T> {
    pub tiles: Vec<(Tile<T>, Position)>,
    pub width: usize,
    pub height: usize,
}

impl<T: Display> TileLayoutMesh<T> {
    pub fn from_tiles(tiles: Vec<Tile<T>>, width: usize) -> Result<Self, String> {
        for tile in &tiles {
            if tile.width > width {
                return Err("the tile width is greater than the mesh width".to_string());
            }
        }
        let mut tiles = tiles;
        tiles.sort_by_key(|t| std::cmp::Reverse(t.size()));
        let mut tiles_id_map = tiles.into_iter().enumerate().collect::<BTreeMap<_, _>>();
        let mut map = HeightExpandableMap::<usize>::new(width, 0);
        let mut width_id_map = tiles_id_map.iter().fold(
            SortedContainer::<usize, usize>::new(),
            |mut acc, (id, v)| {
                acc.insert(v.width, *id);
                acc
            },
        );
        let mut result_map = BTreeMap::new();
        let mut y = 0;
        'outer: loop {
            for x in 0..width {
                let p = Position::new(x, y);
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
                    p,
                    Position::new(x + tile.width - 1, y + tile.height - 1),
                    tile_id,
                );
                result_map.insert(tile_id, p);
            }
            y += 1;
        }
        #[cfg(test)]
        map.print();
        let tiles = result_map
            .into_iter()
            .filter_map(|(id, p)| tiles_id_map.remove(&id).map(|v| (v, p)))
            .collect::<Vec<_>>();
        Ok(Self {
            tiles,
            width,
            height: map.height(),
        })
    }
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
            (1, 4),
            (1, 3),
            (1, 2),
            (1, 2),
            (1, 2),
            (1, 2),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
            (1, 1),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (w, h))| Tile::new(i, w, h))
        .collect::<Vec<Tile<usize>>>();
        let mesh = TileLayoutMesh::from_tiles(tiles, 12).unwrap();
        dbg!(mesh);
    }
}
