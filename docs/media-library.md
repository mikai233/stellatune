# Native media library browsing

The desktop Library page browses the complete local index or an explicitly
selected plugin instance. Albums, artists and folders are backend queries;
loading a page never registers its songs in the playback catalog. Playlists
use the same identity and paging contract through the existing Playlists page.

## Boundaries

Local scanning enables Symphonia metadata readers (`all-meta`) explicitly;
audio codec support alone does not enable ID3/APE tags. Rescans merge available
tags into existing records, preserving known fields when inspection returns no
value. An empty artist list preserves prior artist data unless a new primary
artist is available. Removing a tag from the file does not explicitly clear its
previous database value under this merge policy.

- `stellatune-library::catalog` defines `MediaRef`, `CatalogItem`, `CatalogQuery`,
  `CatalogPage` and source descriptors, and implements local SQL queries.
- `stellatune-backend-api::media_catalog::MediaCatalogService` discovers plugin
  instances, validates results, dispatches queries, registers playback identities
  in batches and collects a full track collection with cancellation.
- Flutter uses generated typed FFI through `CatalogBridge`. Native views never
  interpret plugin-specific JSON, server IDs, URLs or authentication settings.
- The audio engine still receives a file/HTTP source plan. Browsing, server
  identity and credentials do not enter the audio engine.

`MediaRef` is `(sourceInstanceId, kind, id)`. Source instance IDs are persistent
host IDs encoded as decimal strings. Entity IDs are opaque strings: `001` and
`1` are different IDs. The host maps `(plugin ID, library capability ID, plugin
instance ID)` to the existing `SourceInstanceId`. Its resolver configuration
stores the instance and catalog capability identifiers; credentials stay in
the plugin's data directory. Temporary URLs are resolved again for playback.

`CatalogItem` is the common summary/detail envelope: its typed reference
identifies the entity, with title, artist, album, duration, optional count and
artwork, plus album/artist references. Local-only path and cover-track fields
are assigned by the host. Plugin responses cannot grant local-file access.

## Plugin protocol 1

Declare an explicit `media-library` capability in Manifest v2. The transport
remains `stellatune-capability-rpc/1`; `network-control` is no longer discovered
as a native catalog. Types are exported by the TypeScript SDK.

| Operation | Input | Output |
| --- | --- | --- |
| `list-sources` | `{ protocolVersion: 1 }`, no instance | `PluginLibraries` |
| `browse` | `CatalogQuery`, with RPC `instanceId` | `CatalogPage` |
| `get-detail` | `MediaRef`, with RPC `instanceId` | `CatalogItem` |
| `resolve` on the declared source resolver | `{ trackId: string }`, with the same RPC `instanceId` | `SourcePlan` |

Each discovered instance names its exact resolver capability and declares its
supported browse kinds, search kinds and sorts. Return a per-instance `error`
when that instance cannot currently be used. A disabled or missing plugin does
not erase source identity or queued tracks. Refresh discovers restored sources.
Every available instance supports default ordering; title sorting is optional.
An instance advertising albums, artists or playlists also supports their tracks.

`browse` supports a parent reference: album → tracks, artist → albums/tracks,
folder → direct subfolders/tracks, playlist → tracks. Empty search means
browsing; nonempty search is an actual provider query. The desktop title-bar
search stays within the active kind and parent (including album/artist/folder
details). Its hint names that scope. Switching top-level tabs carries the query
to the new kind when searchable; unsupported search is disabled without falling
back to a different kind. Clearing search retains the parent and back stack.
Unsupported browse categories are hidden. Cross-kind global search is deferred.

Return `nextCursor: null` only at the end, including when the last page happens
to be full. A cursor is opaque to the host and UI and must be bound to its
instance, query, sort and page size. Page sizes are 1–200. Do not repeat a cursor
or entity within a page. Return an error when a cursor is stale. The host rejects
malformed references, oversized pages, duplicate identities and cursor cycles.

For plugin Web UI playback, include `catalogCapabilityId` in `ProviderTrack`,
use `providerId` for the plugin instance ID and keep `providerKey` as a string.
The host discovers that explicit instance and uses the same catalog identity as
native browsing. The existing player control endpoints and SDK remain in use.

The Netease package implements track search and playlist browsing through this
contract, with its existing login/configuration UI. It does not advertise album,
artist or folder APIs. `tools/typescript-plugin-runtime/media-library-fixture`
provides two server instances, all entity kinds and more than one page per kind
for contract tests; it is not bundled as a user library.

## Local indexing and desktop behavior

Migration 0008 adds disc/track numbers and structured artist values. A derived SQL
view and a trigger-maintained artist association index keep relationships current
with scans and file watching, without a second copy of track metadata. Indexes
support album identity, artist membership and directory lookup. Existing data is upgraded additively; run a forced metadata
scan to populate the new fields. Startup never clears the database on failure.

Albums group by trimmed album title and album artist, falling back to the track
artist. Untitled albums group by directory. Repeated artist tags and names
separated by supported delimiters are indexed as individual artists. Album order is
disc number, track number, title and ID, with unknown numbers last. Playlist
order follows its stored sort index. Counts come from the complete index.
Other local song lists default to file modification time, newest first, with
numeric track ID descending as a stable tie-breaker across pages. Clicking `#`
restores this order; explicit column sorting still overrides it. The library
does not currently record a separate added-at timestamp.

Local cursors include a query identity and database revision. A scan, file change
or playlist edit invalidates old cursors instead of silently skipping tracks.
The desktop debounces local change events and offers refresh/retry after errors.
Navigation and scroll state are scoped by source, kind, parent, search and sort;
late responses cannot overwrite a different selection.

The desktop presentation retains the original underlined category tabs and
uses the title-bar search as its single search entry. Songs use a compact table;
albums use square artwork grids and artists use circular artwork grids, with
independent grid/list preferences and scroll positions. Counts show provider
totals when supplied, or explicitly identify the number received while loading.
The controller automatically drains every page for the current collection,
including local songs, albums, artists and search results. There is no manual
load-more action or scroll threshold: pagination is an internal transport detail.
The complete collection remains accessible in one virtualized list. Initial
results appear promptly; subsequent updates are throttled to avoid rebuilding
on every batch. Switching locations stops further requests from the old query.
Failures preserve received results and offer retry; incomplete collections are
never cached as complete. Once loading finishes, the count reflects all entries.
Artist artwork uses the available provider artwork or a representative local
cover; missing images have a placeholder rather than fabricated portraits.

Folders use a split view: a lazily expanded directory tree on the left
and the selected folder's direct songs on the right. Each pane scrolls
independently. Breadcrumbs select ancestors without accumulating sibling paths.
The tree collapses and expands over 240 ms with an ease-in/out curve while the
song pane takes the released width. Collapsing retains tree expansion, selection
and both scroll positions without refetching songs. Reduced-motion settings
disable this animation. Tree state is scoped to each source for the session;
refresh invalidates outstanding tree requests and reloads expanded branches.
Each expanded branch automatically loads all of its child-directory pages;
collapsed branches are not recursively scanned just to display the tree.

Song tables sort through the title, artist, album and duration column headers;
repeated clicks toggle direction, and `#` restores source order. There is no
separate song-sort dropdown. Sorting uses the full loaded collection, retains
source order for ties and puts missing metadata last in either direction.
The sorted view is cached until the data or sort changes; scrolling does not
re-sort it. Newly arriving pages join the same ordering automatically.

Returning to the library and refreshing an existing view keep the visible
snapshot and count while fetching a replacement. The replacement is published
only after all pages arrive, so intermediate pages do not shrink the scroll
extent or move the table header. Refresh failure retains the old snapshot and
does not attach a replacement-page cursor to it. First loads still stream pages.

Artist and album text in song rows opens the referenced native detail page
without starting playback. Links use source-scoped `artistRefs` / `albumRef`;
absent references remain plain text. Multi-artist rows show independent inline
links separated by plain slashes; each name opens its artist directly. Local
names are available immediately from the local provider's JSON name IDs;
remote names resolve through shared lookups for visible rows, without guessing
names from opaque IDs or display credits. Each link has its own hover/focus
feedback and ellipsis within the artist column. Related
navigation starts a new entity path while retaining the previous view in history,
so Back returns to the originating collection or folder.

Drag the separators between song-table headers to resize adjacent columns.
Separators appear only on hover, keyboard focus or while dragging; the resting
header stays free of vertical rules. Numbers and the playing indicator share
the same centered alignment under `#`.
Headers and visible rows share the same geometry and minimum widths, including
space for the largest track number. Double-click a separator to restore all
default widths; focused separators support arrow keys and Home to reset.
Column preferences are shared across song views, saved locally on drag release,
and restored after restart. Text columns adapt proportionally to window width,
while secondary columns shrink and fade across a width range instead of being
removed at a hard breakpoint. Gaps shrink with their columns, and headers and
rows follow the folder pane's available width on every animation frame without
separate per-row animation controllers. Responsive changes do not overwrite the
saved column preferences.

Play all collects every track page in the backend before touching the queue,
then applies the current column ordering before preparing the queue.
Outside search, clicking a song queues the complete current collection in display
order and starts at that song's reference-matched index. A fully loaded song
view is reused directly; an incomplete view collects all pages first. A missing
selected reference aborts preparation instead of playing a different track.
The explicit Add to queue action appends only the selected song.
Searching, clearing a query, sorting and switching tabs never mutate the playing
queue. Clicking a search result prepares only that song: if it is already queued,
play its existing occurrence (prefer the current occurrence when duplicated);
otherwise splice it after the current song and play it, retaining the other
tracks and source label. An empty queue starts with that one song. Only the
explicit "Play all search results" action adopts the entire filtered collection.
When the source and complete ordered track references match the existing
catalog queue, selecting another song reuses that queue and selects its index.
Outside search, starting playback from a different collection or ordering
prepares a replacement queue. Catalog
references and known presentation metadata survive the native queue projection.
The playback bar displays the pending song's metadata together, keeps its seek
bar mounted, and shows loading feedback only after a 300 ms wait. Small local
library covers share the playback bar's decoded image cache; pending image
loads retain the previous frame until the new cover is ready.
It can be cancelled, rejects duplicate/cyclic pages, and fails explicitly above
100,000 tracks. Failure or cancellation during collection leaves the queue
unchanged. Folder playback includes direct songs only. Source references are
retained with the queue's presentation label.

## Validation and next extensions

Local artist tags are split on `/`, `／`, `、`, commas, semicolons, pipes
(including full-width forms), NUL and line breaks. Names are trimmed and
deduplicated in credit order, including when a file provides multiple Artist
tags. `&`, `and` and `feat.` remain literal. This is a text-tag heuristic;
punctuation inside an actual artist name can be ambiguous. Original display
credits and album identities are preserved; separate normalized performer and
album-artist names drive browsing and performer links. Existing libraries are
backfilled once in batches on database open, without rescanning audio files.
Remote catalog artist references remain provider-owned and are not rewritten.

Rust tests cover complete SQL paging, compilations, ordering, unknown tags,
Unicode folders, query/revision-bound cursors, two instances with the same IDs,
plugin disable/re-enable, persistence, exact resolver instance context and
cancellation. Flutter tests cover request races, page retry, navigation, native
category browsing and cancelling play-all preparation. Netease tests cover the
installed bundle, typed library mapping and more than 200 playlists.

Windows is the primary desktop build target. macOS/Linux share the UI but need
their own platform builds. The mobile local-library UI remains unchanged.

Later work can add account/server management, cross-source search, writable
remote playlists, unified favorites, background synchronization and offline
downloads using these source/entity references. No remote write or sync methods
are advertised by this version. Native browsing does not require a remote
library to be copied into the local scan database.
