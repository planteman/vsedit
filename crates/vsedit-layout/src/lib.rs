//! Flexbox-like terminal layout engine.
use vsedit_tui::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy)]
pub enum Constraint {
    Fixed(u16),
    Percentage(u16),
    Min(u16),
    Max(u16),
    /// Flex grow factor — remaining space is distributed proportionally.
    Flex(u16),
}

pub struct LayoutNode {
    pub direction: Direction,
    pub constraints: Vec<Constraint>,
}

impl LayoutNode {
    pub fn horizontal(constraints: Vec<Constraint>) -> Self {
        Self {
            direction: Direction::Horizontal,
            constraints,
        }
    }

    pub fn vertical(constraints: Vec<Constraint>) -> Self {
        Self {
            direction: Direction::Vertical,
            constraints,
        }
    }

    /// Split `area` according to the configured constraints.
    pub fn split(&self, area: Rect) -> Vec<Rect> {
        let n = self.constraints.len();
        if n == 0 {
            return vec![];
        }

        let total = match self.direction {
            Direction::Horizontal => area.width,
            Direction::Vertical => area.height,
        };

        let mut sizes: Vec<u16> = vec![0; n];
        let mut remaining = total;
        let mut flex_total: u16 = 0;

        // First pass: allocate Fixed, Percentage, Min, Max; accumulate Flex.
        for (i, c) in self.constraints.iter().enumerate() {
            match *c {
                Constraint::Fixed(v) => {
                    let v = v.min(remaining);
                    sizes[i] = v;
                    remaining = remaining.saturating_sub(v);
                }
                Constraint::Percentage(p) => {
                    let v = ((total as u32 * p as u32) / 100) as u16;
                    let v = v.min(remaining);
                    sizes[i] = v;
                    remaining = remaining.saturating_sub(v);
                }
                Constraint::Min(v) => {
                    let v = v.min(remaining);
                    sizes[i] = v;
                    remaining = remaining.saturating_sub(v);
                }
                Constraint::Max(v) => {
                    let v = v.min(remaining);
                    sizes[i] = v;
                    remaining = remaining.saturating_sub(v);
                }
                Constraint::Flex(f) => {
                    flex_total = flex_total.saturating_add(f);
                }
            }
        }

        // Second pass: distribute remaining space to Flex constraints.
        if flex_total > 0 {
            let flex_remaining = remaining;
            for (i, c) in self.constraints.iter().enumerate() {
                if let Constraint::Flex(f) = *c {
                    let v = ((flex_remaining as u32 * f as u32) / flex_total as u32) as u16;
                    sizes[i] = v;
                }
            }
        }

        // Clamp Min/Max after flex distribution.
        for (i, c) in self.constraints.iter().enumerate() {
            match *c {
                Constraint::Min(min_val) => {
                    if sizes[i] < min_val {
                        sizes[i] = min_val.min(total);
                    }
                }
                Constraint::Max(max_val) => {
                    if sizes[i] > max_val {
                        sizes[i] = max_val;
                    }
                }
                _ => {}
            }
        }

        // Build result rects.
        let mut rects = Vec::with_capacity(n);
        let mut offset: u16 = 0;

        for size in &sizes {
            let rect = match self.direction {
                Direction::Horizontal => Rect::new(area.x + offset, area.y, *size, area.height),
                Direction::Vertical => Rect::new(area.x, area.y + offset, area.width, *size),
            };
            rects.push(rect);
            offset = offset.saturating_add(*size);
        }

        rects
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(x: u16, y: u16, w: u16, h: u16) -> Rect {
        Rect::new(x, y, w, h)
    }

    #[test]
    fn fixed_horizontal() {
        let node = LayoutNode::horizontal(vec![Constraint::Fixed(10), Constraint::Fixed(20)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], rect(0, 0, 10, 50));
        assert_eq!(rects[1], rect(10, 0, 20, 50));
    }

    #[test]
    fn fixed_vertical() {
        let node = LayoutNode::vertical(vec![Constraint::Fixed(5), Constraint::Fixed(10)]);
        let rects = node.split(rect(0, 0, 80, 24));
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[0], rect(0, 0, 80, 5));
        assert_eq!(rects[1], rect(0, 5, 80, 10));
    }

    #[test]
    fn percentage() {
        let node = LayoutNode::horizontal(vec![Constraint::Percentage(25), Constraint::Percentage(75)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects[0].width, 25);
        assert_eq!(rects[1].width, 75);
    }

    #[test]
    fn flex_even_split() {
        let node = LayoutNode::horizontal(vec![Constraint::Flex(1), Constraint::Flex(1)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects[0].width, 50);
        assert_eq!(rects[1].width, 50);
    }

    #[test]
    fn flex_weighted() {
        let node = LayoutNode::horizontal(vec![Constraint::Flex(1), Constraint::Flex(3)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects[0].width, 25);
        assert_eq!(rects[1].width, 75);
    }

    #[test]
    fn mixed_fixed_and_flex() {
        let node = LayoutNode::horizontal(vec![
            Constraint::Fixed(20),
            Constraint::Flex(1),
            Constraint::Flex(1),
        ]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert_eq!(rects[0].width, 20);
        assert_eq!(rects[1].width, 40);
        assert_eq!(rects[2].width, 40);
    }

    #[test]
    fn min_constraint() {
        let node = LayoutNode::horizontal(vec![Constraint::Min(30)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert!(rects[0].width >= 30);
    }

    #[test]
    fn max_constraint() {
        let node = LayoutNode::horizontal(vec![Constraint::Max(50)]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert!(rects[0].width <= 50);
    }

    #[test]
    fn empty_constraints() {
        let node = LayoutNode::horizontal(vec![]);
        let rects = node.split(rect(0, 0, 100, 50));
        assert!(rects.is_empty());
    }

    #[test]
    fn offset_area() {
        let node = LayoutNode::horizontal(vec![Constraint::Fixed(10), Constraint::Fixed(10)]);
        let rects = node.split(rect(5, 3, 100, 50));
        assert_eq!(rects[0], rect(5, 3, 10, 50));
        assert_eq!(rects[1], rect(15, 3, 10, 50));
    }
}
