# Stick — Release notes / ملاحظات التحديث

> **Maintainer checklist:** Update the English and Egyptian Arabic notes for **every new release** before publishing. Never claim features that did not pass tests.

## English (US)

### Bugs fixed
- **Support popup:** bounded window size and scrollable body, with accessible Close/X and Esc; prevents giant fullscreen-height popup caused by unconstrained RTL layout.
- **Google OAuth:** older settings with an empty client ID now pick up a public Desktop OAuth ID embedded in newer builds; Sign in is disabled with setup guidance until a valid client ID is present. Real Google sign-in still requires app-owner Google Cloud configuration and testing.
- **Online:** stroke packets now share the host edit-order gate with structural changes; local TCP integration test checks that bad room passwords are rejected. Internet access still requires LAN reachability or Tailscale, and real two-PC QA is outstanding.
- **Brushes:** removed forced minimum watercolor/airbrush halo alpha when actual stroke opacity is zero, with regression coverage.

### Artist workspace refresh
- **Art System / نظام الفنان:** an anime-inspired, bilingual artist profile with editable name, selectable local avatar and rank progress. XP award formulas and rank thresholds are no longer displayed in the settings UI.
- **Drawing:** improved lasso geometry and drag-to-move; free-rotation selection handle; canvas zoom anchored under mouse; right-drag canvas rotation with the Hand tool; paper, strokes and imported bitmap objects share the view transform.
- **Tools:** colored cursor previews, right-click quick choices for common tools, two extra geometric shapes (triangle/pentagon), and six persistent brush-tip shapes shared between live rendering and image/video exports.
- **Workspace:** user-controlled toolbar, properties dock, tool list and timeline visibility; enhanced layer rows; refreshed timeline controls and layout.
- **Text:** multiline text editor with joined-Arabic preview; local-font-based text rendering in exported images/video.
- **Persistence:** profile/settings saved under the Windows user profile, with read compatibility for prior executable-adjacent JSON files; new stroke tip field defaults to a round brush in older projects.
- **Known boundaries:** bucket fill operates on recognized closed stroke contours (now with improved near-closure tolerance), not arbitrary raster flood fill. Imported artwork/text still needs further parity tests for every transform. No Krita GPL code is copied into Stick.

### What's new
- **Arabic support window fix:** donation copy now uses properly ordered, manually wrapped right-aligned Arabic lines, with corrected colloquial spelling and separated emojis to prevent Unicode bidi wrapping errors.
- **Download progress:** verified GitHub releases now show a real download progress bar, completed percent, and transferred/total megabytes before SHA-256 verification and restart.
- **Top-bar feedback:** the blue Feedback label now opens the same in-app Instagram / GitHub report dialog as the launcher.
- **Support modal:** improved its layout, included the developer's requested humorous optional-support message, and retained the original InstaPay link.
- **Online host moderation:** room host can kick members and change each member's cursor/tool color; roster colors relay to all clients.
- **Graceful host handoff (protocol v4):** the oldest admitted guest can take over the room and remaining clients reconnect when the current host leaves normally. Unexpected crashes, unreachable new hosts and full multi-PC QA remain unresolved; see STICK_ONLINE_GUIDE.md. This update is not a worldwide room directory.
- **Artist cloud profile continuity:** opt-in automatic profile-only Google Drive sync for XP, displayed artist name, progress and a bounded 256px avatar PNG; the local avatar path is never uploaded. Restoring a profile refreshes the visible avatar. Requires app-owner OAuth setup and actual Windows sign-in validation.
- **Settings discovery:** main top-right gear is larger, amber and high-contrast with a tooltip.
- **Online test stability:** Windows integration tests run serially to prevent hard-coded local TCP-port contention; this does not establish worldwide room discovery or internet-wide collaboration.
- **Google Sign in / cloud backups (requires developer OAuth setup):** added a new Sign in settings tab using browser-based Desktop OAuth with PKCE, Windows credential storage and private Google Drive appDataFolder for artist XP/level data and saved Stick project files. Supports manual backup, cloud listing, safe restore and opt-in automatic backup after local save. **The production release needs the app owner's Google OAuth Client ID configured first; see GOOGLE_DRIVE_SETUP.md.** External media files are not bundled yet.
- **Perspective guides:** start disabled on every application launch, independent of the prior stored guide setting. New 2-point perspective controls the left/right vanishing points plus horizon; guides remain non-destructive and excluded from exports.
- **Eyedropper accuracy:** visible eyedropper tip now aligns precisely with actual cursor hit point across icon scale levels and samples from the top visible layer first.
- **Simplified Online joining:** cleaner room cards, direct IP/password, and collapsible 8-digit code/help. The existing localhost TCP host↔guest handshake regression test covers the networking protocol; independent-network Tailscale access still depends on device sharing and firewall rules.
- **OLD / NEW Animation Timeline:** the Timeline gear now lets artists switch between the familiar **OLD Timeline** (unchanged original Stick grid) and **NEW Timeline**, a clearer exposure-sheet layout with colored keyframe diamonds, continuous held drawing strips, a time ruler, taller customizable layer rows and independent horizontal zoom.
- **Preserved data & workflows:** switching style only changes local UI preferences; frame/keyframe copy, paste, blank keys, deletion, playback, loop range and project file format keep working through the same underlying commands. Existing preferences load with OLD selected by default.
- **Feedback dialog:** clicking "Report a bug or suggest a feature" on the start screen now opens a closable window with the maintainer's Instagram @ablikash and the project's GitHub Issues page.
- **Source limitation:** the uploaded AniHady2D 0.1.1 archive contains a Windows installer and setup README, not timeline source code or a screenshot of its timeline. NEW uses exposure-sheet design ideas; it is not claimed to be a pixel-perfect copy of the old app. Other already available capabilities (audio/video export, layers, keyframes) were not duplicated.
- **Online privacy/diagnostics:** LAN/Tailscale discovery and room connections are now **explicit opt-in per session**, never started merely by opening the online lobby. The "Tailscale connected, 0 peers" case explains that the host must share device access or invite collaborators into the same tailnet. Room codes remain discovery-only; a remote friend can use the host 100.x IP if allowed.
- **Exit hang:** no synchronous audio-drain on application close. Unsaved documents get Save & Exit / Exit without saving / Cancel.
- **Brush stability:** G-Pen now draws a continuous pressure-aware outline as one translucent geometry rather than a stack of dark, overlapping circles. PNG/video export uses the same outline where possible. Freehand strokes no longer receive destructive fourfold Catmull-Rom oversampling at release.
- **Vector nodes:** vector pen no longer multiplies stored nodes eightfold; configurable node reduction and right-click advanced options, Shift selection and box selection of multiple nodes with combined dragging.
- **Sculpt / Finger:** new drag-to-deform tool with adjustable radius and strength, using local point sampling and smooth falloff. New Blender-like X/Y translation arrows appear on selected content, alongside existing scale and rotate handles.
- **Cursor customization:** Personalization includes brush cursor size, tool icon size and optional circular halo (off by default).
- **Limitations:** Multi-node edit is an MVP, not full Blender Bezier-handle or Grease Pencil parity. Two-computer remote connectivity depends on Tailscale permissions and still requires physical UI QA.  
- **Tools reliability sprint (all 13 toolbar tools reviewed):** Bucket now fills enclosed complex/concave freehand loops with proper tessellation, grows color beneath visible outlines to avoid white halos, supports small ink-edge clicks, and exposes adjustable gap closure and optional all-visible-layer reference. Redrawing the same region changes its color on top of older fills.
- **Erasers:** complete eraser detects long stroke segments even without nearby vertices; partial eraser clips lines at exact intersection points and both erasers sweep over fast pointer paths.
- **Selection & Eyedropper:** precise geometry hit tests replace overly broad bounding boxes; eyedropper picks actual fill/line colors from the frontmost visible layer.
- **Brush & Shapes:** stabilized strokes now retain the pointer-up endpoint; square drawing no longer collapses on perfectly vertical drags. Added comprehensive regression tests; see TOOLS_AUDIT_2026_11_17.md. Windows GUI/physical stylus QA still pending.
- **Friends online (optional Tailscale):** join a room hosted on your friend's own PC from a different Internet connection. Detect the local Tailscale 100.x address, list rooms from visible online tailnet peers using unicast discovery and copy a host address for direct joining. Requires the separately installed Tailscale client and permission to access the host; not a central public room service.
- **Free Transform:** 22px resize hit radii and 21px rotate hit radius, constant in screen coordinates across zoom; all four corners now resize from their opposite anchors. The handles are slightly easier to see.
- **Error guidance:** connection-refused diagnostics distinguish LAN-only addresses from remote Tailscale routes.
- **Online sync v3:** automatic host-relayed updates to strokes, erasing, bucket fill, layers, visibility, frame/keyframe edits, text and transforms. Creating a room explicitly makes your PC its TCP host/server. Existing LAN discovery, eight-digit codes and optional passwords remain.
- **Bucket fix:** fills a bounded region separated by internal lines (for example, either half of a bisected circle), rather than automatically filling the whole outer shape.
- **Timeline:** deleting frame 1 preserves artwork held on the old frame 2; text and symbol instance frames appear as occupied in the timeline.
- **Remaining limits:** overlapping simultaneous edits to the same array position can conflict, assets are not transferred, and two-PC Windows UI acceptance is still necessary. Use host resync if divergent.
- Bright two-tone vector icons for all 13 drawing tools and matching pointer icons; original short, musical game-style UI sound synthesis replaces one-note sine beeps.
- Paint Bucket: bounded raster mask + flood-region vector reconstruction for shapes enclosed by multiple freehand brush strokes; preserves prior direct vector contour filling. Complex cases still have limits.
- Online LAN preview: host the current project, find rooms on the local network, share a copyable 8-digit room code, enter code on LAN or direct IP, and see unique-color participants with names and self-reported levels in the right Layers dock. This is NOT an Internet-wide room directory or verified Google account system. Document-level edits now synchronize in protocol v3, subject to simultaneous-edit conflicts and real-PC QA.
- Perspective settings: non-destructive overlay square grid, parallel lines, or single-point perspective with adjustable spacing, color, line thickness, opacity and vanishing point.
- Persisted timeline panel height and adjustable dock separator (drag between Properties and Layers). Colors and Symmetry inspectors collapse to free space.
- Rollback list now shows the immediate previous release separately from FIVE older releases without duplicating it.
- **Version Backup / النسخة الاحتياطية:** open Settings → Previous Versions to return to the preceding official release or choose from the five latest eligible older GitHub builds, each showing its publication date. All downloads use the existing SHA-256-verified installer flow and require confirmation; unsaved projects block restart.
- **Layers dock refresh:** wider resizable Krita-inspired sidebar with live stroke thumbnails, quick layer-name search, colored selection, stroke/frame metadata, guide-layer context action, and existing visibility, lock, rename, duplicate, delete, move, opacity controls. No Krita GPL code imported.
- **Stick Online beta:** third option under Drawing and Animation. LAN-hosted rooms, optional password, participant limit, LAN discovery and manual direct IPv4 connection. Room-wide project edits, including layers and timeline, now flow through the host; Internet-wide discovery is not implemented. This preview is not end-to-end encrypted.
- Art System: anime-style bilingual artist profile with editable display name, local avatar image and a level/rank progress bar. Progress formulas are not displayed.
- Customizable workspace, cursor-anchored smooth zoom, transform rotation handle, draggable lasso selections, right-click tool choices and more versatile brush-tip geometry.
- A centered 10-second LEVEL UP panel shows one of 1,000 unique motivating lines in easy US English or Egyptian Arabic, with one of 100 small procedural sound variations.
- Settings gear: new Shortcuts, Colors & Timeline, and Personalization tabs with panel visibility controls; sound-effects toggle and artist profile.
- Improved right-side docks: separate scrolling Tool Properties and Layers panels, plus right-click Layer options to Rename, Copy, Paste, Duplicate and Delete.
- Updated timeline appearance, resize by dragging the upper border, and a Timeline gear for separate forward/backward Onion Skin count, tint and opacity.
- Use the original red heart on the left to open YouTube/Instagram, and the gold heart at the top center to open the optional InstaPay support dialog.
- Free Transform now supports dragging a resize handle, Vector Nodes can be picked and dragged, and drawing tools have distinctive cursor previews and sound cues.
- Bucket can also fill clearly closed freehand brush loops as well as closed vector shapes. Fills are included in image/video export.
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
- Vector Nodes tool no longer does nothing when clicked on a vector vertex.
- Selection has a visual transform handle to resize selected strokes/text in stage coordinates.
- Opening Tool Properties no longer forces the Layers list to scroll with it.
- Bucket previously ignored hand-drawn closed loops and lost their interior fill in the G-Pen renderer.
- Cosmetic XP keeps its progress after saving/reopening projects, without counting repeat strokes on the same frame.
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

### المشاكل اللي اتصلحت
- **نافذة الدعم:** حجم ثابت ونص قابل للتمرير وزر إغلاق ظاهر واختصار Esc، لحل مشكلة النافذة الممتدة خارج الشاشة.
- **تسجيل جوجل:** إصلاح قراءة Client ID من النسخة الجديدة عند وجود إعداد فارغ محفوظ من نسخة قديمة، مع منع فتح تسجيل دخول غير مُعد وإظهار التعليمات. الدخول الفعلي يحتاج إعداد OAuth من المطوّر واختبار حساب حقيقي.
- **الأونلاين:** توحيد ترتيب إرسال الرسم والتعديلات واختبار رفض كلمة السر الخطأ. الاتصال خارج الشبكة المحلية ما زال يحتاج Tailscale أو عنوانًا متاحًا، واختبار جهازين مطلوب.
- **الفرش:** تصحيح ظهور أثر لوني للـWatercolor والـAirbrush عند الشفافية صفر.

### تطوير مساحة الفنان
- **نظام الفنان:** بروفايل بشكل أنمي واسم قابل للتغيير وصورة من الجهاز وشريط تقدم، من غير عرض معادلات احتساب النقاط.
- **التحديد والتحويل:** اللّاسو بقى يحسب الخطوط المتقاطعة ويتحرك بعد التحديد، وضفنا مقبض لتدوير التحديد بحرية.
- **الكانفا:** زووم ناحية مؤشر الماوس، تدوير اللوحة بالكليك اليمين مع أداة اليد، وتدوير الورقة نفسها مع الرسومات والصور المستوردة.
- **الأدوات:** مؤشرات بألوان مميزة، خيارات بالكليك اليمين، مثلث وخماسي، وست أشكال لرأس الفرشة محفوظة مع المشروع وموحدة بين المعاينة والتصدير.
- **الواجهة:** إخفاء وإظهار الجنبين والشريط العلوي والتايم لاين من التخصيص، وتجديد صفوف الطبقات وشكل الخط الزمني.
- **النصوص:** محرر نص متعدد الأسطر مع معاينة عربية متصلة، ورسم النصوص في التصدير باستخدام خط محلي من Windows.
- **التوافق:** حفظ الإعدادات والبروفايل في مجلد المستخدم القابل للكتابة مع استيراد الملفات القديمة، وفرش المشاريع القديمة تفضل دائرية تلقائيًا.
- **القيود الحالية:** الدلو بيلون المسارات المقفولة اللي بيتعرف عليها، ولسه مش Flood Fill كامل على أي سطح Raster. لم يتم نسخ أكواد Krita الخاضعة لترخيص GPL.

### إيه الجديد؟
- **تصحيح نافذة الدعم:** ضبط صياغة رسالة الدعم المصري وتقسيمها إلى سطور قصيرة بمحاذاة اليمين لمنع تلخبط ترتيب الحروف والكلمات أثناء العرض.
- **شريط التحميل:** النسبة المئوية وحجم الملف المحمّل أثناء تحديث Stick، ثم التحقق من سلامة الملف قبل التثبيت.
- **الإبلاغ:** زر «ابلغ عن مشكلة» فوق بيفتح نفس نافذة GitHub وInstagram الموجودة في الشاشة الرئيسية.
- **الدعم:** شكل نافذة القلب الأصفر اتنظّم ونص الدعم اتغيّر حسب طلب المطوّر، مع الحفاظ على رابط InstaPay.
- **إدارة الهوست:** طرد أعضاء الغرفة وتغيير لونهم ولون مؤشر أدواتهم من قائمة إدارة الغرفة، مع مزامنة الألوان.
- **نقل الهوست عند المغادرة الطبيعية:** عند خروج صاحب الروم، أول ضيف دخل في جلسة الهوست الحالية يقدر يستلم السيرفر، وباقي الأعضاء يعيدوا الاتصال تلقائيًا عبر LAN أو Tailscale. البروتوكول بقى v4. ده مش حل لانقطاع الكهرباء المفاجئ أو غياب الاتصال بالهوست الجديد، ولسه محتاج اختبار على أكثر من جهاز.
- **الحفظ السحابي لملف الفنان:** المزامنة الاختيارية بقت ترفع مستوى الفنان والاسم والتقدم والصورة الشخصية المصغرة بشكل مستقل عن حفظ المشروع. الصورة بتتخزن كصورة صغيرة مش كمسار من الجهاز، وتظهر عند الاستعادة. لازم إعداد OAuth واختبار جوجل فعليًا.
- **ترس الإعدادات:** أكبر ولونه ذهبي واضح في أعلى اليمين، مع شرح عند الوقوف عليه.
- **اختبارات الأونلاين:** تشغيل اختبارات الاتصال بالتتابع لتجنب تعارض المنفذ المحلي. كود الـ8 أرقام ما زال مش خدمة غرف عالمية.
- **تسجيل الدخول وجوجل درايف:** تبويب تسجيل دخول جديد، باستخدام متصفح جوجل الرسمي وWindows Credential Manager. تقدر تحفظ مستوى الرسام ومشاريعه المحفوظة يدويًا أو تفعل النسخ التلقائي بعد حفظ المشروع، وتسترجع نسخة بدون الكتابة فوق المشروع المحلي. **تشغيل جوجل لكل المستخدمين محتاج إعداد Google OAuth Client ID للتطبيق في Google Cloud أولًا؛ التفاصيل في GOOGLE_DRIVE_SETUP.md.** الملفات الخارجية للصوت/الصور لسه مش جزء من النسخ السحابية.
- **المنظور:** يبدأ دائمًا مقفول عند تشغيل Stick، ومعاه منظور جديد بنقطتي تلاشي تقدر تتحكم في الأولى والثانية وفي ارتفاع الأفق، من غير ما خطوط الإرشاد تظهر في التصدير.
- **القطارة:** رأس أيقونة القطارة بقى على مكان النقر الفعلي بدل الإزاحة القديمة، واللون بيتاخد من أعلى طبقة ظاهرة.
- **الانضمام أونلاين:** غرف بشكل أوضح، دخول مباشر بعنوان IP/كلمة السر، وكود الغرفة وتعليمات الشبكة داخل قسم اختياري. الاختبار الموجود للهوست والضيف عبر TCP المحلي بيشتغل؛ الاتصال عبر الإنترنت الحقيقي لسه يحتاج ضبط Tailscale وصلاحيات الشبكة.
- **OLD / NEW Timeline:** من ترس التايم لاين تقدر تختار الشكل القديم OLD زي ما هو، أو NEW بتصميم صفوف أوضح، شريط استمرار الرسم بين الفريمات، علامات كي فريم، ومسطرة توقيت. لكل شكل تكبيره الخاص، وطول صف اللير قابل للتعديل.
- **أمان المشاريع:** تغيير الشكل بيغيّر إعدادات العرض فقط، مش البيانات ولا ملفات المشروع. عمليات النسخ والحذف والكي فريم والتشغيل والتكرار شغالة بنفس المحرك.
- **التواصل:** زر «بلّغ عن مشكلة أو اقترح ميزة» في الواجهة الرئيسية بيفتح نافذة قابلة للإغلاق فيها إنستجرام @ablikash و GitHub Issues.
- **ملاحظة مرجعية:** ملف AniHady2D اللي اترفع فيه Setup.exe و README فقط، مش كود أو صورة للتايم لاين. NEW مستوحى من فكرة التايم لاين القديم، مش نسخ مطابق مؤكد.
- **خصوصية الأونلاين:** مفيش بحث عن غرف أو اتصال غير لما المستخدم يختار تشغيل الأونلاين بنفسه. لو Tailscale متصل لكن 0 جهاز ظاهر، البرنامج بيوضح خطوة مشاركة جهاز الهوست أو دعوة أصحابك للشبكة.
- **إصلاح الخروج:** إلغاء انتظار مؤثر الصوت عند قفل البرنامج، مع خيارات احفظ واخرج / خروج بدون حفظ / إلغاء.
- **فرشة G-Pen:** الضربة بقت خط متصل بحساب ضغط القلم بدل دوائر شفافة بتتراكب وتتسبب في البقع السودا. والتصدير يستخدم نفس شكل الضربة. وقفنا التنعيم اللي كان بيخلق نقط زيادة وبيشوّه بعض الزوايا.
- **الفيكتور:** عدد نقاط أقل في الرسم الجديد، تحكم في نسبة تقليلها من كلك يمين، وتحديد Shift أو مربع تحديد لنقل نقاط متعددة.
- **أداة النحت / الصباع:** حركة مرنة لجزء من الخط بالحجم والقوة اللي تختارهم؛ وأسهم X/Y جديدة لتحريك العنصر المحدد أفقيًا أو رأسيًا زي واجهات 3D.
- **التخصيص:** تكبير/تصغير مؤشر الفرشة وأيقونة الأداة بجوار الماوس، وإمكانية إخفاء أو إظهار الدائرة المحيطة (مقفولة افتراضيًا).
- **ملاحظة:** ما زال مطلوب اختبار عملي على جهازين Windows، والفيكتور مش نسخة كاملة من Bézier/Grease Pencil في Blender.
- **تحديث تطوير أدوات الرسم:** راجعنا الأدوات الـ13؛ أصلحنا الدلو مع الحلقات والخطوط المتقاطعة والأشكال المقعّرة، وخفّفنا الفراغات البيضا بين التلوين وحدود الخط. اتضاف قفل فجوات قابل للتعديل وخيار استخدام حدود كل الليرات الظاهرة.
- **الممحاة الكاملة والجزئية:** بقوا بيحسبوا تقاطع الممحاة مع الخط نفسه وبيمسحوا أثناء الحركة السريعة، حتى لو مفيش نقاط خط تحت المؤشر بالضبط.
- **التحديد والقطارة:** الضغط على الخطوط بقى أدق، والقطارة بتجيب ألوان الخط أو التعبئة من أعلى لاير ظاهر.
- **الفرشة والأشكال:** الاحتفاظ بنهاية حركة الفرشة، ومنع المربعات الفاضية لو سحبت بشكل عمودي تمامًا. اختبارات الآلية اتوسعت، ولسه مطلوب تجربة الواجهة الحقيقية.
- **اللعب مع أصحابك على نت مختلف:** دعم Tailscale اختياري من غير بورت فوروارد. برنامج Stick بيكتشف عنوان 100.x الخاص بجهازك والغرف اللي يقدر يوصل لها على شبكة Tailscale، وبيسمح بنسخ عنوان الهوست للانضمام المباشر. يلزم تنزيل Tailscale وتسجيل الدخول والسماح للأصحاب بالوصول للجهاز؛ مش سيرفر غرف عالمي.
- **أداة التحويل:** تكبير مساحة الضغط غير المرئية للمقابض وتثبيتها مع الزوم، مع تفعيل الأربع زوايا للتكبير والتصغير بدل زاوية واحدة. تكبير المقابض المرئية بشكل بسيط.
- **اتصال الأونلاين:** رسائل مساعدة أوضح لخطأ 10061 ولما المستخدم يحاول يستخدم عنوان LAN بعيد عن البيت.
- **مزامنة الأونلاين v3:** أي تعديل في محتوى المشروع بيتبعت من خلال جهاز الهوست: رسم ومسح ودلو وإضافة أو حذف ليرات وفريمات وتعديل النص والتحويلات. إنشاء غرفة بيحوّل جهازك فعليًا لسيرفر TCP للغرفة، وبيظهر توضيح واضح في الواجهة.
- **إصلاح الدلو:** الخط اللي بيقسّم الدايرة بيمنع اللون من الانتقال للنصف التاني، مش بيتملى الشكل كله.
- **تحسين التايم لاين:** حذف أول فريم بيحافظ على الرسم اللي كان ظاهر في الفريم التاني، والفريمات اللي فيها نص أو Symbol بتبان مش فاضية.
- **حدود الأونلاين:** التعديلات المتزامنة على نفس العنصر ممكن تتعارض، والصور والصوت من ملفات خارجية مش بتتنقل، ولسه مطلوب تجربة فعلية بين جهازين Windows.
- أيقونات ملونة بتفاصيل أوضح لكل الأدوات الـ13 ومؤشرات الماوس، وأصوات UI جديدة مولّدة داخل البرنامج بدل الصفارات القديمة.
- تعبئة الدلو: طريقة إضافية بتحدد المساحات المقفولة من عدة خطوط مرسومة بالفرشة، وبترجع المنطقة لحدود فيكتور؛ مع استمرار التعبئة المباشرة للأشكال المغلقة. الحالات المعقدة لسه ليها حدود.
- أونلاين تجريبي على نفس الشبكة: الهوست بيشارك المشروع الحالي، وكود غرفة من 8 أرقام سهل النسخ، والانضمام بالكود محليًا أو IP، وقائمة بأسماء المشاركين ومستوياتهم اللي بيبلغوا بيها وألوانهم. ده مش دليل غرف عالمي، ولا تسجيل دخول Google معتمد؛ مزامنة الأدوات غير الرسم الجديد لسه مش كاملة.
- خانة المنظور: شبكة مربعات أو خطوط متساوية أو منظور بنقطة تلاشي، مع التحكم في المسافة واللون والشفافية والسمك ومكان نقطة التلاشي. الخطوط مساعدة ومش بتدخل في التصدير.
- التايم لاين يقدر يتمدد ويتحفظ ارتفاعه، وقسم الخصائص والليرز في اليمين بينهم فاصل قابل للسحب، وقسم الألوان والتماثل قابل للفتح والقفل.
- رجوع النسخ الاحتياطية بقى يعرض النسخة السابقة منفصلة عن خمس نسخ أقدم منها، من غير تكرار.
- **النسخة الاحتياطية:** خانة جديدة في ترس الإعدادات، تقدر ترجع منها لآخر إصدار أقدم أو تختار من 5 إصدارات قديمة متاحة بتاريخ نشر كل واحد. البرنامج بيتأكد من سلامة ملف EXE ببصمة SHA-256 وبيطلب تأكيد، ومش بيقفل لو في شغل لسه متحفظش.
- **لوحة الليرز:** مساحة أوسع قابلة للتمدد شبه برامج الرسم الاحترافية، ومعاينة فعلية صغيرة من خطوط كل لير، وبحث بالاسم، ومعلومات عن عدد الفريمات والخطوط، وخيارات للكليك يمين، بالإضافة لأزرار القفل والإظهار والنسخ والترتيب والشفافية. الكود مكتوب لـStick من غير نقل كود Krita.
- **أونلاين تجريبي:** اختيار جديد في شاشة البداية للرسم أو الأنيميشن سوا، مع غرف محلية وكلمة سر اختيارية وحد أقصى للأعضاء، وإظهار الغرف على نفس الشبكة أو الدخول بعنوان IP. مزامنة الخطوط الجديدة متاحة؛ التايم لاين والليرز مش متزامنين بالكامل، ومفيش دليل عالمي بالغرف أو تشفير كامل لسه.
- نظام الفنان (Art System) الجديد: شكل أنمي مع اسم تقدر تعدّله، وصورة شخصية من جهازك، وشريط المستوى والرتبة. تفاصيل احتساب التقدم مش ظاهرة.
- تخصيص مساحة الرسم، زووم ناحية الماوس، سهم تدوير حر للتحديد، تحريك التحديد الحر، وقائمة كليك يمين للأدوات، وست أشكال لرأس الفرشة.
- لما المستوى يعلى هتظهر نافذة حماسية في نص الشاشة 10 ثواني، وفيها رسالة من 1000 جملة مختلفة بالمصري أو الإنجليزي، ومعها صوت من 100 تنويعة مختلفة وخفيفة.
- ترس الإعدادات فيه تخصيص اختصارات الأدوات، وتغيير ألوان الواجهة والتايم لاين، وزر يرجّع الألوان الأصلية، وتحكم في أصوات البرنامج.
- قسم اليمين اتقسم لخصائص الأدوات والليرز كل واحد بسكرول مستقل. كليك يمين على اللير لتغيير اسمه أو نسخه أو لصقه أو دبلكيت أو حذفه.
- شكل التايم لاين اتحسّن، وتقدر تمدّه بسحب الحافة، وترس التايم لاين بيفتح إعدادات الأونين سكين من حيث العدد واللون والشفافية لقدام وورا.
- القلب الأحمر على الشمال بيفتح روابط يوتيوب وإنستجرام، والقلب الدهبي اللي في نص الشريط العلوي بيفتح دعم إنستا باي الاختياري.
- التحديد الحر بقى فيه مقبض لتكبير وتصغير الرسم، ونقاط الفيكتور تقدر تحركها، ومؤشر كل أداة بقى ليه شكله وصوته.
- الدلو بقى بيملأ كمان الرسومات الحرة المغلقة كويس، والتلوين بيظهر في الفيديو والصور المصدّرة.
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
- أداة نقاط الفيكتور بقت تسمح بتحديد وتحريك النقط.
- التحديد والتحويل بقى له مقبض مرئي لتغيير حجم الرسم والنص المحدد.
- الليرز مبقتش تتحرك مع سكرول قائمة الخصائص.
- البوكت كان بيتجاهل المساحات اللي مرسومة بالفرشة ومقفولة، وبعض الألوان كانت بتختفي من التصدير.
- احتساب نقاط XP بقى بيحفظ إنجاز كل فريم ولير وما بيكررش المكافأة بمجرد تكرار نفس الخط.
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

### Technical limitations / حدود مهمة
- Bucket fill supports **closed drawn contours**, not full multi-layer pixel flood fill with tolerance; that is planned in Issue #15.
- The 100 level-up sounds are **lightweight synthesized patterns**, not bundled third-party sound files.
- The 1,000 level-up lines are 40 independently authored openings combined with 25 endings in each language.
- This release does not copy GPL code or artwork from Krita or Blender; full brush-engine integration remains Issue #14.
- رتب وترقيات Hunter دي للمتعة والتشجيع بس؛ مالهاش تأثير على ملفات الرسم أو جودة التصدير.
