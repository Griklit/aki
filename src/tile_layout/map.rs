pub use super::Position;

pub struct HeightExpandableMap<T: Clone> {
    map: Vec<Option<T>>,
    width: usize,
    height: usize,
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
        for y in 0..self.height() {
            for x in 0..self.width() {
                print!(
                    "{}",
                    self.get(Position::new(x, y))
                        .map(|v| to_chinese_character_ordered(*v))
                        .unwrap_or('　')
                );
            }
            print!("\n");
        }
    }
}
