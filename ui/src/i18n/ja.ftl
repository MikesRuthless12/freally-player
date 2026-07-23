# Freally Player — 日本語. Checked against en.ftl by `npm run i18n:lint`.
# "Freally Player" is the brand and is never translated.

titlebar-settings = 設定
titlebar-about = このアプリについて
titlebar-minimize = 最小化
titlebar-maximize = 最大化
titlebar-restore = 元のサイズに戻す
titlebar-close = 閉じる

eula-heading = Freally Player — エンドユーザー使用許諾契約
eula-version = バージョン { $version }
eula-intro = Freally Player を使用するには、以下の契約をお読みのうえ同意してください。
eula-scroll-prompt = 続けるには契約の最後までスクロールしてください。
eula-scrolled = お読みいただきありがとうございます。
eula-decline = 同意せずに終了
eula-agree = 同意する

stage-label = ビデオステージ
stage-empty = メディアが読み込まれていません
transport-open = メディアを開く…
transport-play = 再生
transport-pause = 一時停止
transport-back = −10秒
transport-forward = +10秒

scrubber-label = シーク
transport-frame-back = 前のフレーム
transport-frame-forward = 次のフレーム
transport-mute = ミュート
transport-unmute = ミュート解除
transport-volume = 音量
transport-speed = 再生速度
transport-chapters = チャプター
transport-chapter-n = チャプター { $n }
transport-ab-set-a = リピート開始を設定
transport-ab-set-b = リピート終了を設定
transport-ab-clear = リピートを解除
transport-snapshot = スナップショットを保存
transport-fullscreen = 全画面表示
transport-exit-fullscreen = 全画面表示を終了

idle-title = メディアが読み込まれていません
idle-drop-hint = ここに動画をドロップするか、開いてください。
idle-continue = 続きを見る

status-idle = 待機中
status-playing = 再生中
status-paused = 一時停止中

footer-report-bug = バグを報告
footer-theme-light = ライトモード
footer-theme-dark = ダークモード
footer-switch-to-light = ライトモードに切り替える
footer-switch-to-dark = ダークモードに切り替える
footer-version-unavailable = バージョンを取得できません

settings-title = 設定
settings-categories = 設定のカテゴリ
settings-close = 閉じる
settings-general = 一般
settings-appearance = 外観
settings-language = 言語
settings-about = このアプリについて

settings-window-title = ウィンドウ
settings-window-hint = 使っていないときの Freally Player の動作。
settings-tray-label = 通知領域に最小化する
settings-tray-hint = 最小化するとウィンドウが隠れ、通知領域にアイコンが残ります。アイコンをクリックすると元に戻ります。

settings-theme-title = テーマ
settings-theme-hint = ダークが Havoc の既定です。
settings-theme-dark = ダーク
settings-theme-light = ライト

settings-language-title = 表示言語
settings-language-hint = すぐに適用されます。再起動は不要です。

settings-about-hint = 何でも再生。美しく。広告なし、スパイウェアなし。
settings-about-version = バージョン
settings-about-licence = ライセンス
settings-about-rights = © 2026 Mike Weaver — All Rights Reserved
settings-about-privacy = プライバシー
settings-about-privacy-value = 広告なし、テレメトリなし、解析なし、アカウント不要。

bug-title = バグを報告
bug-close = 閉じる
bug-intro = 報告は任意かつ匿名です。自動送信は一切なく、サーバーもありません。下のボタンは内容が入力済みの下書きを開くだけで、送信するのはあなたです。
bug-pending-crash = 前回、Freally Player が予期せず終了しました。以下のクラッシュレポートはこのパソコンにのみ保存されています。
bug-what-happened = 何が起きましたか？
bug-placeholder = 問題が起きたとき、何をしていましたか？
bug-include-crash = クラッシュの抜粋を含める
bug-preview-heading = 送信される内容のすべて
bug-submit-github = GitHub で issue を開く
bug-submit-gmail = Gmail で作成
bug-submit-email = メールを送信
bug-copy = レポートをコピー
bug-copied = コピーしました
bug-dismiss-crash = クラッシュを破棄
bug-copy-failed = レポートをクリップボードにコピーできませんでした。


## Audio, subtitles & online fetch (Phase 2).

audio-menu = 音声
audio-tracks = 音声トラック
audio-track-n = トラック { $n }

subtitle-menu = 字幕
subtitle-tracks = 字幕トラック
subtitle-off = オフ
subtitle-track-n = トラック { $n }
subtitle-external = 外部
subtitle-image-based = 画像
subtitle-load-file = 字幕ファイルを読み込む…
subtitle-visible = 字幕を表示
subtitle-secondary = 2つ目の字幕
subtitle-adjust = タイミングと位置
subtitle-delay = 遅延
subtitle-position = 位置
subtitle-scale = サイズ
subtitle-reset = リセット
unit-seconds-short = 秒

subtitle-loaded = 字幕を読み込みました。
subtitle-loaded-encoding = { $encoding } から変換して読み込みました。
subtitle-loaded-image = 画像ベースの字幕を読み込みました — スタイルは適用されません。

subtitle-online = オンライン字幕
subtitle-online-disabled = 検索するには設定でオンライン字幕を有効にしてください。
subtitle-online-query = 検索するタイトル
subtitle-online-languages = 言語
subtitle-online-search = 検索
subtitle-online-username = ユーザー名
subtitle-online-password = パスワード
subtitle-online-signin = サインイン

settings-subtitles = 字幕

settings-sub-style-title = 字幕スタイル
settings-sub-style-hint = 読みやすさのため、字幕の見た目を強制します。
settings-sub-style-enable = 字幕スタイルを上書き
settings-sub-style-enable-hint = ファイル本来のスタイルではなく、指定したフォント・サイズ・色を使います。テキスト字幕にのみ適用されます。
settings-sub-style-font = フォント
settings-sub-style-size = サイズ
settings-sub-style-color = 色

settings-online-title = オンライン字幕
settings-online-hint = OpenSubtitles から字幕を取得します。既定ではオフです。
settings-online-enable = オンライン字幕の取得を有効にする
settings-online-enable-hint = オプトイン。ご自身の無料の OpenSubtitles アカウントと API キーが必要です。
settings-online-key = API キー
settings-online-username = ユーザー名
settings-online-privacy = 検索したタイトルと言語のみが送信されます。パスワードは保存されません。
