use serde::{Deserialize, Serialize};

/// Data for a title block on a print layout sheet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TitleBlockData {
    pub project_name: String,
    pub drawing_title: String,
    pub drawn_by: String,
    pub date: String,
    pub scale: String,
    pub sheet_number: String,
    pub revision: String,
    pub client: String,
    pub notes: String,
}

impl Default for TitleBlockData {
    fn default() -> Self {
        Self {
            project_name: String::new(),
            drawing_title: String::new(),
            drawn_by: String::new(),
            date: String::new(),
            scale: "1:100".into(),
            sheet_number: "1 of 1".into(),
            revision: "A".into(),
            client: String::new(),
            notes: String::new(),
        }
    }
}
