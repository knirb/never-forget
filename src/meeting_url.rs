use std::sync::LazyLock;

use regex::Regex;

static MEETING_PATTERNS: LazyLock<[Regex; 4]> = LazyLock::new(|| [
    Regex::new(r"https?://[\w.-]*zoom\.us/j/[\w?=&]+").unwrap(),
    Regex::new(r"https?://meet\.google\.com/[\w-]+").unwrap(),
    Regex::new(r"https?://teams\.microsoft\.com/l/meetup-join/[\S]+").unwrap(),
    Regex::new(r"https?://[\w.-]*webex\.com/[\S]+").unwrap(),
]);

pub fn extract_meeting_url(text: &str) -> Option<String> {
    for re in MEETING_PATTERNS.iter() {
        if let Some(m) = re.find(text) {
            return Some(m.as_str().to_string());
        }
    }
    None
}

pub fn extract_from_event(location: Option<&str>, notes: Option<&str>) -> Option<String> {
    if let Some(loc) = location {
        if let Some(url) = extract_meeting_url(loc) {
            return Some(url);
        }
    }
    if let Some(n) = notes {
        if let Some(url) = extract_meeting_url(n) {
            return Some(url);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_zoom_url() {
        let text = "Join: https://zoom.us/j/1234567890?pwd=abc123";
        let url = extract_meeting_url(text).unwrap();
        assert_eq!(url, "https://zoom.us/j/1234567890?pwd=abc123");
    }

    #[test]
    fn test_extract_zoom_subdomain() {
        let text = "https://company.zoom.us/j/9876543210";
        let url = extract_meeting_url(text).unwrap();
        assert_eq!(url, "https://company.zoom.us/j/9876543210");
    }

    #[test]
    fn test_extract_google_meet_url() {
        let text = "Meeting at https://meet.google.com/abc-defg-hij";
        let url = extract_meeting_url(text).unwrap();
        assert_eq!(url, "https://meet.google.com/abc-defg-hij");
    }

    #[test]
    fn test_extract_teams_url() {
        let text = "Join here: https://teams.microsoft.com/l/meetup-join/19%3ameeting_abc123";
        let url = extract_meeting_url(text).unwrap();
        assert!(url.starts_with("https://teams.microsoft.com/l/meetup-join/"));
    }

    #[test]
    fn test_extract_webex_url() {
        let text = "https://company.webex.com/meet/john.doe";
        let url = extract_meeting_url(text).unwrap();
        assert!(url.contains("webex.com"));
    }

    #[test]
    fn test_no_meeting_url() {
        let text = "Just a regular meeting in room 42";
        let url = extract_meeting_url(text);
        assert!(url.is_none());
    }

    #[test]
    fn test_extract_from_event_location_first() {
        let url = extract_from_event(
            Some("https://meet.google.com/abc-def-ghi"),
            Some("Notes with https://zoom.us/j/123"),
        );
        assert_eq!(url.unwrap(), "https://meet.google.com/abc-def-ghi");
    }

    #[test]
    fn test_extract_from_event_falls_back_to_notes() {
        let url = extract_from_event(
            Some("Conference Room B"),
            Some("Join: https://zoom.us/j/123456"),
        );
        assert_eq!(url.unwrap(), "https://zoom.us/j/123456");
    }

    #[test]
    fn test_extract_from_event_none() {
        let url = extract_from_event(Some("Room 42"), Some("Bring laptop"));
        assert!(url.is_none());
    }

    #[test]
    fn test_extract_from_event_all_none() {
        let url = extract_from_event(None, None);
        assert!(url.is_none());
    }
}
