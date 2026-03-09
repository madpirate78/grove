use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::layout::{Orientation, PaperSize};
use super::title_block::TitleBlockData;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum RevisionStatus {
    Preliminary,
    Issued,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revision {
    pub label: String,
    pub date: String,
    pub description: String,
    pub by: String,
}

/// Layer visibility overrides — only stores DIFFERENCES from model-space.
/// Layers not present here inherit model-space visibility.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayerOverrides {
    pub visibility: HashMap<Uuid, bool>,
}

/// A viewport — a window into model space on a sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawingViewport {
    pub id: Uuid,
    pub name: String,
    pub sheet_position: [f32; 2],
    pub sheet_size: [f32; 2],
    pub model_center: [f32; 2],
    pub scale: u32,
    pub layer_overrides: LayerOverrides,
}

impl DrawingViewport {
    pub fn new(name: impl Into<String>, scale: u32) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.into(),
            sheet_position: [20.0, 20.0],
            sheet_size: [400.0, 300.0],
            model_center: [0.0, 0.0],
            scale,
            layer_overrides: LayerOverrides::default(),
        }
    }
}

/// A complete drawing (one printable sheet).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Drawing {
    pub id: Uuid,
    pub number: String,
    pub sub_sheet: String,
    pub title: String,
    pub paper_size: PaperSize,
    pub orientation: Orientation,
    pub viewports: Vec<DrawingViewport>,
    pub title_block: TitleBlockData,
    pub revisions: Vec<Revision>,
    pub revision_status: RevisionStatus,
    pub show_title_block: bool,
    pub show_north_arrow: bool,
    pub show_scale_bar: bool,
    pub show_legend: bool,
    pub margins: [f32; 4],
}

impl Drawing {
    pub fn new(number: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            number: number.into(),
            sub_sheet: String::new(),
            title: title.into(),
            paper_size: PaperSize::A1,
            orientation: Orientation::Landscape,
            viewports: Vec::new(),
            title_block: TitleBlockData::default(),
            revisions: Vec::new(),
            revision_status: RevisionStatus::Preliminary,
            show_title_block: true,
            show_north_arrow: true,
            show_scale_bar: true,
            show_legend: true,
            margins: [10.0, 10.0, 10.0, 10.0],
        }
    }

    /// Full sheet reference, e.g. "01" or "02A".
    pub fn full_sheet_ref(&self) -> String {
        format!("{}{}", self.number, self.sub_sheet)
    }

    /// Most recent revision (newest first).
    pub fn current_revision(&self) -> Option<&Revision> {
        self.revisions.first()
    }

    /// Add a revision with auto-generated label.
    /// Preliminary: P01, P02, ... Issued: A, B, C, ...
    pub fn add_revision(&mut self, description: impl Into<String>, by: impl Into<String>) -> &Revision {
        let label = match self.revision_status {
            RevisionStatus::Preliminary => {
                let n = self.revisions.iter()
                    .filter(|r| r.label.starts_with('P'))
                    .count() + 1;
                format!("P{:02}", n)
            }
            RevisionStatus::Issued => {
                let n = self.revisions.iter()
                    .filter(|r| !r.label.starts_with('P'))
                    .count();
                // A=0, B=1, C=2, ...
                let ch = (b'A' + n as u8) as char;
                ch.to_string()
            }
        };
        let rev = Revision {
            label,
            date: String::new(),
            description: description.into(),
            by: by.into(),
        };
        self.revisions.insert(0, rev);
        &self.revisions[0]
    }

    /// Paper dimensions in mm, applying orientation.
    pub fn paper_dimensions_mm(&self) -> (f32, f32) {
        let (w, h) = self.paper_size.dimensions_mm();
        match self.orientation {
            Orientation::Landscape => (w, h),
            Orientation::Portrait => (h, w),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_full_sheet_ref() {
        let d = Drawing::new("01", "Master Plan");
        assert_eq!(d.full_sheet_ref(), "01");

        let mut d2 = Drawing::new("02", "Detail");
        d2.sub_sheet = "A".into();
        assert_eq!(d2.full_sheet_ref(), "02A");
    }

    #[test]
    fn test_paper_dimensions_landscape() {
        let d = Drawing::new("01", "Test");
        let (w, h) = d.paper_dimensions_mm();
        assert_eq!(w, 841.0);
        assert_eq!(h, 594.0);
    }

    #[test]
    fn test_paper_dimensions_portrait() {
        let mut d = Drawing::new("01", "Test");
        d.orientation = Orientation::Portrait;
        let (w, h) = d.paper_dimensions_mm();
        assert_eq!(w, 594.0);
        assert_eq!(h, 841.0);
    }

    #[test]
    fn test_revision_preliminary_labels() {
        let mut d = Drawing::new("01", "Test");
        d.add_revision("First draft", "AB");
        d.add_revision("Second draft", "AB");
        assert_eq!(d.revisions[0].label, "P02");
        assert_eq!(d.revisions[1].label, "P01");
    }

    #[test]
    fn test_revision_issued_labels() {
        let mut d = Drawing::new("01", "Test");
        d.revision_status = RevisionStatus::Issued;
        d.add_revision("Initial issue", "AB");
        d.add_revision("Revised planting", "AB");
        assert_eq!(d.revisions[0].label, "B");
        assert_eq!(d.revisions[1].label, "A");
    }

    #[test]
    fn test_current_revision() {
        let mut d = Drawing::new("01", "Test");
        assert!(d.current_revision().is_none());
        d.add_revision("Draft", "AB");
        assert_eq!(d.current_revision().unwrap().label, "P01");
    }

    #[test]
    fn test_layer_overrides_default_empty() {
        let overrides = LayerOverrides::default();
        assert!(overrides.visibility.is_empty());
    }

    #[test]
    fn test_viewport_creation() {
        let vp = DrawingViewport::new("Main View", 100);
        assert_eq!(vp.name, "Main View");
        assert_eq!(vp.scale, 100);
    }

    #[test]
    fn test_drawing_serde_roundtrip() {
        let mut d = Drawing::new("01", "Planting Plan");
        d.viewports.push(DrawingViewport::new("Main", 100));
        d.add_revision("First draft", "AB");

        let json = serde_json::to_string(&d).unwrap();
        let d2: Drawing = serde_json::from_str(&json).unwrap();

        assert_eq!(d2.number, "01");
        assert_eq!(d2.title, "Planting Plan");
        assert_eq!(d2.viewports.len(), 1);
        assert_eq!(d2.revisions.len(), 1);
    }
}
