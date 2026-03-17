use crate::llm_output::LlmOutput;
use std::fmt;

#[derive(Debug)]
pub enum JsonParseError {
    Json(serde_json::Error),
}

impl fmt::Display for JsonParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonParseError::Json(e) => write!(f, "JSON parsing error: {e}"),
        }
    }
}

impl std::error::Error for JsonParseError {}

impl From<serde_json::Error> for JsonParseError {
    fn from(e: serde_json::Error) -> Self {
        JsonParseError::Json(e)
    }
}

/// Parse LLM JSON output generously: try direct parse first, then strip
/// `` ```json `` / `` ``` `` code fence wrapping.
pub fn parse_llm_json(raw: &str) -> Result<LlmOutput, JsonParseError> {
    // Try direct parse
    match serde_json::from_str::<LlmOutput>(raw) {
        Ok(obj) => return Ok(obj),
        Err(_) => {}
    }

    // Strip code fence wrapping and retry
    let trimmed = raw
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    Ok(serde_json::from_str::<LlmOutput>(trimmed)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_JSON: &str = r#"{
        "intro-structures": ["use std::fmt;"],
        "fragments": [
            {
                "fragment": "fmt",
                "criteria": "1",
                "dependencies": ["use std::fmt;"],
                "placeholders": []
            }
        ]
    }"#;

    #[test]
    fn parse_direct_json() {
        let result = parse_llm_json(VALID_JSON);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.intro_structures.len(), 1);
        assert_eq!(output.fragments.len(), 1);
        assert_eq!(output.fragments[0].fragment, "fmt");
    }

    #[test]
    fn parse_fenced_json() {
        let fenced = format!("```json\n{}\n```", VALID_JSON);
        let result = parse_llm_json(&fenced);
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.fragments[0].fragment, "fmt");
    }

    #[test]
    fn parse_malformed_json() {
        let result = parse_llm_json("this is not json at all");
        assert!(result.is_err());
    }
}
