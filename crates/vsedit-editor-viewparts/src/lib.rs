//! Editor view parts: view zones, overlay widgets, content widgets, glyph margins.

/// Line number rendering mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineNumberMode {
    Absolute,
    Relative,
    Interval(u32),
}

/// Format a line number for display.
pub fn format_line_number(line: u32, current_line: u32, mode: LineNumberMode) -> String {
    match mode {
        LineNumberMode::Absolute => format!("{line}"),
        LineNumberMode::Relative => {
            if line == current_line {
                format!("{line}")
            } else {
                format!("{}", (line as i64 - current_line as i64).unsigned_abs())
            }
        }
        LineNumberMode::Interval(n) => {
            if line == current_line || line % n == 0 {
                format!("{line}")
            } else {
                String::new()
            }
        }
    }
}

/// Ruler position.
pub struct Ruler {
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ViewZone {
    pub id: u64,
    pub after_line: u32,
    pub height_in_lines: u32,
    pub content: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OverlayWidget {
    pub id: String,
    pub position_top: u32,
    pub position_left: u32,
    pub content: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentWidget {
    pub id: String,
    pub line: u32,
    pub column: u32,
    pub content: String,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphMarginWidget {
    pub line: u32,
    pub glyph: String,
    pub tooltip: Option<String>,
}

pub struct EditorViewParts {
    pub zones: Vec<ViewZone>,
    pub overlays: Vec<OverlayWidget>,
    pub content_widgets: Vec<ContentWidget>,
    pub glyph_margins: Vec<GlyphMarginWidget>,
    next_zone_id: u64,
}

impl EditorViewParts {
    pub fn new() -> Self {
        Self {
            zones: Vec::new(),
            overlays: Vec::new(),
            content_widgets: Vec::new(),
            glyph_margins: Vec::new(),
            next_zone_id: 1,
        }
    }

    pub fn add_view_zone(&mut self, after_line: u32, height_in_lines: u32) -> u64 {
        let id = self.next_zone_id;
        self.next_zone_id += 1;
        self.zones.push(ViewZone {
            id,
            after_line,
            height_in_lines,
            content: None,
        });
        id
    }

    pub fn remove_view_zone(&mut self, id: u64) {
        self.zones.retain(|z| z.id != id);
    }

    pub fn add_overlay(&mut self, widget: OverlayWidget) {
        self.overlays.push(widget);
    }

    pub fn remove_overlay(&mut self, id: &str) {
        self.overlays.retain(|o| o.id != id);
    }

    pub fn add_glyph_margin(&mut self, widget: GlyphMarginWidget) {
        self.glyph_margins.push(widget);
    }

    pub fn get_view_zones(&self) -> &[ViewZone] {
        &self.zones
    }
}

impl Default for EditorViewParts {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_remove_view_zones() {
        let mut parts = EditorViewParts::new();
        let id1 = parts.add_view_zone(10, 3);
        let id2 = parts.add_view_zone(20, 5);
        assert_eq!(parts.get_view_zones().len(), 2);
        parts.remove_view_zone(id1);
        assert_eq!(parts.get_view_zones().len(), 1);
        assert_eq!(parts.get_view_zones()[0].id, id2);
    }

    #[test]
    fn zone_ids_increment() {
        let mut parts = EditorViewParts::new();
        let id1 = parts.add_view_zone(1, 1);
        let id2 = parts.add_view_zone(2, 1);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
    }

    #[test]
    fn add_and_remove_overlay() {
        let mut parts = EditorViewParts::new();
        parts.add_overlay(OverlayWidget {
            id: "find".into(),
            position_top: 0,
            position_left: 100,
            content: "Find widget".into(),
            visible: true,
        });
        assert_eq!(parts.overlays.len(), 1);
        parts.remove_overlay("find");
        assert!(parts.overlays.is_empty());
    }

    #[test]
    fn add_glyph_margin() {
        let mut parts = EditorViewParts::new();
        parts.add_glyph_margin(GlyphMarginWidget {
            line: 5,
            glyph: "●".into(),
            tooltip: Some("Breakpoint".into()),
        });
        assert_eq!(parts.glyph_margins.len(), 1);
        assert_eq!(parts.glyph_margins[0].line, 5);
    }
}
