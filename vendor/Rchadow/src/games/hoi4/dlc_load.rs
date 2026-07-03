use serde::{Deserialize, Serialize};

/// HOI4 launcher-compatible load document.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DlcLoadDocument {
    /// Enabled launcher mod descriptor paths.
    pub enabled_mods: Vec<String>,

    /// Disabled DLC launcher paths.
    pub disabled_dlcs: Vec<String>,
}
