# Stick Studio website — GitHub Pages and Google OAuth
The site's source files are in `docs/` (static HTML/CSS, no cookies, tracker or external JS).

## Activate GitHub Pages (required once)
1. Open **KaniEGY/Stick → Settings → Pages** as a repository administrator.
2. Under **Build and deployment**, select **Deploy from a branch**.
3. Select branch **main** and directory **/docs**; click **Save**.
4. After Pages deployment succeeds, confirm the homepage, Privacy Policy and Terms pages open publicly.
5. Expected *after activation* (not guaranteed live beforehand):
   - Home: https://kaniegy.github.io/Stick/
   - Privacy: https://kaniegy.github.io/Stick/privacy.html
   - Terms: https://kaniegy.github.io/Stick/terms.html

## Google Cloud branding
**Do not submit production verification before domain ownership is proven.**
Google's OAuth branding verification requires a publicly accessible homepage, a same-domain privacy policy and ownership verification. Google Help Center's current domain guidance requires an **owned DNS domain**, verified as a **Domain Property** with a TXT record through Google Search Console by an Owner/Editor of Google Cloud project `stick-studio-515102`.

The free GitHub Pages URL is useful for previewing the site, but relying on `github.io` as a production-verified domain is not recommended: its DNS belongs to GitHub and cannot be verified by this project's developer using the root domain TXT record. For reliable **Production** verification, obtain a domain you control and point it at the same Pages site. Follow GitHub's custom-domain instructions, then verify it using Google's DNS TXT record. Enter the verified HTTPS home, privacy and terms URLs on **Google Auth Platform → Branding** and add the owned root domain in **Authorized domains**.

For now **Audience → Testing** remains appropriate; only explicitly listed test users may complete OAuth. This repository's `drive.appdata` scope is non-sensitive, but branding verification / Google's latest review rules still apply to public apps.

## Privacy accuracy review
The website accurately describes the current Rust implementation at time of writing (18 Nov 2026): OAuth browser + PKCE, `drive.appdata`, Windows Credential Manager, optional backup into private Drive appDataFolder, artist profile and locally saved project data, manual restore and direct online room networking. If features or data handling change, update the privacy policy **in the same release**. Keep support email current.

Official references:
- https://developers.google.com/identity/verification/authentication-policy-compliance
- https://support.google.com/cloud/answer/13804266
- https://support.google.com/cloud/answer/13464321
- https://docs.github.com/en/pages/getting-started-with-github-pages/configuring-a-publishing-source-for-your-github-pages-site
