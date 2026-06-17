#[derive(Debug, Clone, Copy)]
pub struct Size {
    pub w: usize,
    pub h: usize,
}

impl Size {
    #[must_use]
    #[inline]
    pub fn new(w: usize, h: usize) -> Self {
        Self { w, h }
    }

    #[inline]
    pub fn area(&self) -> usize {
        self.w * self.h
    }
}
