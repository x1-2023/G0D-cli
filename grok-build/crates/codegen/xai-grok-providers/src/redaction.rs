use crate::request::ModelRequest;
use crate::response::ModelResponse;

pub fn redact_request_credentials(req: &mut ModelRequest) {
    req.extra_headers.retain(|(k, _)| {
        !k.to_lowercase().contains("auth") && !k.to_lowercase().contains("api-key")
    });
}

pub fn redact_response_credentials(resp: &mut ModelResponse) {
    if let Some(ref mut meta) = resp.model_metadata {
        if let Some(obj) = meta.as_object_mut() {
            obj.retain(|k, _| {
                let lower = k.to_lowercase();
                !lower.contains("auth") && !lower.contains("key") && !lower.contains("token")
            });
        }
    }
}

pub fn sanitize_error_for_display(err: &crate::error::ProviderError) -> String {
    let msg = err.to_string();
    msg
}

pub fn sanitize_for_export(text: &str) -> String {
    let sanitized = text.to_string();
    sanitized
}
