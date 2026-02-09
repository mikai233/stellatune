const path = require('path')
const fs = require('fs')
const os = require('os')
const express = require('express')
const ncm = require('NeteaseCloudMusicApi')

const PORT = Number(process.env.PORT || '46321')
const HOST = process.env.HOST || '127.0.0.1'
let sessionCookie = ''
const SESSION_COOKIE_FILE = resolveSessionCookieFilePath()

const app = express()
app.use(express.json({ limit: '1mb' }))

function toInt(raw, fallback) {
  const n = Number.parseInt(String(raw ?? ''), 10)
  if (Number.isNaN(n)) return fallback
  return n
}

function isProcessAlive(pid) {
  if (!Number.isInteger(pid) || pid <= 0) return true
  try {
    process.kill(pid, 0)
    return true
  } catch (err) {
    const code = err && typeof err === 'object' ? err.code : ''
    if (code === 'EPERM' || code === 'EACCES') return true
    return false
  }
}

function startOwnerProcessMonitor() {
  const ownerPid = toInt(process.env.STELLATUNE_NCM_OWNER_PID, 0)
  if (!ownerPid) return
  if (ownerPid === process.pid) {
    console.error('[stellatune-ncm-sidecar] skip owner monitor: owner pid equals self pid')
    return
  }
  console.error(`[stellatune-ncm-sidecar] owner monitor enabled owner_pid=${ownerPid}`)
  const check = () => {
    if (isProcessAlive(ownerPid)) return
    console.error(`[stellatune-ncm-sidecar] owner process exited owner_pid=${ownerPid}, shutting down`)
    process.exit(0)
  }
  check()
  const timer = setInterval(check, 2000)
  if (typeof timer.unref === 'function') {
    timer.unref()
  }
}

function resolveSessionCookieFilePath() {
  const fromEnv = String(process.env.STELLATUNE_NCM_COOKIE_FILE || '').trim()
  if (fromEnv) {
    return path.resolve(fromEnv)
  }

  if (process.platform === 'win32') {
    const base = String(process.env.LOCALAPPDATA || '').trim() || path.join(os.homedir(), 'AppData', 'Local')
    return path.join(base, 'StellaTune', 'netease', 'session-cookie.json')
  }

  const stateHome = String(process.env.XDG_STATE_HOME || '').trim() || path.join(os.homedir(), '.local', 'state')
  return path.join(stateHome, 'stellatune', 'netease', 'session-cookie.json')
}

function hasPersistedSessionCookie() {
  try {
    const stat = fs.statSync(SESSION_COOKIE_FILE)
    return stat.isFile() && stat.size > 0
  } catch (_) {
    return false
  }
}

function persistSessionCookie() {
  try {
    if (!sessionCookie) {
      if (fs.existsSync(SESSION_COOKIE_FILE)) {
        fs.unlinkSync(SESSION_COOKIE_FILE)
      }
      return
    }
    fs.mkdirSync(path.dirname(SESSION_COOKIE_FILE), { recursive: true })
    const payload = JSON.stringify(
      {
        cookie: sessionCookie,
        updated_at: new Date().toISOString(),
      },
      null,
      2
    )
    fs.writeFileSync(SESSION_COOKIE_FILE, payload, { encoding: 'utf8', mode: 0o600 })
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err)
    console.warn('[stellatune-ncm-sidecar] failed to persist cookie:', msg)
  }
}

function setSessionCookie(nextCookie, reason = '') {
  const normalized = normalizeCookieValue(nextCookie)
  const prev = sessionCookie
  sessionCookie = normalized
  persistSessionCookie()
  if (normalized !== prev) {
    const from = prev ? `len=${prev.length}` : 'empty'
    const to = normalized ? `len=${normalized.length}` : 'empty'
    console.log(`[stellatune-ncm-sidecar] session cookie updated (${reason || 'n/a'}): ${from} -> ${to}`)
  }
}

function loadSessionCookieFromDisk() {
  try {
    if (!fs.existsSync(SESSION_COOKIE_FILE)) {
      return
    }
    const raw = fs.readFileSync(SESSION_COOKIE_FILE, 'utf8')
    if (!raw.trim()) {
      return
    }
    const parsed = JSON.parse(raw)
    const cookie = normalizeCookieValue(parsed?.cookie)
    if (!cookie) {
      return
    }
    sessionCookie = cookie
    console.log(`[stellatune-ncm-sidecar] loaded session cookie from disk (len=${cookie.length})`)
  } catch (err) {
    const msg = err instanceof Error ? err.message : String(err)
    console.warn('[stellatune-ncm-sidecar] failed to load cookie from disk:', msg)
  }
}

function getCookie(req) {
  const fromHeader = req.header('x-ncm-cookie')
  if (typeof fromHeader === 'string' && fromHeader.trim().length > 0) {
    return fromHeader.trim()
  }
  const fromQuery = req.query.cookie
  if (typeof fromQuery === 'string' && fromQuery.trim().length > 0) {
    return fromQuery.trim()
  }
  return sessionCookie
}

function normalizeCookieValue(raw) {
  if (Array.isArray(raw)) {
    const merged = raw.map((v) => String(v ?? '').trim()).filter((v) => v.length > 0).join(';')
    return merged.trim()
  }
  if (typeof raw === 'string') {
    return raw.trim()
  }
  return ''
}

function updateSessionCookieFromResult(result) {
  const fromBody = normalizeCookieValue(result?.body?.cookie)
  if (fromBody) {
    setSessionCookie(fromBody, 'result.body.cookie')
    return sessionCookie
  }
  const fromResult = normalizeCookieValue(result?.cookie)
  if (fromResult) {
    setSessionCookie(fromResult, 'result.cookie')
    return sessionCookie
  }
  return sessionCookie
}

function ensureApiCode(result, method, allowedCodes = [200]) {
  if (!result.body || typeof result.body !== 'object') {
    throw new Error(`empty body from method: ${method}`)
  }
  if (!('code' in result.body)) {
    return
  }
  const code = Number(result.body.code)
  if (allowedCodes.includes(code)) {
    return
  }
  const msg = result.body.msg || result.body.message || `code=${code}`
  throw new Error(`${method} failed: ${msg}`)
}

function guessExt(url, fallback = 'mp3') {
  try {
    const u = new URL(url)
    const ext = path.extname(u.pathname || '').replace('.', '').toLowerCase()
    if (ext) return ext
  } catch (_) {
    // ignore parse failures and fall through to fallback
  }
  return fallback
}

function normalizeSongItem(song, level, streamUrl = null) {
  if (!song || !song.id) {
    return null
  }

  const artists = Array.isArray(song.ar)
    ? song.ar.map((v) => v?.name).filter((v) => typeof v === 'string' && v.length > 0).join(' / ')
    : typeof song.artist === 'string'
      ? song.artist
      : ''

  const album = typeof song.al?.name === 'string'
    ? song.al.name
    : typeof song.album === 'string'
      ? song.album
      : ''

  const durationMs = Number.isFinite(song.dt) ? song.dt : Number.isFinite(song.duration) ? song.duration : null
  const extHint = streamUrl ? guessExt(streamUrl, 'mp3') : 'mp3'
  const cover = typeof song.al?.picUrl === 'string'
    ? song.al.picUrl.trim()
    : typeof song.album?.picUrl === 'string'
      ? song.album.picUrl.trim()
      : ''
  const coverRef = cover
    ? { kind: 'url', value: cover, mime: null }
    : null

  return {
    song_id: Number(song.id),
    title: typeof song.name === 'string' ? song.name : `Song ${song.id}`,
    artist: artists || null,
    album: album || null,
    duration_ms: durationMs,
    ext_hint: extHint,
    cover: coverRef,
    stream_url: streamUrl,
    level,
  }
}

function normalizePlaylistItem(playlist, sourceLabel = '') {
  if (!playlist || !playlist.id) {
    return null
  }
  const title = typeof playlist.name === 'string' && playlist.name.trim()
    ? playlist.name.trim()
    : `Playlist ${playlist.id}`
  const trackCount = Number.isFinite(playlist.trackCount) ? Number(playlist.trackCount) : null
  const cover = typeof playlist.coverImgUrl === 'string' ? playlist.coverImgUrl.trim() : ''
  const coverRef = cover
    ? { kind: 'url', value: cover, mime: null }
    : null
  return {
    kind: 'playlist',
    source_id: 'netease',
    source_label: sourceLabel || 'Netease Cloud Music',
    playlist_id: String(playlist.id),
    title,
    track_count: trackCount,
    cover: coverRef,
    playlist_ref: {
      playlist_id: Number(playlist.id),
    },
  }
}

async function runNcm(method, payload) {
  const fn = ncm[method]
  if (typeof fn !== 'function') {
    throw new Error(`NeteaseCloudMusicApi method not found: ${method}`)
  }
  const result = await fn(payload)
  if (!result || typeof result !== 'object') {
    throw new Error(`invalid response from method: ${method}`)
  }
  return result
}

async function resolveCurrentUserId(cookie) {
  const status = await runNcm('login_status', { cookie })
  ensureApiCode(status, 'login_status')
  updateSessionCookieFromResult(status)
  const account = status?.body?.data?.account
  const profile = status?.body?.data?.profile

  const accountId = Number(account?.id)
  if (Number.isFinite(accountId) && accountId > 0) {
    return accountId
  }
  const profileUserId = Number(profile?.userId)
  if (Number.isFinite(profileUserId) && profileUserId > 0) {
    return profileUserId
  }
  console.error('[stellatune-ncm-sidecar] resolveCurrentUserId failed: account/profile uid unavailable')
  return null
}

app.get('/health', (_req, res) => {
  res.json({
    ok: true,
    service: 'stellatune-ncm-sidecar',
    has_cookie: sessionCookie.length > 0,
    cookie_file: SESSION_COOKIE_FILE,
  })
})

app.get('/v1/auth/session', (_req, res) => {
  res.json({
    has_cookie: sessionCookie.length > 0,
    cookie_length: sessionCookie.length,
    persisted: hasPersistedSessionCookie(),
    cookie_file: SESSION_COOKIE_FILE,
  })
})

app.get('/v1/admin/shutdown', (_req, res) => {
  console.error('[stellatune-ncm-sidecar] shutdown requested')
  res.json({ ok: true, shutting_down: true })
  const timer = setTimeout(() => process.exit(0), 60)
  if (typeof timer.unref === 'function') {
    timer.unref()
  }
})

app.get('/v1/search', async (req, res, next) => {
  try {
    const keywords = String(req.query.keywords || '').trim()
    if (!keywords) {
      return res.json({ items: [] })
    }

    const limit = Math.max(1, Math.min(toInt(req.query.limit, 30), 200))
    const offset = Math.max(0, toInt(req.query.offset, 0))
    const level = String(req.query.level || 'standard').toLowerCase()
    const cookie = getCookie(req)

    const result = await runNcm('search', {
      keywords,
      type: 1,
      limit,
      offset,
      cookie,
    })
    ensureApiCode(result, 'search')

    const songs = Array.isArray(result.body?.result?.songs) ? result.body.result.songs : []
    const items = songs
      .map((song) => normalizeSongItem(song, level))
      .filter((v) => v !== null)

    res.json({ items })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/playlist/tracks', async (req, res, next) => {
  try {
    const playlistId = toInt(req.query.playlist_id, 0)
    if (!playlistId) {
      return res.status(400).json({ error: 'playlist_id is required' })
    }

    const limit = Math.max(1, Math.min(toInt(req.query.limit, 100), 1000))
    const offset = Math.max(0, toInt(req.query.offset, 0))
    const level = String(req.query.level || 'standard').toLowerCase()
    const cookie = getCookie(req)

    const result = await runNcm('playlist_track_all', {
      id: String(playlistId),
      limit,
      offset,
      cookie,
    })
    ensureApiCode(result, 'playlist_track_all')

    const songs = Array.isArray(result.body?.songs) ? result.body.songs : []
    const items = songs
      .map((song) => normalizeSongItem(song, level))
      .filter((v) => v !== null)

    res.json({ items })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/playlists', async (req, res, next) => {
  try {
    const limit = Math.max(1, Math.min(toInt(req.query.limit, 100), 1000))
    const offset = Math.max(0, toInt(req.query.offset, 0))
    const cookie = getCookie(req)
    const sourceLabel = String(req.query.source_label || 'Netease Cloud Music')
    console.error(
      `[stellatune-ncm-sidecar] /v1/playlists request limit=${limit} offset=${offset} has_cookie=${cookie.length > 0}`
    )

    let uid = toInt(req.query.uid, 0)
    if (!uid) {
      const resolved = await resolveCurrentUserId(cookie)
      uid = Number.isFinite(resolved) ? Number(resolved) : 0
    }
    if (!uid) {
      console.error('[stellatune-ncm-sidecar] /v1/playlists reject: uid unavailable')
      return res.status(401).json({ error: 'user not logged in or uid unavailable' })
    }
    console.error(`[stellatune-ncm-sidecar] /v1/playlists using uid=${uid}`)

    const result = await runNcm('user_playlist', {
      uid: String(uid),
      limit,
      offset,
      cookie,
    })
    ensureApiCode(result, 'user_playlist')
    updateSessionCookieFromResult(result)

    const raw = Array.isArray(result.body?.playlist) ? result.body.playlist : []
    const items = raw.map((playlist) => normalizePlaylistItem(playlist, sourceLabel)).filter((v) => v !== null)
    console.error(`[stellatune-ncm-sidecar] /v1/playlists response count=${items.length}`)
    res.json({ items })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/song/url', async (req, res, next) => {
  try {
    const songId = toInt(req.query.song_id, 0)
    if (!songId) {
      return res.status(400).json({ error: 'song_id is required' })
    }

    const level = String(req.query.level || 'standard').toLowerCase()
    const cookie = getCookie(req)

    const result = await runNcm('song_url_v1', {
      id: String(songId),
      level,
      cookie,
    })
    ensureApiCode(result, 'song_url_v1')

    const row = Array.isArray(result.body?.data) ? result.body.data[0] : null
    if (!row || typeof row.url !== 'string' || row.url.length === 0) {
      return res.status(404).json({ error: 'song url unavailable' })
    }

    res.json({
      url: row.url,
      ext_hint: guessExt(row.url, typeof row.type === 'string' ? row.type : 'mp3'),
      level,
      bitrate: Number.isFinite(row.br) ? row.br : null,
    })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/lyric', async (req, res, next) => {
  try {
    const songId = toInt(req.query.song_id, 0)
    if (!songId) {
      return res.status(400).json({ error: 'song_id is required' })
    }

    const cookie = getCookie(req)
    const result = await runNcm('lyric', {
      id: String(songId),
      cookie,
    })
    ensureApiCode(result, 'lyric')
    res.json({ body: result.body })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/auth/login_status', async (req, res, next) => {
  try {
    const result = await runNcm('login_status', { cookie: getCookie(req) })
    ensureApiCode(result, 'login_status')
    const cookie = updateSessionCookieFromResult(result)
    res.json({ body: result.body, cookie: cookie || null })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/auth/login_refresh', async (req, res, next) => {
  try {
    const result = await runNcm('login_refresh', { cookie: getCookie(req) })
    ensureApiCode(result, 'login_refresh')
    const cookie = updateSessionCookieFromResult(result)
    res.json({ body: result.body, cookie: cookie || null })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/auth/logout', async (req, res, next) => {
  try {
    const result = await runNcm('logout', { cookie: getCookie(req) })
    ensureApiCode(result, 'logout')
    setSessionCookie('', 'logout')
    res.json({ body: result.body, cookie: null })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/auth/qr/key', async (_req, res, next) => {
  try {
    const result = await runNcm('login_qr_key', {})
    ensureApiCode(result, 'login_qr_key')
    const cookie = updateSessionCookieFromResult(result)
    res.json({ body: result.body, cookie: cookie || null })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/auth/qr/create', async (req, res, next) => {
  try {
    const key = String(req.query.key || '').trim()
    if (!key) {
      return res.status(400).json({ error: 'key is required' })
    }

    const result = await runNcm('login_qr_create', {
      key,
      qrimg: String(req.query.qrimg || 'true'),
      cookie: getCookie(req),
    })
    ensureApiCode(result, 'login_qr_create')
    const cookie = updateSessionCookieFromResult(result)
    res.json({ body: result.body, cookie: cookie || null })
  } catch (err) {
    next(err)
  }
})

app.get('/v1/auth/qr/check', async (req, res, next) => {
  try {
    const key = String(req.query.key || '').trim()
    if (!key) {
      return res.status(400).json({ error: 'key is required' })
    }

    const result = await runNcm('login_qr_check', {
      key,
      cookie: getCookie(req),
    })
    ensureApiCode(result, 'login_qr_check', [800, 801, 802, 803])
    const cookie = updateSessionCookieFromResult(result)
    res.json({ body: result.body, cookie: cookie || null })
  } catch (err) {
    next(err)
  }
})

app.use((err, _req, res, _next) => {
  const message = err instanceof Error ? err.message : String(err)
  console.error('[stellatune-ncm-sidecar] error:', message)
  res.status(502).json({ error: message })
})

loadSessionCookieFromDisk()
startOwnerProcessMonitor()

app.listen(PORT, HOST, () => {
  console.log(`[stellatune-ncm-sidecar] listening on http://${HOST}:${PORT}`)
})
