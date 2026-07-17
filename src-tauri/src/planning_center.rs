//! Planning Center Online (Services) import. Given a Personal Access Token
//! (Application ID + Secret) and a plan, fetch the plan's item list so the
//! operator can pull a Sunday's order of service into the app. This is the one
//! deliberately-online feature — it does nothing unless the operator asks and
//! provides their own church's credentials.

use serde::Serialize;

const B64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Minimal standard base64 (for the HTTP Basic auth header), so we don't pull a
/// crate just for this.
fn base64_encode(input: &[u8]) -> String {
    let mut out = String::new();
    for chunk in input.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | (b[2] as u32);
        out.push(B64[((n >> 18) & 63) as usize] as char);
        out.push(B64[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { B64[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { B64[(n & 63) as usize] as char } else { '=' });
    }
    out
}

fn basic_auth(app_id: &str, secret: &str) -> String {
    format!("Basic {}", base64_encode(format!("{app_id}:{secret}").as_bytes()))
}

/// One item in a Planning Center plan (a song, header, media, scripture, …).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanItem {
    pub title: String,
    pub kind: String,
}

/// Fetch a plan's ordered items from the Planning Center Services API.
pub fn fetch_plan(
    app_id: &str,
    secret: &str,
    service_type_id: &str,
    plan_id: &str,
) -> Result<Vec<PlanItem>, String> {
    let url = format!(
        "https://api.planningcenteronline.com/services/v2/service_types/{service_type_id}/plans/{plan_id}/items?per_page=100"
    );
    let resp = ureq::get(&url)
        .set("Authorization", &basic_auth(app_id, secret))
        .call()
        .map_err(|e| format!("Planning Center request failed: {e}"))?;
    let json: serde_json::Value = resp.into_json().map_err(|e| e.to_string())?;
    let data = json
        .get("data")
        .and_then(|d| d.as_array())
        .ok_or_else(|| "unexpected Planning Center response".to_string())?;
    Ok(parse_items(data))
}

/// Pull `{title, item_type}` out of the API's item array. Split out so it's
/// testable without a network call.
fn parse_items(data: &[serde_json::Value]) -> Vec<PlanItem> {
    data.iter()
        .filter_map(|it| {
            let attr = it.get("attributes")?;
            let title = attr.get("title").and_then(|t| t.as_str()).unwrap_or("").trim().to_string();
            if title.is_empty() {
                return None;
            }
            let kind = attr
                .get("item_type")
                .and_then(|t| t.as_str())
                .unwrap_or("item")
                .to_string();
            Some(PlanItem { title, kind })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_known_vectors() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        assert_eq!(base64_encode(b"app:secret"), "YXBwOnNlY3JldA==");
    }

    #[test]
    fn parses_items_and_skips_untitled() {
        let data = serde_json::json!([
            {"attributes": {"title": "Amazing Grace", "item_type": "song"}},
            {"attributes": {"title": "", "item_type": "header"}},
            {"attributes": {"title": "Welcome", "item_type": "header"}},
            {"attributes": {"item_type": "media"}}
        ]);
        let items = parse_items(data.as_array().unwrap());
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Amazing Grace");
        assert_eq!(items[0].kind, "song");
        assert_eq!(items[1].title, "Welcome");
    }
}
