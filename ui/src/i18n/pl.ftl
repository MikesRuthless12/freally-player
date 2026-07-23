# Freally Player — Polski. Checked against en.ftl by `npm run i18n:lint`.
# "Freally Player" is the brand and is never translated.

titlebar-settings = Ustawienia
titlebar-about = O programie
titlebar-minimize = Minimalizuj
titlebar-maximize = Maksymalizuj
titlebar-restore = Przywróć
titlebar-close = Zamknij

eula-heading = Freally Player — Umowa licencyjna użytkownika końcowego
eula-version = Wersja { $version }
eula-intro = Przeczytaj i zaakceptuj poniższą umowę, aby korzystać z Freally Player.
eula-scroll-prompt = Przewiń do końca umowy, aby kontynuować.
eula-scrolled = Dziękujemy za przeczytanie.
eula-decline = Odrzuć i zakończ
eula-agree = Zgadzam się

stage-label = Scena wideo
stage-empty = Nie wczytano żadnych multimediów
transport-open = Otwórz multimedia…
transport-play = Odtwórz
transport-pause = Wstrzymaj
transport-back = −10 s
transport-forward = +10 s

scrubber-label = Przewiń
transport-frame-back = Poprzednia klatka
transport-frame-forward = Następna klatka
transport-mute = Wycisz
transport-unmute = Wyłącz wyciszenie
transport-volume = Głośność
transport-speed = Prędkość odtwarzania
transport-chapters = Rozdziały
transport-chapter-n = Rozdział { $n }
transport-ab-set-a = Ustaw początek powtórzenia
transport-ab-set-b = Ustaw koniec powtórzenia
transport-ab-clear = Wyczyść powtórzenie
transport-snapshot = Zapisz zrzut
transport-fullscreen = Pełny ekran
transport-exit-fullscreen = Zamknij pełny ekran

idle-title = Nie wczytano żadnych multimediów
idle-drop-hint = Upuść tutaj wideo lub otwórz jedno.
idle-continue = Oglądaj dalej

status-idle = bezczynny
status-playing = odtwarzanie
status-paused = wstrzymano

footer-report-bug = Zgłoś błąd
footer-theme-light = Tryb jasny
footer-theme-dark = Tryb ciemny
footer-switch-to-light = Przełącz na tryb jasny
footer-switch-to-dark = Przełącz na tryb ciemny
footer-version-unavailable = wersja niedostępna

settings-title = Ustawienia
settings-categories = Kategorie ustawień
settings-close = Zamknij
settings-general = Ogólne
settings-appearance = Wygląd
settings-language = Język
settings-about = O programie

settings-window-title = Okno
settings-window-hint = Jak Freally Player zachowuje się, gdy go odkładasz.
settings-tray-label = Minimalizuj do zasobnika systemowego
settings-tray-hint = Minimalizowanie ukrywa okno i pozostawia ikonę w zasobniku. Kliknij ikonę, aby przywrócić okno.

settings-theme-title = Motyw
settings-theme-hint = Ciemny to domyślny motyw Havoc.
settings-theme-dark = Ciemny
settings-theme-light = Jasny

settings-language-title = Język interfejsu
settings-language-hint = Działa od razu — nic nie trzeba uruchamiać ponownie.

settings-about-hint = Odtwarza wszystko. Pięknie. Bez reklam, bez oprogramowania szpiegującego.
settings-about-version = Wersja
settings-about-licence = Licencja
settings-about-rights = © 2026 Mike Weaver — Wszelkie prawa zastrzeżone
settings-about-privacy = Prywatność
settings-about-privacy-value = Bez reklam, bez telemetrii, bez analityki, bez konta.

bug-title = Zgłoś błąd
bug-close = Zamknij
bug-intro = Zgłaszanie jest dobrowolne i anonimowe. Nic nie jest wysyłane automatycznie i nie ma żadnego serwera — przyciski poniżej otwierają wstępnie wypełniony szkic, który wysyłasz samodzielnie.
bug-pending-crash = Freally Player zamknął się nieoczekiwanie przy ostatnim uruchomieniu. Poniższy raport o awarii jest zapisany wyłącznie na tym komputerze.
bug-what-happened = Co się stało?
bug-placeholder = Co robiłeś, kiedy wystąpił problem?
bug-include-crash = Dołącz fragment awarii
bug-preview-heading = Dokładnie to zostanie wysłane
bug-submit-github = Otwórz zgłoszenie na GitHubie
bug-submit-gmail = Napisz w Gmailu
bug-submit-email = Wyślij e-mail
bug-copy = Kopiuj raport
bug-copied = Skopiowano
bug-dismiss-crash = Odrzuć awarię
bug-copy-failed = Nie udało się skopiować raportu do schowka.


## Audio, subtitles & online fetch (Phase 2).

audio-menu = Dźwięk
audio-tracks = Ścieżki dźwiękowe
audio-track-n = Ścieżka { $n }

subtitle-menu = Napisy
subtitle-tracks = Ścieżka napisów
subtitle-off = Wyłączone
subtitle-track-n = Ścieżka { $n }
subtitle-external = zewnętrzne
subtitle-image-based = obraz
subtitle-load-file = Wczytaj plik napisów…
subtitle-visible = Pokaż napisy
subtitle-secondary = Drugie napisy
subtitle-adjust = Synchronizacja i położenie
subtitle-delay = Opóźnienie
subtitle-position = Położenie
subtitle-scale = Rozmiar
subtitle-reset = Resetuj
unit-seconds-short = s

subtitle-loaded = Wczytano napisy.
subtitle-loaded-encoding = Wczytano, przekonwertowano z { $encoding }.
subtitle-loaded-image = Wczytano napisy obrazkowe — styl nie ma zastosowania.

subtitle-online = Napisy online
subtitle-online-disabled = Włącz napisy online w Ustawieniach, aby wyszukiwać.
subtitle-online-query = Tytuł do wyszukania
subtitle-online-languages = Języki
subtitle-online-search = Szukaj
subtitle-online-username = Nazwa użytkownika
subtitle-online-password = Hasło
subtitle-online-signin = Zaloguj się

settings-subtitles = Napisy

settings-sub-style-title = Styl napisów
settings-sub-style-hint = Wymuś wygląd napisów dla lepszej czytelności.
settings-sub-style-enable = Zastąp styl napisów
settings-sub-style-enable-hint = Używa Twojej czcionki, rozmiaru i koloru zamiast stylu pliku. Dotyczy tylko napisów tekstowych.
settings-sub-style-font = Czcionka
settings-sub-style-size = Rozmiar
settings-sub-style-color = Kolor

settings-online-title = Napisy online
settings-online-hint = Pobieraj napisy z OpenSubtitles. Domyślnie wyłączone.
settings-online-enable = Włącz pobieranie napisów online
settings-online-enable-hint = Opcjonalne. Wymaga własnego bezpłatnego konta OpenSubtitles i klucza API.
settings-online-key = Klucz API
settings-online-username = Nazwa użytkownika
settings-online-privacy = Wysyłane są tylko wyszukiwany tytuł i języki. Twoje hasło nigdy nie jest zapisywane.
