use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LlmOutput {
    #[serde(rename = "intro-structures")]
    pub intro_structures: Vec<String>,
    pub fragments: Vec<RawFragment>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RawFragment {
    pub fragment: String,
    pub criteria: String,
    pub dependencies: Vec<String>,
    pub placeholders: Vec<String>,
}
