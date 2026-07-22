# Freally Player — English. THE SOURCE CATALOG.
#
# Every other locale is checked against this file key for key by `npm run i18n:lint`, which
# fails on a key that is here and missing there, or there and not here. Add a string here
# first, then translate it into all 17 others IN THE SAME CHANGE — a half-translated catalog
# is how a shipped locale silently degrades into half-English.
#
# The product name "Freally Player" is never translated; it is the brand, not a string.


## Title bar. These are icon-only buttons, so the labels are what a screen reader announces.

titlebar-settings = Settings
titlebar-about = About
titlebar-minimize = Minimize
titlebar-maximize = Maximize
titlebar-restore = Restore
titlebar-close = Close


## First-run agreement gate. The agreement TEXT itself stays in English — it is a legal
## document, not UI chrome, and a translated licence would not be the one that binds.

eula-heading = Freally Player — End User License Agreement
eula-version = Version { $version }
eula-intro = Please read and accept the agreement below to use Freally Player.
eula-scroll-prompt = Scroll to the end of the agreement to continue.
eula-scrolled = Thanks for reading.
eula-decline = Decline & Quit
eula-agree = I Agree


## The video stage and the transport.

stage-label = Video stage
stage-empty = No media loaded
transport-open = Open media…
transport-play = Play
transport-pause = Pause
transport-back = −10s
transport-forward = +10s

# Transport status, shown beside the open media's title.
status-idle = idle
status-playing = playing
status-paused = paused


## Footer.

footer-report-bug = Report a bug
footer-theme-light = Light mode
footer-theme-dark = Dark mode
footer-switch-to-light = Switch to light mode
footer-switch-to-dark = Switch to dark mode
footer-version-unavailable = version unavailable


## Settings.

settings-title = Settings
settings-categories = Settings categories
settings-close = Close
settings-general = General
settings-appearance = Appearance
settings-language = Language
settings-about = About

settings-window-title = Window
settings-window-hint = How Freally Player behaves when you put it away.
settings-tray-label = Minimize to system tray
settings-tray-hint = Minimising hides the window and leaves a tray icon. Click the icon to bring it back.

settings-theme-title = Theme
settings-theme-hint = Dark is the Havoc default.
settings-theme-dark = Dark
settings-theme-light = Light

settings-language-title = Interface language
settings-language-hint = Applies straight away — nothing needs restarting.

settings-about-hint = Plays anything. Beautifully. No ads, no spyware.
settings-about-version = Version
settings-about-licence = Licence
settings-about-rights = © 2026 Mike Weaver — All Rights Reserved
settings-about-privacy = Privacy
settings-about-privacy-value = No ads, no telemetry, no analytics, no account.


## Bug reporter.
##
## The report PREVIEW is not in here on purpose. It is headed "exactly what will be sent" and
## mirrors `compose_body()` in `src-tauri/src/bugreport.rs`, which builds the real report in
## English for whoever reads it. Translating the preview would make that heading untrue.

bug-title = Report a bug
bug-close = Close
bug-intro = Reporting is opt-in and anonymous. Nothing is sent automatically and there is no server — the buttons below open a pre-filled draft that you send yourself.
bug-pending-crash = Freally Player closed unexpectedly last time. The crash report below is saved on this machine only.
bug-what-happened = What happened?
bug-placeholder = What were you doing when it went wrong?
bug-include-crash = Include the crash excerpt
bug-preview-heading = Exactly what will be sent
bug-submit-github = Open GitHub issue
bug-submit-gmail = Compose in Gmail
bug-submit-email = Send email
bug-copy = Copy report
bug-copied = Copied
bug-dismiss-crash = Dismiss crash
bug-copy-failed = Could not copy the report to the clipboard.
