use std::collections::HashMap;

use anyhow::{Context, Result, anyhow};
use fontdb::{Database, Family, Query, Stretch, Style, Weight};
use vello::peniko::{Blob, FontData};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontHandle(u64);

#[derive(Debug, Clone)]
pub struct FontResource {
    font: FontData,
    family_name: String,
}

impl FontResource {
    pub fn font_data(&self) -> &FontData {
        &self.font
    }

    pub fn family_name(&self) -> &str {
        &self.family_name
    }
}

#[derive(Debug)]
pub struct FontCatalog {
    db: Database,
    next_id: u64,
    fonts: HashMap<FontHandle, FontResource>,
}

impl Default for FontCatalog {
    fn default() -> Self {
        let mut db = Database::new();
        db.load_system_fonts();
        Self {
            db,
            next_id: 0,
            fonts: HashMap::new(),
        }
    }
}

impl FontCatalog {
    pub fn load_ui_font(&mut self) -> Result<FontHandle> {
        const FAMILIES: &[Family<'static>] = &[
            Family::Name("SF Pro Display"),
            Family::Name("Helvetica Neue"),
            Family::Name("Segoe UI"),
            Family::Name("Noto Sans"),
            Family::SansSerif,
        ];

        let query = Query {
            families: FAMILIES,
            weight: Weight::SEMIBOLD,
            stretch: Stretch::Normal,
            style: Style::Normal,
        };
        let id = self
            .db
            .query(&query)
            .ok_or_else(|| anyhow!("no suitable UI font found in system database"))?;
        let family_name = self
            .db
            .face(id)
            .and_then(|face| face.families.first())
            .map(|family| family.0.clone())
            .unwrap_or_else(|| "sans-serif".to_owned());
        let font = self
            .db
            .with_face_data(id, |bytes, face_index| {
                FontData::new(Blob::from(bytes.to_vec()), face_index)
            })
            .context("load font bytes from database")?;

        let handle = self.next_handle();
        self.fonts
            .insert(handle, FontResource { font, family_name });
        Ok(handle)
    }

    pub fn get(&self, handle: FontHandle) -> Option<&FontResource> {
        self.fonts.get(&handle)
    }

    fn next_handle(&mut self) -> FontHandle {
        let handle = FontHandle(self.next_id);
        self.next_id = self.next_id.saturating_add(1);
        handle
    }
}
