pub use super::{Position, Size};

pub struct HeightExpandableMap<T: Clone> {
    map: Vec<Option<T>>,
    size: Size,
}

impl<T: Clone> HeightExpandableMap<T> {
    #[must_use]
    pub fn new(size: Size) -> Self {
        Self {
            map: vec![None; size.w * size.h],
            size,
        }
    }

    #[inline]
    #[must_use]
    pub fn size(&self) -> Size {
        self.size
    }

    fn resize(&mut self, height: usize) {
        self.map.resize(self.size.w * height, None);
        self.size.h = height;
    }

    #[must_use]
    pub fn get(&self, p: Position) -> Option<&T> {
        self.map
            .get(p.y * self.size.w + p.x)
            .and_then(|v| v.as_ref())
    }

    pub fn set(&mut self, p: Position, value: T) {
        if p.y >= self.size.h {
            self.resize(p.y + 1);
        }
        self.map[p.y * self.size.w + p.x] = Some(value);
    }
    pub fn set_block(&mut self, p1: Position, p2: Position, value: T) {
        for i in p1.y..=p2.y {
            for j in p1.x..=p2.x {
                self.set(Position { x: j, y: i }, value.clone());
            }
        }
    }
}

#[cfg(test)]
pub fn to_chinese_character_ordered(i: usize) -> char {
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
impl HeightExpandableMap<usize> {
    pub fn print(&self) {
        for y in 0..self.size().h {
            for x in 0..self.size().w {
                print!(
                    "{}",
                    self.get(Position::new(x, y))
                        .map_or('　', |v| to_chinese_character_ordered(*v))
                );
            }
            println!();
        }
    }
}
