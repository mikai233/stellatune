use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct InstalledPluginInfo {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub install_state: Option<String>,
}

impl InstalledPluginInfo {
    pub fn display_name(&self) -> String {
        let name = self.name.as_deref().unwrap_or("").trim();
        if name.is_empty() {
            self.id.clone()
        } else {
            name.to_string()
        }
    }
}
