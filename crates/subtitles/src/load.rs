//! Loading an external subtitle file: bounded, untrusted, and encoding-normalising.
//!
//! An external subtitle is untrusted input, and two things follow. First, the read is
//! **bounded** — a pathological file cannot make us allocate without limit. Second, **text**
//! subtitles (SRT/ASS/SSA/WebVTT/MicroDVD) are transcoded to UTF-8 here, from a charset guess,
//! before libass ever sees them — so a legacy Windows-1251 or Shift-JIS file renders as its
//! real letters instead of mojibake.
//!
//! We do **not** re-implement the subtitle formats: rendering, ASS styling and image decoding
//! stay the engine's job (libass / libavcodec). Image-based subtitles (PGS `.sup`, VobSub
//! `.idx`/`.sub`) are binary bitmaps — those are only size-bounded and handed over by path,
//! never transcoded.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// The largest external **text** subtitle we will read. Real cue files are tens to a few
/// hundred kilobytes; this ceiling is orders of magnitude over that, so it only ever stops a
/// file that has no business being loaded as text.
pub const MAX_TEXT_SUBTITLE_BYTES: u64 = 16 * 1024 * 1024;

/// The largest external **image** subtitle (VobSub/PGS bitmap stream) we will hand to the
/// engine. These are legitimately much larger than text, but still bounded.
pub const MAX_IMAGE_SUBTITLE_BYTES: u64 = 256 * 1024 * 1024;

/// Why loading an external subtitle failed. Reported to the user verbatim.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubtitleError {
    /// The path does not exist.
    NotFound(String),
    /// The path is not a regular file.
    NotAFile(String),
    /// The file is larger than the bound for its kind.
    TooLarge {
        path: String,
        bytes: u64,
        limit: u64,
    },
    /// The filesystem refused the read/write.
    Io(String),
}

impl fmt::Display for SubtitleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(path) => write!(f, "no such subtitle file: {path}"),
            Self::NotAFile(path) => write!(f, "not a subtitle file: {path}"),
            Self::TooLarge { path, bytes, limit } => write!(
                f,
                "subtitle file is too large ({bytes} bytes, limit {limit}): {path}"
            ),
            Self::Io(reason) => write!(f, "{reason}"),
        }
    }
}

impl std::error::Error for SubtitleError {}

/// The result of loading an external subtitle: the path to hand the engine, plus what we had to
/// do to get there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedSubtitle {
    /// The path for the engine's `sub-add`. A freshly written UTF-8 temp file when a text
    /// subtitle had to be transcoded; the original path when it was already clean UTF-8, or
    /// image-based.
    pub path: PathBuf,
    /// The charset a text subtitle was decoded from, for an honest "loaded as Windows-1251"
    /// note. `None` when nothing was transcoded (already UTF-8, or image-based).
    pub source_encoding: Option<String>,
    /// Whether the track is image-based (PGS/VobSub) — no text, so no encoding and no style
    /// override applies.
    pub image_based: bool,
}

/// Whether a subtitle file is text (transcodable) or an image-based bitmap stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Image,
}

/// Classify by extension, resolving the ambiguous `.sub`: a VobSub `.sub` is a binary bitmap
/// paired with a sibling `.idx`; a lone `.sub` is MicroDVD text.
fn classify(path: &Path) -> Kind {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "sup" | "idx" => Kind::Image,
        "sub" if path.with_extension("idx").exists() => Kind::Image,
        _ => Kind::Text,
    }
}

/// The UTF-8 text of a decoded subtitle, and how it got there.
struct Decoded {
    text: String,
    /// `Some(label)` when a charset had to be transcoded (a temp file is needed); `None` when
    /// the bytes were already clean UTF-8 (the original file can be handed over as-is).
    label: Option<String>,
}

/// Decode subtitle bytes to UTF-8. A byte-order mark is authoritative; otherwise valid UTF-8 is
/// passed through untouched, and anything else is charset-detected and transcoded.
fn decode_text(bytes: &[u8]) -> Decoded {
    // A BOM names the encoding outright.
    if let Some(rest) = bytes.strip_prefix(b"\xEF\xBB\xBF") {
        // Already UTF-8, but the BOM must go — hence a temp file, hence a label.
        return Decoded {
            text: String::from_utf8_lossy(rest).into_owned(),
            label: Some("UTF-8".to_owned()),
        };
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFF\xFE") {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(rest);
        return Decoded {
            text: text.into_owned(),
            label: Some("UTF-16LE".to_owned()),
        };
    }
    if let Some(rest) = bytes.strip_prefix(b"\xFE\xFF") {
        let (text, _, _) = encoding_rs::UTF_16BE.decode(rest);
        return Decoded {
            text: text.into_owned(),
            label: Some("UTF-16BE".to_owned()),
        };
    }
    // No BOM: if it is already valid UTF-8, hand the original file over unchanged.
    if let Ok(text) = std::str::from_utf8(bytes) {
        return Decoded {
            text: text.to_owned(),
            label: None,
        };
    }
    // Otherwise guess the legacy charset and transcode it.
    let mut detector = chardetng::EncodingDetector::new();
    detector.feed(bytes, true);
    let encoding = detector.guess(None, true);
    let (text, _, _) = encoding.decode(bytes);
    Decoded {
        text: text.into_owned(),
        label: Some(encoding.name().to_owned()),
    }
}

/// Load an external subtitle file for the engine to render.
///
/// Text subtitles are read (bounded), charset-detected and, when not already clean UTF-8,
/// written to a UTF-8 temp file whose path is returned. Image-based subtitles are size-bounded
/// and handed over by their original path.
pub fn load(path: &Path) -> Result<LoadedSubtitle, SubtitleError> {
    let display = path.to_string_lossy().into_owned();
    let meta = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(SubtitleError::NotFound(display))
        }
        Err(e) => return Err(SubtitleError::Io(e.to_string())),
    };
    if !meta.is_file() {
        return Err(SubtitleError::NotAFile(display));
    }

    match classify(path) {
        Kind::Image => {
            if meta.len() > MAX_IMAGE_SUBTITLE_BYTES {
                return Err(SubtitleError::TooLarge {
                    path: display,
                    bytes: meta.len(),
                    limit: MAX_IMAGE_SUBTITLE_BYTES,
                });
            }
            Ok(LoadedSubtitle {
                path: path.to_path_buf(),
                source_encoding: None,
                image_based: true,
            })
        }
        Kind::Text => {
            if meta.len() > MAX_TEXT_SUBTITLE_BYTES {
                return Err(SubtitleError::TooLarge {
                    path: display,
                    bytes: meta.len(),
                    limit: MAX_TEXT_SUBTITLE_BYTES,
                });
            }
            let bytes = fs::read(path).map_err(|e| SubtitleError::Io(e.to_string()))?;
            let decoded = decode_text(&bytes);
            match decoded.label {
                // Already clean UTF-8 — no rewrite, hand the original over.
                None => Ok(LoadedSubtitle {
                    path: path.to_path_buf(),
                    source_encoding: None,
                    image_based: false,
                }),
                // Transcoded — write the UTF-8 form to a temp file for the engine to open,
                // preserving the source extension so the engine sniffs the right format.
                Some(label) => {
                    let ext = extension_or_srt(path);
                    let temp = write_temp(ext, decoded.text.as_bytes())
                        .map_err(|e| SubtitleError::Io(e.to_string()))?;
                    Ok(LoadedSubtitle {
                        path: temp,
                        source_encoding: Some(label),
                        image_based: false,
                    })
                }
            }
        }
    }
}

/// Load an external subtitle from an in-memory byte buffer (e.g. an online download) rather than
/// a file path. Text only: the bytes are size-bounded, charset-detected, and written to a UTF-8
/// temp file the engine can open. `ext` sets the temp file's extension so the engine sniffs the
/// right format. This is the reuse point that keeps the download path off a second on-disk round
/// trip and through the same bounded, transcoding logic as a local file.
pub fn load_bytes(bytes: &[u8], ext: &str) -> Result<LoadedSubtitle, SubtitleError> {
    if bytes.len() as u64 > MAX_TEXT_SUBTITLE_BYTES {
        return Err(SubtitleError::TooLarge {
            path: format!("<download>.{ext}"),
            bytes: bytes.len() as u64,
            limit: MAX_TEXT_SUBTITLE_BYTES,
        });
    }
    let decoded = decode_text(bytes);
    let temp =
        write_temp(ext, decoded.text.as_bytes()).map_err(|e| SubtitleError::Io(e.to_string()))?;
    Ok(LoadedSubtitle {
        path: temp,
        // `label` is None when the bytes were already clean UTF-8 — reported honestly as "not
        // transcoded" even though a temp file was still written (there is no original to reuse).
        source_encoding: decoded.label,
        image_based: false,
    })
}

/// The file's extension as a `&str`, or "srt" when it has none.
fn extension_or_srt(path: &Path) -> &str {
    path.extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .unwrap_or("srt")
}

/// Write subtitle bytes to a uniquely named temp file with the given extension. Returns the path.
fn write_temp(ext: &str, bytes: &[u8]) -> std::io::Result<PathBuf> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);

    let dir = std::env::temp_dir().join("freally-player-subs");
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("sub-{}-{n}.{ext}", std::process::id()));
    fs::write(&path, bytes)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    fn scratch_dir(name: &str) -> PathBuf {
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("freally-subload-{}-{name}-{n}", std::process::id()));
        fs::create_dir_all(&dir).expect("create scratch dir");
        dir
    }

    #[test]
    fn a_missing_file_is_refused_by_name() {
        let err = load(Path::new("Z:/nope/missing.srt")).expect_err("should fail");
        assert!(matches!(err, SubtitleError::NotFound(_)));
    }

    #[test]
    fn a_directory_is_not_a_subtitle() {
        let dir = scratch_dir("dir");
        assert!(matches!(load(&dir), Err(SubtitleError::NotAFile(_))));
    }

    #[test]
    fn a_utf8_srt_is_passed_through_untouched() {
        let dir = scratch_dir("utf8");
        let path = dir.join("clip.srt");
        let body = "1\n00:00:01,000 --> 00:00:02,000\nHello, world.\n";
        fs::write(&path, body).expect("write srt");

        let loaded = load(&path).expect("load");
        // Clean UTF-8: no transcode, the original file is handed over.
        assert_eq!(loaded.path, path);
        assert_eq!(loaded.source_encoding, None);
        assert!(!loaded.image_based);
    }

    #[test]
    fn a_shift_jis_subtitle_is_detected_and_transcoded_to_utf8() {
        let dir = scratch_dir("sjis");
        let path = dir.join("jp.srt");
        // A believable Japanese cue, repeated so the detector has signal to work with.
        let plain =
            "1\n00:00:01,000 --> 00:00:04,000\nこんにちは、これは字幕のテストです。\n\n".repeat(6);
        let (bytes, _, _) = encoding_rs::SHIFT_JIS.encode(&plain);
        assert!(!bytes.starts_with(b"\xEF\xBB\xBF"));
        fs::write(&path, &bytes).expect("write sjis");

        let loaded = load(&path).expect("load");
        // Transcoded: a temp file, and an honest record of the source charset.
        assert_ne!(loaded.path, path);
        assert!(loaded.source_encoding.is_some());
        let recovered = fs::read_to_string(&loaded.path).expect("read temp");
        assert!(
            recovered.contains("字幕のテスト"),
            "expected the Japanese to survive the round trip, got: {recovered}"
        );
    }

    #[test]
    fn a_utf16le_bom_file_is_transcoded() {
        let dir = scratch_dir("utf16");
        let path = dir.join("bom.srt");
        let plain = "1\n00:00:01,000 --> 00:00:02,000\nCafé déjà vu\n";
        // encoding_rs treats UTF-16 as decode-only (its `encode` would emit UTF-8), so build
        // real little-endian UTF-16 code units by hand and prepend the UTF-16LE BOM.
        let mut with_bom = vec![0xFF, 0xFE];
        for unit in plain.encode_utf16() {
            with_bom.extend_from_slice(&unit.to_le_bytes());
        }
        fs::write(&path, &with_bom).expect("write utf16");

        let loaded = load(&path).expect("load");
        assert_eq!(loaded.source_encoding.as_deref(), Some("UTF-16LE"));
        let recovered = fs::read_to_string(&loaded.path).expect("read temp");
        assert!(recovered.contains("Café déjà vu"));
    }

    #[test]
    fn an_oversized_text_file_is_refused() {
        let dir = scratch_dir("huge");
        let path = dir.join("huge.srt");
        // A sparse file past the text bound, without allocating it in memory.
        let f = fs::File::create(&path).expect("create");
        f.set_len(MAX_TEXT_SUBTITLE_BYTES + 1).expect("grow");
        drop(f);
        assert!(matches!(load(&path), Err(SubtitleError::TooLarge { .. })));
    }

    #[test]
    fn extension_classifies_image_versus_text() {
        assert_eq!(classify(Path::new("movie.sup")), Kind::Image);
        assert_eq!(classify(Path::new("movie.idx")), Kind::Image);
        assert_eq!(classify(Path::new("movie.srt")), Kind::Text);
        assert_eq!(classify(Path::new("movie.ass")), Kind::Text);
        // A lone `.sub` with no `.idx` beside it is MicroDVD text.
        assert_eq!(classify(Path::new("movie.sub")), Kind::Text);
    }

    #[test]
    fn a_vobsub_sub_beside_its_idx_is_image_based() {
        let dir = scratch_dir("vobsub");
        let sub = dir.join("movie.sub");
        fs::write(dir.join("movie.idx"), b"# VobSub index").expect("write idx");
        fs::write(&sub, b"\x00\x01\x02binary bitmap data").expect("write sub");
        let loaded = load(&sub).expect("load");
        assert!(loaded.image_based);
        assert_eq!(loaded.path, sub);
        assert_eq!(loaded.source_encoding, None);
    }
}
