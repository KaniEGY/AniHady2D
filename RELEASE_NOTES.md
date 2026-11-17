# Stick — Release notes / ملاحظات التحديث

> **Maintainer checklist:** Update the English and Egyptian Arabic notes for **every new release** before publishing. Never claim features that did not pass tests.

## English (US)

### What's new
- Right-click timeline frames to Copy, Paste, Clear, Delete, create Keyframes or Blank Keyframes; removing time shifts all unlocked layers together.
- Pick separate Onion Skin frame counts and transparency for earlier and later frames; adjust loop start/end and timeline zoom.
- Layers now have Delete (with confirmation), Duplicate, Move Up/Down, visibility and lock controls, and true 0–100% opacity.
- Shape tools now have size, opacity and optional solid fill. Brush and eraser cursors show their actual size. The Properties panel scrolls independently of the canvas.
- Closed vector shapes can now be filled by the Bucket tool; selection offers rectangular marquee and freehand lasso, and text can be dragged using the Selection tool.
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
- Fixed broken Arabic word order in Save Your Work Before Update notices.
- Fixed the Bucket painting a nearby line instead of finding a clicked enclosed vector shape.
- Fixed the layer delete control being missing; fixed opacity/visibility changes not always marking the document as unsaved.
- Removed an unsafe old Merge ↓ button that could silently drop keyframe content.
- Fixed the main problem: successful Windows builds on main were not being published as updates, so users saw no newer version.
- Fixed standalone Stick.exe reporting an old or missing version.txt instead of its own compiled build number.
- Fixed version checks that could offer the same build again.
- Fixed the updater getting stuck if GitHub can't be reached.
- Added an unsaved-work guard to avoid restarting with an unfinished project.

## العربية (المصري)

### إيه الجديد؟
- لما تدوس كليك يمين على فريم في التايم لاين، هتلاقي نسخ ولصق ومسح وحذف وكي فريم وكي فريم فاضي، مع حذف الزمن بشكل منظم من كل الليرز المفتوحة.
- الـ Onion Skin بقى فيه عدد فريمات وشفافية منفصلين للقديم والجديد، وكمان تقدر تحدد بداية ونهاية الـ Loop وتكبر التايم لاين.
- الليرز بقى فيها حذف بتأكيد، ونسخ وتحريك لفوق وتحت، وإظهار وقفل، وتحكم في الشفافية من صفر لـ 100%.
- أداة الأشكال بقى ليها حجم وشفافية ولون تعبئة اختياري. وحجم مؤشر الفرشاة والممحاة بقى بيتغير مع حجمهم، والخصائص بتتحرك لوحدها بالسكرول.
- البوكت بقى بيلوّن الأشكال الفيكتور المقفولة، والتحديد بقى مستطيل أو لاسو، والنص تقدر تحركه بأداة التحديد.
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
- صلحنا مشكلة ترتيب كلام رسالة «احفظ شغلك الأول» اللي كانت بتظهر قبل التحديث.
- صلحنا البوكت اللي كان بيلوّن خط قريب بدل المنطقة المقفولة اللي اخترتها.
- ضفنا حذف اللير وخلّينا تغييرات الشفافية والإظهار تتحفظ مع المشروع.
- شيلنا زر دمج قديم كان ممكن يضيّع محتوى الكي فريمات في ليرز تانية.
- صلحنا مشكلة إن النسخ الجديدة كانت بتتعمل على GitHub بس مبتتنشرش للمستخدمين كتحديثات.
- صلحنا ظهور رقم إصدار غلط لما ملف version.txt يكون قديم أو مش موجود.
- صلحنا مقارنة الإصدارات عشان نفس التحديث ميتعرضش كل شوية.
- صلحنا تعليق البحث عن تحديثات لما الاتصال بـGitHub يفشل.
- ضفنا تنبيه يحمي مشروعك لو عندك شغل لسه متحفظش.

> **For testers:** Every successful main build is published automatically. Please report issues using GitHub Issues. Back up important projects when testing new builds.

## Known limits / حدود النسخة التجريبية

- Bucket fill currently supports **closed vector shapes**. A genuine raster flood-fill brush-painted region is still being designed: https://github.com/KaniEGY/Stick/issues/15
- Krita/Blender brush asset importing isn't shipping in this update; source formats/licenses and engine work are tracked here: https://github.com/KaniEGY/Stick/issues/14
- This public build is for testing. Save backups of important projects and report problems via GitHub Issues.

- **مهم:** البوكت دلوقتي بيملأ الأشكال الفيكتور المقفولة بس. تلوين أي مساحة مرسومة بالفرشة محتاج محرك Raster مستقل.
- فرش Krita وBlender الأصلية لسه ما اتضافتش. هنستورد اللي ترخيصه يسمح بعد ما نجهز محرك فرش حقيقي ومتوافق.
