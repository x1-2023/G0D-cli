use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub mime_type: String,
    pub base64_data: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub file_name: Option<String>,
    pub file_size: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MultimodalInput {
    pub text: Option<String>,
    pub images: Vec<ImageAttachment>,
    pub audio: Vec<AudioAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioAttachment {
    pub mime_type: String,
    pub base64_data: String,
    pub duration_seconds: Option<f64>,
}

pub const SUPPORTED_IMAGE_TYPES: &[&str] = &["image/png", "image/jpeg", "image/gif", "image/webp"];
pub const MAX_IMAGE_SIZE_BYTES: u64 = 20 * 1024 * 1024;

impl MultimodalInput {
    pub fn is_empty(&self) -> bool {
        self.text.is_none() && self.images.is_empty() && self.audio.is_empty()
    }

    pub fn has_visual_input(&self) -> bool {
        !self.images.is_empty()
    }

    pub fn total_size_bytes(&self) -> u64 {
        self.images.iter().map(|i| i.base64_data.len() as u64).sum::<u64>()
            + self.audio.iter().map(|a| a.base64_data.len() as u64).sum::<u64>()
    }
}
