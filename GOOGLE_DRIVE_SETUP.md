# Google Account + Drive Cloud Backups for Stick Studio

## Status
The public Stick Studio Desktop OAuth Client ID is embedded in official Windows builds, with an optional `STICK_GOOGLE_CLIENT_ID` build-time override. It is a **public application identifier**, not a secret. Earlier installations with an empty or malformed `cloud_google_client_id` recover the official identifier on startup. A valid user-supplied Desktop ID is preserved for developer testing.

The Google OAuth client and Drive API are provisioned for Google Cloud project `stick-studio-515102`, but the app starts in Google's **External / Testing** audience. Test accounts must be added in Google Auth Platform → Audience, and production launch still requires the appropriate Google publication and verification process. Embedding the client ID is not proof that real Windows sign-in, token refresh or cross-device Drive restore has passed manual testing.

## Google Cloud Console setup — required for every public release
1. Open [Google Cloud Console](https://console.cloud.google.com/) and create/select a project for Stick Studio.
2. Enable **Google Drive API** (Google APIs & Services → Library).
3. Configure the Google Auth Platform/OAuth consent screen, app name **Stick Studio**, support information, requested scopes: `openid`, `email`, and `https://www.googleapis.com/auth/drive.appdata`. Add test users while the app is in Testing, then complete Google's production/verification requirements as applicable.
4. Under Clients/Credentials, create an **OAuth Client ID → Desktop app**. This must be a Desktop client; the program uses browser login with an ephemeral `http://127.0.0.1:PORT` callback and PKCE-S256.
5. Official releases embed the public Desktop Client ID. Optionally, GitHub Settings → Secrets and variables → Actions → **Variables** → `STICK_GOOGLE_CLIENT_ID` can override it with another valid public Desktop Client ID for a special build. The workflow passes this variable to Rust; if unset or invalid, the official embedded ID is used. **Never add a Client Secret, password or refresh token to source code, Actions variables or release assets.**
6. Build and test a release. Sign in from **Settings → تسجيل دخول**, grant consent in the default web browser, then press **Back up level and saved projects** or enable **Automatically back up after saving a project**.

Google's official source docs:
- [Desktop OAuth & PKCE](https://developers.google.com/identity/protocols/oauth2/native-app)
- [Drive appDataFolder](https://developers.google.com/workspace/drive/api/guides/appdata)
- [Drive uploads](https://developers.google.com/workspace/drive/api/guides/manage-uploads)

## Behavior
- Permissions requested: Google OpenID email + `drive.appdata` only. The app cannot read the user's arbitrary Drive files.
- A sanitized progress record (username, XP, frames painted, time spent, painted layer count) and a resized, bounded avatar PNG (when valid) are uploaded. The original avatar path is never uploaded. Locally saved project files from the recent-projects list are uploaded separately. Legacy profile `sync_token`, the avatar's absolute path and any secrets are **excluded**.
- The Google refresh token is stored in **Windows Credential Manager** via keyring. The local config stores only the public OAuth client ID, Google email and backup preferences; the profile stores only the account ID. No tokens should enter Git or project files.
- Backup is on-demand by default. With explicit opt-in, project backups run after a successful **local project save**, while artist name, avatar and progress use a separate asynchronous profile-only backup (throttled during ongoing drawing). The upload does not run before sign-in.
- Google Drive stores these items in the **private hidden appDataFolder** for Stick Studio; they do not show in the ordinary My Drive file list, but appear in the Stick cloud backup manager. This is intentional scope minimization.
- The **Browse cloud backups** action lists owned Stick backups and **Restore copy** asks for confirmation. Project restore downloads to a new safe `%LOCALAPPDATA%\StickStudio\CloudRestores\...` file rather than overwriting the current project. A profile restore updates only the artist stats, never Google credentials.
- Backup currently uploads the **project document file only**. If a project depends on external image or audio file paths, those external assets are **not bundled** and must be restored separately. Do not market this as a portable full media backup.
- Google rejects OAuth without the correct configured/approved desktop app, relevant test users, or enabled Drive API.
- Failed uploads or network errors are visible in Settings and do not erase local work. There is no silent bidirectional conflict resolution; restore is explicit.

## Current Google testing requirement
While Google's publishing status is **Testing**, sign-in is available only to accounts explicitly added under Google Auth Platform → Audience → Test users. Refresh tokens for this Drive-enabled external test app typically expire in seven days; the app should re-authenticate when a grant expires. Before public release, switch to production and complete any applicable verification, branding, privacy-policy, and consent requirements. The app requests the non-sensitive `drive.appdata` scope, not unrestricted Google Drive access.

## QA checklist
- [ ] Google Cloud OAuth client ID and consent screen configured; user is authorized as tester or the app is published/verified as needed.
- [ ] Sign in Windows default browser; callback state and PKCE validation; refresh after app restart; sign-out.
- [ ] Back up a real saved project and XP progress; list, download, and reopen a separate copy.
- [ ] Update artist name and picture, earn XP without saving a project, then verify auto profile backup and restore on a second Windows device.
- [ ] Restore cloud profile on a second device and confirm XP, level and stats (but no overwritten avatar or tokens).
- [ ] Verify no token appears in `user_profile.json`, `stick_config.json` or GitHub artifacts.
- [ ] Simulate a revoked grant, network drop, unavailable Windows Credential Manager, and missing OAuth client ID.
- [ ] Verify automatic backup triggers after project save and never before explicit sign-in/opt-in.
- [ ] Review hidden Drive app-data scope semantics and project files with external media attachments.
