//! Opt-in OpenSubtitles fetch, over TLS.
//!
//! This is **off unless the user turns it on** and supplies their own OpenSubtitles API key
//! (a free account). Nothing here runs otherwise. When it does run, only the minimal search
//! identifiers the user typed (a title/query and language codes) leave the machine — no file
//! contents, no history. Downloading a candidate needs the account's own login, and the
//! password is never persisted: it is exchanged for a short-lived token held only in memory.
//!
//! The request-building and response-parsing are separated from the HTTP so they can be tested
//! without the network; the live fetch needs a real key and account, which the manual test
//! drill in `Live-To-Do-List.md` covers.

use std::io::Read;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// The public v1 REST endpoint.
const API_BASE: &str = "https://api.opensubtitles.com/api/v1";

/// How long a search/login/download call may take before we give up.
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// The largest subtitle body we will pull down — the same bound the on-disk text loader uses.
const MAX_DOWNLOAD_BYTES: u64 = crate::load::MAX_TEXT_SUBTITLE_BYTES;

/// One search result the user can choose to download.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// The id passed back to `download_link` to get this file.
    pub file_id: i64,
    pub file_name: String,
    /// The subtitle's language code, when the listing gives one.
    pub language: Option<String>,
    /// The release/version string (e.g. the source rip), when given.
    pub release: Option<String>,
    /// How many times it has been downloaded — a rough quality signal for ranking.
    pub download_count: i64,
}

/// Why an OpenSubtitles call failed. Surfaced to the user verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// The network request itself failed (offline, TLS, DNS, timeout).
    Http(String),
    /// The API answered with a non-success status.
    Api { status: u16, message: String },
    /// The response was not the shape we expected.
    Parse(String),
    /// The downloaded body exceeded the size bound.
    TooLarge,
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Http(reason) => write!(f, "could not reach OpenSubtitles: {reason}"),
            Self::Api { status, message } => {
                write!(f, "OpenSubtitles refused the request ({status}): {message}")
            }
            Self::Parse(reason) => write!(f, "unexpected reply from OpenSubtitles: {reason}"),
            Self::TooLarge => write!(f, "the subtitle from OpenSubtitles was too large to load"),
        }
    }
}

impl std::error::Error for FetchError {}

/// A configured client. Holds the caller's API key and a User-Agent identifying the app, as the
/// API requires.
pub struct OpenSubtitlesClient {
    api_key: String,
    user_agent: String,
    http: reqwest::blocking::Client,
}

impl OpenSubtitlesClient {
    /// Build a client for `api_key`, identifying as `user_agent` (e.g. "Freally Player v0.30.0").
    pub fn new(
        api_key: impl Into<String>,
        user_agent: impl Into<String>,
    ) -> Result<Self, FetchError> {
        let http = reqwest::blocking::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| FetchError::Http(e.to_string()))?;
        Ok(Self {
            api_key: api_key.into(),
            user_agent: user_agent.into(),
            http,
        })
    }

    /// Search for subtitles by a free-text query and a set of language codes.
    pub fn search(&self, query: &str, languages: &[String]) -> Result<Vec<Candidate>, FetchError> {
        let mut params: Vec<(String, String)> = vec![("query".to_owned(), query.to_owned())];
        if !languages.is_empty() {
            params.push(("languages".to_owned(), languages.join(",")));
        }
        let resp = self
            .http
            .get(format!("{API_BASE}/subtitles"))
            .header("Api-Key", &self.api_key)
            .header("User-Agent", &self.user_agent)
            .header("Accept", "application/json")
            .query(&params)
            .send()
            .map_err(|e| FetchError::Http(e.to_string()))?;
        let body = read_json(resp)?;
        parse_search_response(&body)
    }

    /// Exchange an account's credentials for a short-lived token. The password is used here and
    /// then dropped by the caller — it is never stored.
    pub fn login(&self, username: &str, password: &str) -> Result<String, FetchError> {
        let resp = self
            .http
            .post(format!("{API_BASE}/login"))
            .header("Api-Key", &self.api_key)
            .header("User-Agent", &self.user_agent)
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "username": username, "password": password }))
            .send()
            .map_err(|e| FetchError::Http(e.to_string()))?;
        let body = read_json(resp)?;
        parse_login_response(&body)
    }

    /// Ask for a one-time download link for a chosen `file_id`, authorised by a login `token`.
    pub fn download_link(&self, token: &str, file_id: i64) -> Result<String, FetchError> {
        let resp = self
            .http
            .post(format!("{API_BASE}/download"))
            .header("Api-Key", &self.api_key)
            .header("User-Agent", &self.user_agent)
            .header("Authorization", format!("Bearer {token}"))
            .header("Accept", "application/json")
            .json(&serde_json::json!({ "file_id": file_id }))
            .send()
            .map_err(|e| FetchError::Http(e.to_string()))?;
        let body = read_json(resp)?;
        parse_download_response(&body)
    }

    /// Pull the actual subtitle bytes from a download link, bounded so a runaway response cannot
    /// exhaust memory.
    pub fn fetch_bytes(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        let resp = self
            .http
            .get(url)
            .header("User-Agent", &self.user_agent)
            .send()
            .map_err(|e| FetchError::Http(e.to_string()))?;
        if !resp.status().is_success() {
            return Err(FetchError::Api {
                status: resp.status().as_u16(),
                message: "download link rejected".to_owned(),
            });
        }
        // Read at most the bound plus one byte: if we get that extra byte, it was over the limit.
        let mut buf = Vec::new();
        resp.take(MAX_DOWNLOAD_BYTES + 1)
            .read_to_end(&mut buf)
            .map_err(|e| FetchError::Http(e.to_string()))?;
        if buf.len() as u64 > MAX_DOWNLOAD_BYTES {
            return Err(FetchError::TooLarge);
        }
        Ok(buf)
    }
}

/// Read a response body as text, turning a non-success status into an [`FetchError::Api`] that
/// carries whatever message the API included.
fn read_json(resp: reqwest::blocking::Response) -> Result<String, FetchError> {
    let status = resp.status();
    let text = resp.text().map_err(|e| FetchError::Http(e.to_string()))?;
    if !status.is_success() {
        return Err(FetchError::Api {
            status: status.as_u16(),
            message: api_error_message(&text).unwrap_or_else(|| text.clone()),
        });
    }
    Ok(text)
}

/// Pull a human message out of an API error body, when it has the usual `{ "message": … }`.
fn api_error_message(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct ApiError {
        message: Option<String>,
    }
    serde_json::from_str::<ApiError>(body)
        .ok()
        .and_then(|e| e.message)
        .filter(|m| !m.is_empty())
}

// --- Response shapes (only the fields we use; serde ignores the rest) ---------------------

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    data: Vec<SearchItem>,
}

#[derive(Deserialize)]
struct SearchItem {
    #[serde(default)]
    attributes: Option<SearchAttributes>,
}

#[derive(Deserialize)]
struct SearchAttributes {
    language: Option<String>,
    release: Option<String>,
    #[serde(default)]
    download_count: i64,
    #[serde(default)]
    files: Vec<SearchFile>,
}

#[derive(Deserialize)]
struct SearchFile {
    file_id: i64,
    file_name: Option<String>,
}

/// Parse a `/subtitles` search response into ranked candidates (most-downloaded first).
fn parse_search_response(body: &str) -> Result<Vec<Candidate>, FetchError> {
    let parsed: SearchResponse =
        serde_json::from_str(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    let mut candidates: Vec<Candidate> = parsed
        .data
        .into_iter()
        .filter_map(|item| {
            let attrs = item.attributes?;
            // The download id lives on the first file entry; a listing with no file is not
            // downloadable, so drop it.
            let file = attrs.files.into_iter().next()?;
            Some(Candidate {
                file_id: file.file_id,
                file_name: file.file_name.unwrap_or_else(|| "subtitle".to_owned()),
                language: attrs.language,
                release: attrs.release,
                download_count: attrs.download_count,
            })
        })
        .collect();
    candidates.sort_by_key(|c| std::cmp::Reverse(c.download_count));
    Ok(candidates)
}

#[derive(Deserialize)]
struct LoginResponse {
    token: Option<String>,
}

/// Pull the session token out of a `/login` response.
fn parse_login_response(body: &str) -> Result<String, FetchError> {
    let parsed: LoginResponse =
        serde_json::from_str(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    parsed
        .token
        .filter(|t| !t.is_empty())
        .ok_or_else(|| FetchError::Parse("login succeeded but returned no token".to_owned()))
}

#[derive(Deserialize)]
struct DownloadResponse {
    link: Option<String>,
}

/// Pull the one-time download link out of a `/download` response.
fn parse_download_response(body: &str) -> Result<String, FetchError> {
    let parsed: DownloadResponse =
        serde_json::from_str(body).map_err(|e| FetchError::Parse(e.to_string()))?;
    parsed
        .link
        .filter(|l| !l.is_empty())
        .ok_or_else(|| FetchError::Parse("no download link in the reply".to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_search_response_parses_and_ranks_by_downloads() {
        let body = r#"{
          "total_count": 2,
          "data": [
            {
              "id": "1",
              "type": "subtitle",
              "attributes": {
                "language": "en",
                "release": "Movie.2020.1080p.BluRay",
                "download_count": 120,
                "files": [ { "file_id": 55, "file_name": "movie.en.srt" } ]
              }
            },
            {
              "id": "2",
              "type": "subtitle",
              "attributes": {
                "language": "en",
                "release": "Movie.2020.WEBRip",
                "download_count": 900,
                "files": [ { "file_id": 66, "file_name": "movie.web.srt" } ]
              }
            }
          ]
        }"#;
        let candidates = parse_search_response(body).expect("parse");
        assert_eq!(candidates.len(), 2);
        // Most-downloaded first.
        assert_eq!(candidates[0].file_id, 66);
        assert_eq!(candidates[0].download_count, 900);
        assert_eq!(candidates[1].file_id, 55);
        assert_eq!(candidates[0].language.as_deref(), Some("en"));
    }

    #[test]
    fn a_listing_with_no_downloadable_file_is_dropped() {
        let body = r#"{ "data": [
          { "attributes": { "language": "de", "download_count": 5, "files": [] } }
        ] }"#;
        assert!(parse_search_response(body).expect("parse").is_empty());
    }

    #[test]
    fn an_empty_search_is_not_an_error() {
        assert!(parse_search_response(r#"{ "data": [] }"#)
            .expect("parse")
            .is_empty());
    }

    #[test]
    fn a_malformed_search_body_is_a_parse_error() {
        assert!(matches!(
            parse_search_response("not json"),
            Err(FetchError::Parse(_))
        ));
    }

    #[test]
    fn a_login_token_is_extracted() {
        let body = r#"{ "token": "abc123", "status": 200 }"#;
        assert_eq!(parse_login_response(body).expect("token"), "abc123");
    }

    #[test]
    fn a_login_without_a_token_is_a_parse_error() {
        assert!(matches!(
            parse_login_response(r#"{ "status": 401 }"#),
            Err(FetchError::Parse(_))
        ));
    }

    #[test]
    fn a_download_link_is_extracted() {
        let body = r#"{ "link": "https://dl.opensubtitles.com/x.srt", "requests": 3 }"#;
        assert_eq!(
            parse_download_response(body).expect("link"),
            "https://dl.opensubtitles.com/x.srt"
        );
    }

    #[test]
    fn an_api_error_message_is_pulled_out() {
        assert_eq!(
            api_error_message(r#"{ "message": "invalid api key" }"#).as_deref(),
            Some("invalid api key")
        );
        assert_eq!(api_error_message("plain text, not json"), None);
    }
}
