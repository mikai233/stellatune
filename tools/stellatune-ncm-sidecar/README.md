# stellatune-ncm-sidecar

Local sidecar service for `stellatune-plugin-netease`.

## Purpose

This service wraps `NeteaseCloudMusicApi` and exposes a small local HTTP surface for the plugin.

Default bind address:

- `HOST=127.0.0.1`
- `PORT=46321`

## Run

```powershell
cd tools/stellatune-ncm-sidecar
npm install
npm run start
```

## Build standalone exe (Windows)

```powershell
cd tools/stellatune-ncm-sidecar
npm ci
npm run build:exe
```

Output:

- `dist/stellatune-ncm-sidecar.exe`

The generated executable bundles Node.js runtime, so end users do not need a local Node.js installation.

## Endpoints

- `GET /health`
- `GET /v1/search?keywords=...&limit=30&offset=0&level=standard`
- `GET /v1/playlists?limit=100&offset=0`
- `GET /v1/playlist/tracks?playlist_id=...&limit=100&offset=0&level=standard`
- `GET /v1/song/url?song_id=...&level=standard`
- `GET /v1/lyric?song_id=...`
- `GET /v1/auth/login_status`
- `GET /v1/auth/login_refresh`
- `GET /v1/auth/logout`
- `GET /v1/auth/session`
- `GET /v1/auth/qr/key`
- `GET /v1/auth/qr/create?key=...`
- `GET /v1/auth/qr/check?key=...`

Optional cookie can be provided via:

- Header `x-ncm-cookie`, or
- Query `cookie=...`

If neither header nor query cookie is provided, the sidecar falls back to its
in-memory session cookie (updated by login APIs).

## Login cookie persistence

- Session cookie is persisted to local file and restored on next startup.
- Default file path on Windows:
  - `%LOCALAPPDATA%\StellaTune\netease\session-cookie.json`
- Override file path with env:
  - `STELLATUNE_NCM_COOKIE_FILE`

You can inspect persistence status with:

- `GET /v1/auth/session`

## Notes

- This sidecar is intended for local development/testing.
- Keep it bound to loopback (`127.0.0.1`) unless you fully understand the security implications.
