# Google Account + Drive Cloud Backups for Stick Studio

## Status
The Windows app has a Google sign-in/Drive backup **implementation**, but production sign-in **requires a Google Cloud OAuth client ID configured by the app owner**. A build without this value must not claim that Google sign-in is live. No Google credentials are shipped or requested from end users.

## Google Cloud Console setup — required for every public release
1. Open [Google Cloud Console](https://console.cloud.google.com/) and create/select a project for Stick Studio.
2. Enable **Google Drive API** (Google APIs & Services → Library).
3. Configure the Google Auth Platform/OAuth consent screen, app name **Stick Studio**, support information, requested scopes: `openid`, `email`, and `https://www.googleapis.com/auth/drive.appdata`. Add test users while the app is in Testing, then complete Google's production/verification requirements as applicable.
4. Under Clients/Credentials, create an **OAuth Client ID → Desktop app**. This must be a Desktop client; the program uses browser login with an ephemeral `http://127.0.0.1:PORT` callback and PKCE-S256.
5. On GitHub repository Settings → Secrets and variables → Actions → **Variables**, add `STICK_GOOGLE_CLIENT_ID` with the **public Desktop Client ID**, ending with `.apps.googleusercontent.com`. The `.github/workflows/build.yml` file passes this variable into the Rust compilation environment for the official EXE. **Do not add a Google Client Secret, password or refresh token.**
6. Build and test a release. Sign in from **Settings → تسجيل دخول**, grant consent in the default web browser, then press **Back up level and saved projects** or enable **Automatically back up after saving a project**.

Google's official source docs:
- [Desktop OAuth & PKCE](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Drive appDataFolder](https://developers.google.com/workspace/drive/api/guides/appdata)
- [Drive uploads](https://developers.google.com/workspace/drive/api/guides/manage-uploads)

## Behavior
- Permissions requested: Google OpenID email + `drive.appdata` only. The app cannot read the user's arbitrary Drive files.
- Only a sanitized progress record (username, XP, frames painted, time spent and painted layer count) and locally saved project files from the recent-projects list are uploaded. Legacy profile `sync_token`, the avatar's absolute path and any secrets are **excluded**.
- The Google refresh token is stored in **Windows Credential Manager** via keyring. The local config stores only the public OAuth client ID, Google email and backup preferences; the profile stores only the account ID. No tokens should enter Git or project files.
- Backup is on-demand by default. Automatic Drive backup runs asynchronously after a successful **local project save**, only if the user explicitly enables it after signing in.
- Google Drive stores these items in the **private hidden appDataFolder** for Stick Studio; they do not show in the ordinary My Drive file list, but appear in the Stick cloud backup manager. This is intentional scope minimization.
- The **Browse cloud backups** action lists owned Stick backups and **Restore copy** asks for confirmation. Project restore downloads to a new safe `%LOCALAPPDATA%\StickStudio\CloudRestores\...` file rather than overwriting the current project. A profile restore updates only the artist stats, never Google credentials.
- Backup currently uploads the **project document file only**. If a project depends on external image or audio file paths, those external assets are **not bundled** and must be restored separately. Do not market this as a portable full media backup.
- Google rejects OAuth without the correct configured/approved desktop app, relevant test users, or enabled Drive API.
- Failed uploads or network errors are visible in Settings and do not erase local work. There is no silent bidirectional conflict resolution; restore is explicit.

## QA checklist
- [ ] Google Cloud OAuth client ID and consent screen configured; user is authorized as tester or the app is published/verified as needed.
- [ ] Sign in Windows default browser; callback state and PKCE validation; refresh after app restart; sign-out.
- [ ] Back up a real saved project and XP progress; list, download, and reopen a separate copy.
- [ ] Restore cloud profile on a second device and confirm XP, level and stats (but no overwritten avatar or tokens).
- [ ] Verify no token appears in `user_profile.json`, `stick_config.json` or GitHub artifacts.
- [ ] Simulate a revoked grant, network drop, unavailable Windows Credential Manager, and missing OAuth client ID.
- [ ] Verify automatic backup triggers after project save and never before explicit sign-in/opt-in.
- [ ] Review hidden Drive app-data scope semantics and project files with external media attachments.
