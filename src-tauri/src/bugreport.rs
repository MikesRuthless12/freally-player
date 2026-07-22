//! Opt-in, anonymous bug reporting — **charter-clean: no telemetry, nothing auto-sends, no
//! server we run, no credentials shipped.**
//!
//! Ported from the Havoc standard (`HAVOC-STANDARD-bug-report-and-updater.md`, reference
//! implementation Freally Capture). Every design note below is a bug that was actually hit
//! there; none of it is style preference.
//!
//! A panic hook captures a **scrubbed** crash report to a local file; on the next launch the
//! UI offers to report it. The "Report a bug" dialog shows the user the **exact** anonymous
//! report and lets them submit it via a pre-filled **GitHub issue**, **Gmail compose**, or
//! their **mail client** — all explicit clicks. Diagnostics carry the app version + OS/arch
//! and (optionally) a crash excerpt with the home path + username redacted; never file
//! contents, media paths, or personal data.
//!
//! One deliberate exception: a crash excerpt is stamped with **when** it happened, in both
//! the machine's local time (with its UTC offset) and UTC. The offset narrows the reporter to
//! a timezone, which is weakly identifying — it is included because a crash reported days
//! later is otherwise impossible to order or correlate, and because the user reads the exact
//! text before it is sent anywhere. Nothing finer-grained (locale, hostname, IP) is collected.
//!
//! # The crash loop
//!
//! A dying app cannot show its own error window — the webview goes away with it. So the panic
//! hook spawns **this same executable** as a tiny Tauri-free helper (`--crash-notice <pid>`),
//! then lets the process die. The helper shows a native "stopped unexpectedly" message box,
//! waits for the crashed process to actually leave the process table, and relaunches the
//! player. The relaunched app finds the crash file and auto-opens the report dialog.
//!
//! The wait is load-bearing: a crashed process holds its OS resources (and, once Freally
//! Player grows a single-instance guard in Phase 1, the instance lock) until it is reaped.
//! Relaunching too early forwards the new launch into the dying app, which then exits —
//! leaving the user with no app at all.
//!
//! The notice fires **only when a panic is guaranteed to be fatal** — release builds set
//! `panic = "abort"`. Under `unwind` (debug) a worker-thread panic leaves the app running,
//! and a "restart?" box would be a lie; there the hook keeps its old behaviour of writing the
//! file and nothing else.
//!
//! To drill the loop on demand, launch with `--test-crash` (see [`arm_test_crash`]): it exits
//! explicitly, so it behaves the same in both profiles. There is deliberately **no button and
//! no IPC command** for this — a "crash the app" control has no business shipping.
//!
//! The message box is native on every OS: `MessageBoxW` on Windows, `NSAlert` on macOS, GTK3
//! on Linux. If the dialog cannot open at all, `rfd` reports `Cancel` — indistinguishable
//! from the user declining — so the app simply stays closed. The crash report is on disk
//! either way and the next launch surfaces it, making the worst case exactly the old
//! manual-relaunch behaviour.

use std::path::PathBuf;

use serde::Serialize;

/// This app's name — put in the subject line + body so a report that lands in the shared
/// inbox is instantly attributable to the right Havoc app.
const APP_NAME: &str = "Freally Player";
/// The project's issue tracker (a pre-filled URL the user submits — no token).
const GITHUB_NEW_ISSUE: &str = "https://github.com/MikesRuthless12/freally-player/issues/new";
/// Where an emailed report goes (the user's own mail client sends it).
const REPORT_EMAIL: &str = "mythodikalone@gmail.com";
/// Gmail's web compose window. Plain https — no API key, no token, and nothing is sent until
/// the user clicks Send. A signed-out user lands on Google's login screen and is returned to
/// the pre-filled draft afterwards. Offered *alongside* `mailto:`, which stays the path for
/// anyone not using Gmail.
const GMAIL_COMPOSE: &str = "https://mail.google.com/mail/?view=cm&fs=1";
/// Bounds on the **percent-encoded** body. A character cap cannot bound a URL: one 3-byte
/// character (`—`, `“`) encodes to nine. Browsers take ~32 k, so the https targets are
/// generous; `mailto:` rides Windows' ShellExecute, which in practice truncates near 2048
/// characters and then opens nothing at all — a blank window, no error. "Copy report" always
/// carries the untruncated text.
const MAX_GITHUB_ENCODED: usize = 6000;
const MAX_GMAIL_ENCODED: usize = 6000;
/// The whole `mailto:` URL — scheme, address, subject and body together — stays under this.
/// ShellExecute practically dies near 2048; leave a margin.
const MAX_MAILTO_URL: usize = 1900;
/// …of which the subject may claim at most this much, so a pathological subject can never
/// starve the body of every byte.
const MAX_MAILTO_SUBJECT_ENCODED: usize = 300;
/// Argv flag that turns this executable into the post-crash notice helper:
/// `freally-player --crash-notice <pid-of-the-process-that-died>`.
const CRASH_NOTICE_FLAG: &str = "--crash-notice";
/// Argv flag that crashes the app on purpose a few seconds after launch, to drill the crash
/// loop. Deliberately not a button and not an IPC command.
const TEST_CRASH_FLAG: &str = "--test-crash";
/// How long the helper will wait for the crashed process to leave the process table before
/// relaunching anyway.
const EXIT_WAIT: std::time::Duration = std::time::Duration::from_millis(250);
const EXIT_WAIT_TRIES: u32 = 40; // ≤ 10 s, then relaunch anyway

fn crash_dir() -> Option<PathBuf> {
    crate::paths::data_dir().map(|dir| dir.join("crash-reports"))
}

/// Redact the OS user's home path + username from `text` so a report carries no personal
/// identifiers. The report is always shown to the user before it can be sent, so
/// over-redaction is safe and under-redaction is visible.
pub fn scrub(text: &str) -> String {
    let mut out = text.to_string();
    if let Some(dirs) = directories::UserDirs::new() {
        let home = dirs.home_dir().to_string_lossy().to_string();
        if !home.is_empty() {
            out = out.replace(&home, "<home>");
            // Also the bare username, if it's not a trivially-short substring.
            if let Some(name) = std::path::Path::new(&home)
                .file_name()
                .and_then(|n| n.to_str())
            {
                if name.len() >= 3 {
                    out = out.replace(name, "<user>");
                }
            }
        }
    }
    out
}

/// The always-anonymous system line (no personal data).
pub fn diagnostics() -> String {
    format!(
        "App: {APP_NAME} {}\nOS: {} / {}\n",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH,
    )
}

/// Install the panic hook (call once at startup): a panic writes a scrubbed crash report to
/// the local crash-reports dir, then the previous hook runs.
pub fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let location = info
            .location()
            .map(|loc| format!("{}:{}", loc.file(), loc.line()))
            .unwrap_or_else(|| "unknown".to_string());
        let message = info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "(no message)".to_string());
        let backtrace = std::backtrace::Backtrace::force_capture();
        let raw = format!("Panic at {location}\nMessage: {message}\n\nBacktrace:\n{backtrace}\n");
        write_crash(&scrub(&raw));
        // Only when this panic is certain to kill the process. Release builds set
        // `panic = "abort"`; under unwind, a worker thread can panic while the app keeps
        // running, and an error box offering to restart would be wrong. `--test-crash` exits
        // by hand, so the drill still works in debug.
        if cfg!(panic = "abort") {
            spawn_crash_notice();
        }
        previous(info);
    }));
}

/// Spawn this same executable as the `--crash-notice` helper and hand it our pid. It outlives
/// us: it waits for this process to disappear, then relaunches the player. Best-effort — a
/// failure here just means no error window, and the crash report is still on disk for the
/// next manual launch.
///
/// At most once per process. `panic = "abort"` does not stop the world instantly, so two
/// threads can panic close enough together to run the hook twice — which would put two
/// "stopped unexpectedly" dialogs on screen for one crash.
fn spawn_crash_notice() {
    static SPAWNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if SPAWNED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return;
    }
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let _ = std::process::Command::new(exe)
        .arg(CRASH_NOTICE_FLAG)
        .arg(std::process::id().to_string())
        .spawn();
}

/// Is `pid` still in the process table?
///
/// Identity is the raw pid, so a pid the OS has already recycled onto an unrelated process
/// reads as "still alive". The cost is bounded and benign: at worst [`wait_for_exit`] burns
/// its full timeout before relaunching anyway.
fn process_alive(pid: u32) -> bool {
    use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};
    let pid = Pid::from_u32(pid);
    let mut system = System::new();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing(),
    );
    system.process(pid).is_some()
}

/// Block until the crashed process is gone (bounded — we relaunch regardless rather than
/// strand the user with no player).
fn wait_for_exit(pid: u32) {
    for _ in 0..EXIT_WAIT_TRIES {
        if !process_alive(pid) {
            return;
        }
        std::thread::sleep(EXIT_WAIT);
    }
}

/// With no pid to watch there is nothing to poll, so wait a fixed interval instead of
/// relaunching immediately. A missing or unparseable pid must not mean "skip the wait" —
/// that is the very race this helper exists to lose gracefully.
fn wait_blind() {
    std::thread::sleep(EXIT_WAIT * 8);
}

/// If argv says we are the `--crash-notice <pid>` helper, run that whole flow and return
/// `true` so `run` exits without ever building a Tauri app.
pub fn run_crash_notice(args: &[String]) -> bool {
    let Some(flag_at) = args.iter().position(|arg| arg == CRASH_NOTICE_FLAG) else {
        return false;
    };
    let dead_pid = args
        .get(flag_at + 1)
        .and_then(|arg| arg.parse::<u32>().ok());

    let answer = rfd::MessageDialog::new()
        .set_level(rfd::MessageLevel::Error)
        .set_title("Freally Player stopped unexpectedly")
        .set_description(
            "Freally Player hit an unexpected error and had to close.\n\n\
             A crash report was saved on this machine. Nothing has been sent \
             anywhere. If you restart, you can read the exact report and choose \
             to send it as a GitHub issue or by email.\n\n\
             Restart Freally Player now?",
        )
        .set_buttons(rfd::MessageButtons::YesNo)
        .show();

    if answer != rfd::MessageDialogResult::Yes {
        return true;
    }

    match dead_pid {
        Some(pid) => wait_for_exit(pid),
        None => wait_blind(),
    }
    if let Ok(exe) = std::env::current_exe() {
        let _ = std::process::Command::new(exe).spawn();
    }
    true
}

/// When the crash happened, written into the report itself — the file's own mtime does not
/// survive being pasted into an issue or an email, and a crash is often reported days later.
///
/// Both clocks are given: the user's wall clock (so *they* recognise the moment) and UTC (so
/// the maintainer can order reports from anywhere without doing timezone arithmetic). The
/// `%z` offset is the one piece of weakly identifying data in the report — see [`scrub`].
fn crash_time_line() -> String {
    let now = chrono::Local::now();
    format!(
        "Crashed: {} (UTC {})",
        now.format("%Y-%m-%d %H:%M:%S %z"),
        now.with_timezone(&chrono::Utc).format("%Y-%m-%d %H:%M:%S"),
    )
}

fn write_crash(scrubbed: &str) {
    let Some(dir) = crash_dir() else { return };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let stamped = format!("{}\n{scrubbed}", crash_time_line());
    let _ = std::fs::write(dir.join(format!("crash-{ts}.txt")), stamped);
}

/// The newest pending crash report (already scrubbed), if any.
pub fn pending_crash() -> Option<String> {
    let dir = crash_dir()?;
    let mut newest: Option<(u128, PathBuf)> = None;
    for entry in std::fs::read_dir(&dir).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let mtime = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis())
            .unwrap_or(0);
        if newest.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
            newest = Some((mtime, path));
        }
    }
    let (_, path) = newest?;
    std::fs::read_to_string(path).ok()
}

/// Delete the pending crash reports (the user dismissed or sent them).
pub fn clear_crashes() {
    if let Some(dir) = crash_dir() {
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// Percent-encode a query component (RFC 3986 unreserved kept verbatim).
fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for byte in s.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn truncate_chars(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push_str("\n… (truncated — use “Copy report” for the full text)");
    out
}

/// Percent-encode `s`, stopping before the **encoded** form outgrows `max_encoded`.
/// Truncating by character count cannot bound a URL, because a single 3-byte character
/// expands to nine encoded ones. Always cuts on a character boundary, never mid-escape.
fn encode_bounded(s: &str, max_encoded: usize) -> String {
    encode_bounded_with(
        s,
        max_encoded,
        "\n… (truncated — use “Copy report” for the full text)",
    )
}

/// [`encode_bounded`] without the truncation note — for fields where a trailing sentence
/// would be absurd, like a subject line.
fn encode_capped(s: &str, max_encoded: usize) -> String {
    encode_bounded_with(s, max_encoded, "")
}

/// Percent-encode `s` so the result never exceeds `max_encoded` bytes, appending `note`
/// (itself encoded, and reserved out of the budget) when anything was cut. Whole encoded
/// characters only — never half of a `%E2%80%94`.
fn encode_bounded_with(s: &str, max_encoded: usize, note: &str) -> String {
    let full = urlencode(s);
    if full.len() <= max_encoded {
        return full;
    }
    let note = urlencode(note);
    let budget = max_encoded.saturating_sub(note.len());
    let mut out = String::with_capacity(max_encoded);
    let mut buf = [0u8; 4];
    for ch in s.chars() {
        let piece = urlencode(ch.encode_utf8(&mut buf));
        if out.len() + piece.len() > budget {
            break;
        }
        out.push_str(&piece);
    }
    out.push_str(&note);
    out
}

/// A pre-filled GitHub "new issue" URL (the user submits it while signed in — no token, no
/// server).
pub fn github_url(title: &str, body: &str) -> String {
    format!(
        "{GITHUB_NEW_ISSUE}?labels=bug&title={}&body={}",
        urlencode(&truncate_chars(title, 200)),
        encode_bounded(body, MAX_GITHUB_ENCODED),
    )
}

/// A pre-filled `mailto:` URL (the user's own mail client sends it).
///
/// The bound is on the **whole URL**, not on the body alone. Bounding only the body left the
/// scheme, the address and the subject uncounted — and a subject is user text: it is capped
/// at 80 characters, but 80 CJK characters encode to 720 bytes and 80 emoji to 960. A
/// non-English report could therefore still cross the ~2048 mark where Windows' ShellExecute
/// opens a blank window and reports no error, which is the exact failure this bound prevents.
pub fn mailto_url(subject: &str, body: &str) -> String {
    let head = format!(
        "mailto:{REPORT_EMAIL}?subject={}&body=",
        encode_capped(&truncate_chars(subject, 200), MAX_MAILTO_SUBJECT_ENCODED),
    );
    let budget = MAX_MAILTO_URL.saturating_sub(head.len());
    format!("{head}{}", encode_bounded(body, budget))
}

/// A pre-filled Gmail web-compose URL. Unlike `mailto:` this never depends on a registered
/// mail handler — the browser opens Google's composer with the recipient, subject and body
/// filled in, and Google's login screen first if the user is signed out. Nothing sends
/// without the user's click.
pub fn gmail_url(subject: &str, body: &str) -> String {
    format!(
        "{GMAIL_COMPOSE}&to={}&su={}&body={}",
        urlencode(REPORT_EMAIL),
        urlencode(&truncate_chars(subject, 200)),
        encode_bounded(body, MAX_GMAIL_ENCODED),
    )
}

/// Open an https/mailto URL with the OS default handler. The URL is one we built (validated
/// scheme, no control chars) and passed as a single argv entry — no shell.
fn open_url(url: &str) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("mailto:")) {
        return Err("refusing to open a non-https/mailto URL".into());
    }
    if url.chars().any(char::is_control) {
        return Err("invalid URL".into());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn()
            .map_err(|err| format!("could not open the link: {err}"))?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg(url)
            .spawn()
            .map_err(|err| format!("could not open the link: {err}"))?;
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open")
            .arg(url)
            .spawn()
            .map_err(|err| format!("could not open the link: {err}"))?;
    }
    Ok(())
}

/// Open a vetted external link in the OS default browser. This Tauri webview never follows an
/// `<a target="_blank">` to the system browser, so UI link items hand the URL here instead.
/// Reuses `open_url`'s https/mailto allowlist — a `file:`/`javascript:` URL is refused.
#[tauri::command]
pub fn open_external(url: String) -> Result<(), String> {
    open_url(&url)
}

/// A short one-line summary of the error for the subject: the crash's panic message if there
/// was one, else the first line of the user's description, else a generic label.
fn error_summary(crash: Option<&str>, description: &str) -> String {
    let from_crash = crash.and_then(|c| {
        c.lines()
            .find_map(|line| line.strip_prefix("Message: "))
            .map(str::to_string)
    });
    let raw = from_crash
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            description
                .lines()
                .map(str::trim)
                .find(|line| !line.is_empty())
                .map(str::to_string)
        })
        .unwrap_or_else(|| {
            if crash.is_some() {
                "crash report".to_string()
            } else {
                "bug report".to_string()
            }
        });
    // One line, bounded — the rest lives in the body.
    let one_line: String = raw.split_whitespace().collect::<Vec<_>>().join(" ");
    if one_line.chars().count() > 80 {
        format!("{}…", one_line.chars().take(80).collect::<String>())
    } else {
        one_line
    }
}

/// The subject line: `[<App>] <error summary>` — which app + what went wrong.
fn subject(crash: Option<&str>, description: &str) -> String {
    format!("[{APP_NAME}] {}", error_summary(crash, description))
}

/// How the report body is rendered. The content is identical either way — only the syntax
/// around it changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BodyStyle {
    /// GitHub renders it: `###` headings and a fenced diagnostics block.
    Markdown,
    /// Mail clients do not — they show `###` and ``` as literal noise.
    Plain,
}

/// Build the full report body from the user's note + diagnostics (+ crash).
fn compose_body(description: &str, crash: Option<&str>, style: BodyStyle) -> String {
    let markdown = style == BodyStyle::Markdown;
    let mut body = String::new();

    body.push_str(if markdown {
        "### What happened\n"
    } else {
        "WHAT HAPPENED\n"
    });
    body.push_str(if description.trim().is_empty() {
        "(no description provided)"
    } else {
        description.trim()
    });

    body.push_str(if markdown {
        "\n\n### Anonymous diagnostics (no personal data)\n```\n"
    } else {
        "\n\nANONYMOUS DIAGNOSTICS (no personal data)\n"
    });
    body.push_str(&format!("From: {APP_NAME}\n"));
    body.push_str(&diagnostics());
    if let Some(crash) = crash {
        body.push_str("\n--- crash excerpt ---\n");
        body.push_str(crash);
    }
    body.push_str(if markdown { "\n```\n" } else { "\n" });

    // Belt-and-suspenders: scrub the whole assembled body once more.
    scrub(&body)
}

// --- Tauri commands --------------------------------------------------------

/// What the "Report a bug" dialog shows: the anonymous system info + any pending crash from
/// the last run (already scrubbed). Nothing is sent here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BugReportContextDto {
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub diagnostics: String,
    /// The scrubbed crash text from the previous run, if the app crashed.
    pub pending_crash: Option<String>,
}

#[tauri::command]
pub fn bug_report_context() -> BugReportContextDto {
    BugReportContextDto {
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        diagnostics: diagnostics(),
        pending_crash: pending_crash(),
    }
}

/// Submit a report: build the anonymous body from the user's note (+ the crash excerpt if
/// `include_crash`) and open it via `target` = `"github"` | `"gmail"` | `"email"`. This only
/// opens a pre-filled page/mail draft — the user still clicks send. Nothing leaves the
/// machine automatically.
#[tauri::command]
pub fn bug_report_submit(
    target: String,
    description: String,
    include_crash: bool,
) -> Result<(), String> {
    let crash = if include_crash { pending_crash() } else { None };
    let subject = subject(crash.as_deref(), &description);
    let crash = crash.as_deref();
    // GitHub renders Markdown; a mail client shows it as literal `###` noise.
    let url = match target.as_str() {
        "github" => github_url(
            &subject,
            &compose_body(&description, crash, BodyStyle::Markdown),
        ),
        "gmail" => gmail_url(
            &subject,
            &compose_body(&description, crash, BodyStyle::Plain),
        ),
        "email" => mailto_url(
            &subject,
            &compose_body(&description, crash, BodyStyle::Plain),
        ),
        other => return Err(format!("unknown report target: {other}")),
    };
    open_url(&url)
}

/// Dismiss + delete the pending crash report(s).
#[tauri::command]
pub fn bug_report_clear_crash() {
    clear_crashes();
}

/// `--test-crash`: let the app start normally, then kill it a few seconds in, so the whole
/// crash → error window → restart → report loop can be drilled the way a user would actually
/// meet it. The relaunch carries no arguments, so it comes back clean rather than crashing
/// again.
///
/// This is a **drill hook, not a feature**: no button, no IPC command, nothing a user can
/// reach by clicking. `exit(101)` (a panic-like code) rather than a real `panic!` makes it
/// behave identically under debug's `unwind` and release's `abort`, so the shipped exe drills
/// exactly as it ships.
pub fn arm_test_crash(args: &[String]) {
    if !args.iter().any(|arg| arg == TEST_CRASH_FLAG) {
        return;
    }
    eprintln!("{TEST_CRASH_FLAG}: this process will crash on purpose in 5 seconds");
    std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_secs(5));
        write_crash(&scrub(
            "Panic at src/testcrash.rs:1\nMessage: TEST CRASH — triggered by --test-crash; no \
             real fault occurred.\n\nBacktrace:\n(test)\n",
        ));
        spawn_crash_notice();
        std::process::exit(101);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrub_redacts_home_and_username() {
        if let Some(dirs) = directories::UserDirs::new() {
            let home = dirs.home_dir().to_string_lossy().to_string();
            if !home.is_empty() {
                let text = format!("error opening {home}/Videos/secret.mkv");
                let scrubbed = scrub(&text);
                assert!(!scrubbed.contains(&home), "home path must be redacted");
            }
        }
    }

    #[test]
    fn urlencode_escapes_unsafe_and_keeps_unreserved() {
        assert_eq!(urlencode("a b&c"), "a%20b%26c");
        assert_eq!(urlencode("Aa0-_.~"), "Aa0-_.~");
        assert_eq!(urlencode("líne\n"), "l%C3%ADne%0A");
    }

    #[test]
    fn github_and_mailto_urls_are_wellformed_and_bounded() {
        let long = "x".repeat(20_000);
        let gh = github_url("Bug report", &long);
        assert!(gh.starts_with("https://github.com/MikesRuthless12/freally-player/issues/new"));
        assert!(gh.contains("labels=bug"));
        assert!(gh.len() < 10_000, "github url must be bounded");

        let mail = mailto_url("Bug report", &long);
        assert!(mail.starts_with("mailto:mythodikalone@gmail.com?"));
        assert!(mail.len() < 4_000, "mailto url must be bounded");
    }

    /// The bound must hold on the *encoded* length. Multi-byte characters expand 9x, so a
    /// character-count cap let a real crash excerpt push the `mailto:` URL past ~2048 —
    /// where Windows silently opens a blank window.
    #[test]
    fn multibyte_bodies_cannot_blow_past_the_mailto_url_limit() {
        let body = "—".repeat(2_000);
        assert_eq!(body.chars().count(), 2_000);
        assert!(
            urlencode(&body).len() > 17_000,
            "premise: encoding inflates 9x"
        );

        let mail = mailto_url("Bug report", &body);
        assert!(
            mail.len() < 2_048,
            "mailto url must stay under the ShellExecute limit, was {}",
            mail.len()
        );
    }

    /// A CJK/emoji subject is user text and encodes 9–12x; the subject cap must be charged
    /// against the whole-URL budget, not counted in characters.
    #[test]
    fn a_multibyte_subject_cannot_starve_the_body_or_blow_the_limit() {
        let subject = "🎬".repeat(80);
        let body = "a".repeat(5_000);
        let mail = mailto_url(&subject, &body);

        assert!(mail.len() < 2_048, "whole mailto URL stays bounded");
        let encoded_body = mail.split("&body=").nth(1).expect("body present");
        assert!(!encoded_body.is_empty(), "body must not be starved");
    }

    /// Never cut in the middle of a `%XX` escape — a half escape makes the URL invalid.
    #[test]
    fn truncation_never_splits_a_percent_escape() {
        let encoded = encode_bounded(&"—".repeat(500), 200);
        for (i, ch) in encoded.char_indices() {
            if ch == '%' {
                assert!(
                    encoded[i + 1..].len() >= 2,
                    "trailing escape was cut in half"
                );
            }
        }
    }

    #[test]
    fn the_subject_names_the_app_and_the_error() {
        let crash = "Crashed: now\nPanic at src/x.rs:1\nMessage: engine went away\n";
        assert_eq!(
            subject(Some(crash), ""),
            "[Freally Player] engine went away"
        );
        // No crash: the user's first non-empty line.
        assert_eq!(
            subject(None, "\n\nvideo is green\nmore detail"),
            "[Freally Player] video is green"
        );
        // Neither: a generic but honest label.
        assert_eq!(subject(None, "   "), "[Freally Player] bug report");
    }

    #[test]
    fn open_url_refuses_anything_but_https_and_mailto() {
        assert!(open_url("file:///etc/passwd").is_err());
        assert!(open_url("javascript:alert(1)").is_err());
        assert!(open_url("http://example.com").is_err());
        assert!(open_url("https://example.com/\nX-Injected: 1").is_err());
    }

    #[test]
    fn the_body_carries_the_diagnostics_and_honours_the_style() {
        let md = compose_body("it broke", None, BodyStyle::Markdown);
        assert!(md.contains("### What happened"));
        assert!(md.contains("From: Freally Player"));

        let plain = compose_body("it broke", None, BodyStyle::Plain);
        assert!(plain.contains("WHAT HAPPENED"));
        assert!(
            !plain.contains("###"),
            "mail bodies carry no markdown noise"
        );
    }

    #[test]
    fn an_unknown_submit_target_is_rejected() {
        let err = bug_report_submit("pastebin".into(), String::new(), false)
            .expect_err("unknown target must be refused");
        assert!(err.contains("unknown report target"));
    }
}
