const PROMPT_FRONT: &str = include_str!("../prompts/front.txt");
const PROMPT_END: &str = include_str!("../prompts/end.txt");

/// Build the full LLM prompt by sandwiching the Rust source between the
/// front and end prompt templates.
pub fn build_prompt(rust_source: &str) -> String {
    let mut prompt = String::with_capacity(PROMPT_FRONT.len() + rust_source.len() + PROMPT_END.len());
    prompt.push_str(PROMPT_FRONT);
    prompt.push_str(rust_source);
    prompt.push('\n');
    prompt.push_str(PROMPT_END);
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_contains_all_parts() {
        let source = "fn main() {}";
        let prompt = build_prompt(source);
        assert!(prompt.contains(PROMPT_FRONT));
        assert!(prompt.contains(source));
        assert!(prompt.contains(PROMPT_END));
    }

    #[test]
    fn build_prompt_order() {
        let source = "struct Foo;";
        let prompt = build_prompt(source);
        let front_pos = prompt.find(PROMPT_FRONT).unwrap();
        let source_pos = prompt.find(source).unwrap();
        let end_pos = prompt.find(PROMPT_END).unwrap();
        assert!(front_pos < source_pos);
        assert!(source_pos < end_pos);
    }
}
