use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalizationConfig {
    pub response_language: String,
    pub ui_language: String,
    pub translate_provider_errors: bool,
    pub preserve_original_provider_error: bool,
    pub normalize_unicode: bool,
}

impl Default for LocalizationConfig {
    fn default() -> Self {
        Self {
            response_language: "auto".into(),
            ui_language: "en".into(),
            translate_provider_errors: true,
            preserve_original_provider_error: true,
            normalize_unicode: true,
        }
    }
}

pub fn ui_string<'a>(key: &'a str, ui_lang: &str) -> &'a str {
    if ui_lang == "vi" {
        vi_str(key).unwrap_or(key)
    } else {
        key
    }
}

fn vi_str(key: &str) -> Option<&'static str> {

    let result = match key {
        "GODMODE enabled" => "Đã bật GODMODE",
        "GODMODE disabled" => "Đã tắt GODMODE",
        "Race started" => "Bắt đầu cuộc đua",
        "Candidate queued" => "Đang chờ",
        "Candidate running" => "Đang xử lý",
        "Candidate completed" => "Hoàn thành",
        "Candidate failed" => "Thất bại",
        "Candidate refused" => "Từ chối trả lời",
        "Judging" => "Đang chấm điểm",
        "Winner" => "Người thắng",
        "Confidence" => "Độ tin cậy",
        "Provider" => "Nhà cung cấp",
        "Model" => "Mô hình",
        "Persona" => "Vai trò",
        "Score" => "Điểm",
        "Latency" => "Độ trễ",
        "Input tokens" => "Token đầu vào",
        "Output tokens" => "Token đầu ra",
        "Estimated cost" => "Chi phí ước tính",
        "Budget warning" => "Cảnh báo ngân sách",
        "Permission required" => "Cần cấp quyền",
        "Command requires confirmation" => "Lệnh cần xác nhận",
        "Race cancelled" => "Đã hủy cuộc đua",
        "Race completed" => "Cuộc đua hoàn tất",
        "Local-only mode" => "Chế độ chỉ chạy cục bộ",
        "No-log mode" => "Chế độ không ghi log",
        "Privacy preview" => "Xem trước quyền riêng tư",
        "Context sent remotely" => "Dữ liệu gửi tới dịch vụ từ xa",
        "No valid candidate" => "Không có ứng viên hợp lệ",
        "Provider unavailable" => "Nhà cung cấp không khả dụng",
        "Authentication failed" => "Xác thực thất bại",
        "Rate limited" => "Bị giới hạn tốc độ",
        "Timed out" => "Hết thời gian chờ",
        "Retrying" => "Đang thử lại",
        "Export completed" => "Đã xuất dữ liệu",
        "Error" => "Lỗi",
        "Warning" => "Cảnh báo",
        "Status" => "Trạng thái",
        "Race" => "Cuộc đua",
        "Candidates" => "Ứng viên",
        "Judges" => "Giám khảo",
        "Tournament" => "Vòng đấu",
        "Round" => "Vòng",
        "Usage" => "Mức sử dụng",
        "Cost" => "Chi phí",
        "Disqualified" => "Bị loại",
        "Merged" => "Đã gộp",
        "Overridden" => "Đã ghi đè",
        _ => return None,
    };
    Some(result)
}

pub fn localized_provider_error(error: &str, ui_lang: &str) -> String {
    if ui_lang != "vi" { return error.to_string(); }

    let lower = error.to_lowercase();
    let explanation = if lower.contains("401") || lower.contains("unauthorized") || lower.contains("xác thực") {
        "Xác thực thất bại. Kiểm tra lại API key của bạn."
    } else if lower.contains("429") || lower.contains("rate") || lower.contains("giới hạn") {
        "Bị giới hạn tốc độ. Vui lòng đợi và thử lại sau."
    } else if lower.contains("404") || lower.contains("not found") || lower.contains("không tìm thấy") {
        "Không tìm thấy mô hình hoặc endpoint."
    } else if lower.contains("timeout") || lower.contains("timed out") || lower.contains("hết thời gian") {
        "Hết thời gian chờ. Máy chủ không phản hồi kịp."
    } else if lower.contains("context") && lower.contains("exceed") {
        "Vượt quá giới hạn context window."
    } else if lower.contains("connect") || lower.contains("network") || lower.contains("refused") {
        "Lỗi kết nối mạng. Kiểm tra endpoint và kết nối internet."
    } else if lower.contains("invalid") {
        "Yêu cầu không hợp lệ. Kiểm tra tham số gửi đi."
    } else if lower.contains("unavailable") || lower.contains("không khả dụng") {
        "Nhà cung cấp hiện không khả dụng."
    } else {
        return error.to_string();
    };

    format!("{}\n\nLỗi gốc:\n{}", explanation, error)
}
