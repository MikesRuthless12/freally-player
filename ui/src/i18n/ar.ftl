# Freally Player — العربية. Checked against en.ftl by `npm run i18n:lint`.
# "Freally Player" is the brand and is never translated.
#
# Arabic is the one RTL locale of the 18: selecting it sets `dir="rtl"` on the document root
# and mirrors the whole shell. See `applyLocale` in ./index.ts.

titlebar-settings = الإعدادات
titlebar-about = حول
titlebar-minimize = تصغير
titlebar-maximize = تكبير
titlebar-restore = استعادة
titlebar-close = إغلاق

eula-heading = Freally Player — اتفاقية ترخيص المستخدم النهائي
eula-version = الإصدار { $version }
eula-intro = يُرجى قراءة الاتفاقية أدناه والموافقة عليها لاستخدام Freally Player.
eula-scroll-prompt = مرّر إلى نهاية الاتفاقية للمتابعة.
eula-scrolled = شكرًا لقراءتك.
eula-decline = الرفض والخروج
eula-agree = أوافق

stage-label = منطقة الفيديو
stage-empty = لا توجد وسائط محمّلة
transport-open = فتح وسائط…
transport-play = تشغيل
transport-pause = إيقاف مؤقت
transport-back = −10 ث
transport-forward = +10 ث

scrubber-label = التنقل
transport-frame-back = الإطار السابق
transport-frame-forward = الإطار التالي
transport-mute = كتم الصوت
transport-unmute = إلغاء الكتم
transport-volume = مستوى الصوت
transport-speed = سرعة التشغيل
transport-chapters = الفصول
transport-chapter-n = الفصل { $n }
transport-ab-set-a = تعيين بداية التكرار
transport-ab-set-b = تعيين نهاية التكرار
transport-ab-clear = مسح التكرار
transport-snapshot = حفظ لقطة
transport-fullscreen = ملء الشاشة
transport-exit-fullscreen = إنهاء ملء الشاشة

idle-title = لا توجد وسائط محمّلة
idle-drop-hint = أفلِت مقطع فيديو هنا، أو افتح واحدًا.
idle-continue = متابعة المشاهدة

status-idle = خامل
status-playing = قيد التشغيل
status-paused = موقوف مؤقتًا

footer-report-bug = الإبلاغ عن خلل
footer-theme-light = الوضع الفاتح
footer-theme-dark = الوضع الداكن
footer-switch-to-light = التبديل إلى الوضع الفاتح
footer-switch-to-dark = التبديل إلى الوضع الداكن
footer-version-unavailable = الإصدار غير متاح

settings-title = الإعدادات
settings-categories = فئات الإعدادات
settings-close = إغلاق
settings-general = عام
settings-appearance = المظهر
settings-language = اللغة
settings-about = حول

settings-window-title = النافذة
settings-window-hint = كيف يتصرف Freally Player عندما تضعه جانبًا.
settings-tray-label = التصغير إلى شريط النظام
settings-tray-hint = التصغير يُخفي النافذة ويترك أيقونة في شريط النظام. انقر على الأيقونة لإعادتها.

settings-theme-title = السمة
settings-theme-hint = الداكن هو الافتراضي في Havoc.
settings-theme-dark = داكن
settings-theme-light = فاتح

settings-language-title = لغة الواجهة
settings-language-hint = تُطبَّق فورًا — لا حاجة لإعادة تشغيل أي شيء.

settings-about-hint = يشغّل كل شيء. بجمال. بلا إعلانات وبلا برامج تجسّس.
settings-about-version = الإصدار
settings-about-licence = الترخيص
settings-about-rights = © 2026 Mike Weaver — جميع الحقوق محفوظة
settings-about-privacy = الخصوصية
settings-about-privacy-value = بلا إعلانات، بلا قياس عن بُعد، بلا تحليلات، بلا حساب.

bug-title = الإبلاغ عن خلل
bug-close = إغلاق
bug-intro = الإبلاغ اختياري ومجهول الهوية. لا يُرسَل أي شيء تلقائيًا ولا يوجد خادم — الأزرار أدناه تفتح مسودة معبّأة مسبقًا ترسلها أنت بنفسك.
bug-pending-crash = أُغلق Freally Player بشكل غير متوقع في المرة السابقة. تقرير الأعطال أدناه محفوظ على هذا الجهاز فقط.
bug-what-happened = ماذا حدث؟
bug-placeholder = ماذا كنت تفعل عندما حدث الخلل؟
bug-include-crash = تضمين مقتطف العطل
bug-preview-heading = هذا بالضبط ما سيُرسَل
bug-submit-github = فتح مشكلة على GitHub
bug-submit-gmail = الكتابة في Gmail
bug-submit-email = إرسال بريد إلكتروني
bug-copy = نسخ التقرير
bug-copied = تم النسخ
bug-dismiss-crash = تجاهل العطل
bug-copy-failed = تعذّر نسخ التقرير إلى الحافظة.


## Audio, subtitles & online fetch (Phase 2).

audio-menu = الصوت
audio-tracks = مسارات الصوت
audio-track-n = المسار { $n }

subtitle-menu = الترجمات
subtitle-tracks = مسار الترجمة
subtitle-off = إيقاف
subtitle-track-n = المسار { $n }
subtitle-external = خارجي
subtitle-image-based = صورة
subtitle-load-file = تحميل ملف ترجمة…
subtitle-visible = إظهار الترجمة
subtitle-secondary = ترجمة ثانية
subtitle-adjust = التوقيت والموضع
subtitle-delay = التأخير
subtitle-position = الموضع
subtitle-scale = الحجم
subtitle-reset = إعادة تعيين
unit-seconds-short = ث

subtitle-loaded = تم تحميل الترجمة.
subtitle-loaded-encoding = تم التحميل، مُحوَّل من { $encoding }.
subtitle-loaded-image = تم تحميل ترجمة صورية — لا ينطبق التنسيق.

subtitle-online = ترجمات عبر الإنترنت
subtitle-online-disabled = فعِّل الترجمات عبر الإنترنت من الإعدادات للبحث.
subtitle-online-query = العنوان المراد البحث عنه
subtitle-online-languages = اللغات
subtitle-online-search = بحث
subtitle-online-username = اسم المستخدم
subtitle-online-password = كلمة المرور
subtitle-online-signin = تسجيل الدخول

settings-subtitles = الترجمات

settings-sub-style-title = نمط الترجمة
settings-sub-style-hint = فرض شكل الترجمة لتحسين قابلية القراءة.
settings-sub-style-enable = تجاوز نمط الترجمة
settings-sub-style-enable-hint = يستخدم الخط والحجم واللون الذي تختاره بدلاً من نمط الملف. ينطبق على الترجمات النصية فقط.
settings-sub-style-font = الخط
settings-sub-style-size = الحجم
settings-sub-style-color = اللون

settings-online-title = ترجمات عبر الإنترنت
settings-online-hint = جلب الترجمات من OpenSubtitles. مُعطَّل افتراضيًا.
settings-online-enable = تفعيل جلب الترجمات عبر الإنترنت
settings-online-enable-hint = اختياري. يتطلب حسابك المجاني على OpenSubtitles ومفتاح API خاصًا بك.
settings-online-key = مفتاح API
settings-online-username = اسم المستخدم
settings-online-privacy = يُرسَل فقط العنوان واللغات التي تبحث عنها. لا يتم تخزين كلمة المرور أبدًا.
