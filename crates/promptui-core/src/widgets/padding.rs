/// Padding configuration for widgets.
///
/// Each value is a number of cells (columns for horizontal, rows for vertical).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Padding {
    pub left: u16,
    pub right: u16,
    pub top: u16,
    pub bottom: u16,
}

impl Padding {
    /// Uniform padding on all sides.
    pub fn uniform(n: u16) -> Self {
        Self {
            left: n,
            right: n,
            top: n,
            bottom: n,
        }
    }

    /// Separate horizontal and vertical padding.
    pub fn symmetric(horizontal: u16, vertical: u16) -> Self {
        Self {
            left: horizontal,
            right: horizontal,
            top: vertical,
            bottom: vertical,
        }
    }

    /// Total horizontal padding (left + right).
    pub fn horizontal(&self) -> u16 {
        self.left + self.right
    }

    /// Total vertical padding (top + bottom).
    pub fn vertical(&self) -> u16 {
        self.top + self.bottom
    }
}

impl Default for Padding {
    fn default() -> Self {
        Self::uniform(1)
    }
}
