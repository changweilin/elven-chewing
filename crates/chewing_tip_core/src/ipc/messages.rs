use serde::{Deserialize, Serialize};

// TODO: make sure the coordinate is DPI aware
#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ShowNotification {
    pub position: Position,
    pub text: String,
    pub font_family: String,
    pub font_size: f32,
    pub fg_color: String,
    pub bg_color: String,
    pub border_color: String,
}
pub type ShowNotificationReply = ();
impl ShowNotification {
    pub const METHOD: &str = "im.chewing.ui.ShowNotification";
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ShowCandidateList {
    pub position: Position,
    pub items: Vec<String>,
    pub selkeys: Vec<u16>,
    pub total_page: u32,
    pub current_page: u32,
    pub font_family: String,
    pub font_size: f32,
    pub cand_per_row: u32,
    pub use_cursor: bool,
    pub current_sel: usize,
    pub selkey_color: String,
    pub fg_color: String,
    pub bg_color: String,
    pub highlight_fg_color: String,
    pub highlight_bg_color: String,
    pub border_color: String,
}
pub type ShowCandidateListReply = ();
impl ShowCandidateList {
    pub const METHOD: &str = "im.chewing.ui.ShowCandidateList";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HideCandidateList;
pub type HideCandidateListReply = ();
impl HideCandidateList {
    pub const METHOD: &str = "im.chewing.ui.HideCandidateList";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Stop;
pub type StopReply = ();
impl Stop {
    pub const METHOD: &str = "im.chewing.ui.Stop";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct CheckUpdate;
pub type CheckUpdateReply = ();
impl CheckUpdate {
    pub const METHOD: &str = "im.chewing.ui.CheckUpdate";
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(default)]
pub struct ShowDualPreview {
    pub position: Position,
    pub chinese: String,
    pub english: String,
    /// 0 = Chinese active, 1 = English active
    pub active: u8,
    pub font_family: String,
    pub font_size: f32,
    pub fg_color: String,
    pub bg_color: String,
    pub highlight_fg_color: String,
    pub highlight_bg_color: String,
    pub border_color: String,
}
pub type ShowDualPreviewReply = ();
impl ShowDualPreview {
    pub const METHOD: &str = "im.chewing.ui.ShowDualPreview";
}

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct HideDualPreview;
pub type HideDualPreviewReply = ();
impl HideDualPreview {
    pub const METHOD: &str = "im.chewing.ui.HideDualPreview";
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{ShowCandidateList, ShowDualPreview};
    use crate::ipc::varlink::MethodCall;

    #[test]
    fn candidate_list_accepts_missing_fields_from_older_tip() {
        let msg: ShowCandidateList = serde_json::from_value(json!({
            "position": { "x": 12, "y": 34 },
            "items": ["候"],
            "selkeys": [49],
            "futureField": "ignored"
        }))
        .unwrap();

        assert_eq!(msg.position.x, 12);
        assert_eq!(msg.position.y, 34);
        assert_eq!(msg.items, vec!["候"]);
        assert_eq!(msg.selkeys, vec![49]);
        assert_eq!(msg.font_size, 0.0);
        assert_eq!(msg.border_color, "");
    }

    #[test]
    fn dual_preview_accepts_minimal_payload() {
        let msg: ShowDualPreview = serde_json::from_value(json!({
            "chinese": "你好",
            "english": "su3cl3"
        }))
        .unwrap();

        assert_eq!(msg.chinese, "你好");
        assert_eq!(msg.english, "su3cl3");
        assert_eq!(msg.active, 0);
        assert_eq!(msg.position.x, 0);
        assert_eq!(msg.position.y, 0);
    }

    #[test]
    fn method_call_flags_default_for_minimal_varlink_message() {
        let call: MethodCall = serde_json::from_value(json!({
            "method": "im.chewing.ui.HideCandidateList"
        }))
        .unwrap();

        assert_eq!(call.method, "im.chewing.ui.HideCandidateList");
        assert!(call.parameters.is_null());
        assert_eq!(call.oneway, None);
        assert_eq!(call.more, None);
        assert_eq!(call.upgrade, None);
    }
}
