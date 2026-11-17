# Stick — Release notes / ملاحظات التحديث

> **Maintainer checklist:** Update the English and Egyptian Arabic notes for **every new release** before publishing. Never claim features that did not pass tests.

## English (US)

### What's new
- Public test builds now publish automatically on GitHub Releases whenever a new version on main passes Windows tests and build. All users can try the newest version.
- The Check for Updates button is available from the start screen and the drawing/animation toolbar. The installed build number is compiled into Stick.exe.
- Users can report a bug or suggest a feature directly from Stick using the GitHub Issues page and ready-to-fill templates.
- First-run What's New uses the real release notes embedded inside Stick.exe, shown in simple US English or Egyptian Arabic.
- Updates download in small chunks to reduce RAM use on low-end PCs.
- New Drawing and Animation tools: pressure-aware G-Pen sampling, better layer-opacity export, bilingual toolbar, and Onion Skin frame previews.
- Preview can skip fully offscreen stroke geometry on weaker PCs.
- Check for updates directly from the Stick toolbar. When nothing new is available, Stick tells you you're up to date.
- Updates are downloaded with a SHA-256 integrity check before they are installed.
- Stick restarts to finish a verified update, and shows What's New the first time you start the new version.

### Bugs fixed
- Fixed the main problem: successful Windows builds on main were not being published as updates, so users saw no newer version.
- Fixed standalone Stick.exe reporting an old or missing version.txt instead of its own compiled build number.
- Fixed version checks that could offer the same build again.
- Fixed the updater getting stuck if GitHub can't be reached.
- Added an unsaved-work guard to avoid restarting with an unfinished project.

## العربية (المصري)

### إيه الجديد؟
- أي نسخة جديدة بنضيفها لـ main وبتنجح في اختبارات وبناء Windows هتنزل تلقائيًا على GitHub Releases، عشان كل الناس تقدر تجربها.
- زرار «تحديث البرنامج» بقى متاح من الشاشة الرئيسية وكمان جوّه وضع الرسم والأنيميشن. ورقم الإصدار الحقيقي بقى متسجل جوّه Stick.exe نفسه.
- ضفنا رابط واضح للإبلاغ عن المشاكل واقتراح المميزات، ومعاه نماذج بسيطة عربي وإنجليزي.
- نافذة «إيه الجديد» بقت بتاخد التفاصيل من ملف الإصدار نفسه، وبتعرضها بالمصري أو إنجليزي أمريكي بسيط.
- تنزيل التحديث بقى على أجزاء صغيرة عشان يفضل خفيف على الأجهزة الضعيفة.
- طورنا وضع الرسم: ضغط قلم G-Pen، وشفافية الطبقات في التصدير، وأسماء الأدوات، وعرض الفريمات المجاورة في الأنيميشن.
- الرسم اللي خارج حدود الشاشة مش هيستهلك نفس المجهود في المعاينة.
- تقدر تدوس على «تحديثات» من شريط البرنامج. لو مفيش حاجة جديدة، Stick هيقولك إنك على آخر إصدار.
- Stick بيتأكد من سلامة ملف التحديث ببصمة SHA-256 قبل التثبيت.
- بعد ما التحديث يجهز، البرنامج بيعيد التشغيل، وبيوريك «إيه الجديد» أول مرة تفتح الإصدار الجديد.

### المشاكل اللي اتصلحت
- صلحنا مشكلة إن النسخ الجديدة كانت بتتعمل على GitHub بس مبتتنشرش للمستخدمين كتحديثات.
- صلحنا ظهور رقم إصدار غلط لما ملف version.txt يكون قديم أو مش موجود.
- صلحنا مقارنة الإصدارات عشان نفس التحديث ميتعرضش كل شوية.
- صلحنا تعليق البحث عن تحديثات لما الاتصال بـGitHub يفشل.
- ضفنا تنبيه يحمي مشروعك لو عندك شغل لسه متحفظش.

> **For testers:** Every successful main build is published automatically. Please report issues using GitHub Issues. Back up important projects when testing new builds.
