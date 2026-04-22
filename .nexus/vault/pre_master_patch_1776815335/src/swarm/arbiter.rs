pub struct Arbiter;

impl Arbiter {
    pub fn score(text: &str) -> u32 {
        let mut s: i32 = 50;
        let lower = text.to_lowercase();

        if lower.contains("error") || lower.contains("fault") { s -= 30; }
        if lower.contains("success") || lower.contains("complete") { s += 20; }
        if text.len() > 150 { s += 10; }

        if s < 0 { 0 } else { s as u32 }
    }

    pub fn best(responses: Vec<String>) -> String {
        responses
            .into_iter()
            .max_by_key(|r| Self::score(r))
            .unwrap_or_else(|| "EMPTY".to_string())
    }
}
