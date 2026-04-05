use crate::ui::layout::geometry::LayoutSize;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutConstraints {
    pub min: LayoutSize,
    pub max: LayoutSize,
}

impl LayoutConstraints {
    pub fn tight(size: LayoutSize) -> Self {
        Self {
            min: size,
            max: size,
        }
    }

    pub fn loosen(self) -> Self {
        Self {
            min: LayoutSize::new(0.0, 0.0),
            max: self.max,
        }
    }

    pub fn clamp(self, size: LayoutSize) -> LayoutSize {
        LayoutSize::new(
            size.width.clamp(self.min.width, self.max.width),
            size.height.clamp(self.min.height, self.max.height),
        )
    }
}
