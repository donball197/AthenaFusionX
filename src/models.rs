use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct GemmaRequest {
    pub contents: Vec<Content>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Content {
    pub role: String,
    pub parts: Vec<Part>,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(untagged)]
pub enum Part {
    Text { text: String },
    InlineData { inline_data: InlineData },
}

#[derive(Serialize, Deserialize, Debug)]
pub struct InlineData {
    pub mime_type: String,
    pub data: String, // Base64 encoded string
}

#[derive(Deserialize, Debug)]
pub struct GemmaResponse {
    pub candidates: Vec<Candidate>,
}

#[derive(Deserialize, Debug)]
pub struct Candidate {
    pub content: ContentResponse,
    pub finish_reason: Option<String>,
}

#[derive(Deserialize, Debug)]
pub struct ContentResponse {
    pub parts: Vec<TextPart>,
}

#[derive(Deserialize, Debug)]
pub struct TextPart {
    pub text: String,
}
