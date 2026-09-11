import 'dart:async';

import 'package:stellatune/app/diagnostics/diagnostics_service.dart';

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'catalog_bridge.dart';
import 'catalog_track_sort.dart';

final catalogControllerProvider =
    NotifierProvider<CatalogController, CatalogState>(CatalogController.new);

final catalogVisibleItemsProvider = Provider((ref) {
  final (items, kind, sort) = ref.watch(
    catalogControllerProvider.select((s) => (s.items, s.kind, s.trackSort)),
  );
  return kind == MediaKind.track ? sort.apply(items) : items;
});

class CatalogState {
  const CatalogState({
    this.sources = const [],
    this.sourceId,
    this.kind = MediaKind.track,
    this.parents = const [],
    this.search = '',
    this.sort = CatalogSort.default_,
    this.trackSort = const CatalogTrackSort(),
    this.items = const [],
    this.nextCursor,
    this.total,
    this.loading = false,
    this.loadingMore = false,
    this.error,
    this.sourcesError,
  });
  final List<LibrarySource> sources;
  final String? sourceId;
  final MediaKind kind;
  final List<CatalogItem> parents;
  final String search;
  final CatalogSort sort;
  final CatalogTrackSort trackSort;
  final List<CatalogItem> items;
  final String? nextCursor;
  final int? total;
  final bool loading, loadingMore;
  final String? error, sourcesError;
  LibrarySource? get source =>
      sources.where((s) => s.id == sourceId).firstOrNull;
  CatalogItem? get parent => parents.lastOrNull;
  CatalogQuery get query => CatalogQuery(
    sourceInstanceId: sourceId!,
    kind: kind,
    parent: parent?.reference,
    search: search,
    sort: sort,
    limit: 200,
  );
  String get locationKey => jsonEncode([
    sourceId,
    kind.name,
    parent?.reference.kind.name,
    parent?.reference.id,
    search,
    sort.name,
  ]);
  CatalogState copyWith({
    List<LibrarySource>? sources,
    String? sourceId,
    MediaKind? kind,
    List<CatalogItem>? parents,
    String? search,
    CatalogSort? sort,
    CatalogTrackSort? trackSort,
    List<CatalogItem>? items,
    Object? nextCursor = _keep,
    Object? total = _keep,
    bool? loading,
    bool? loadingMore,
    Object? error = _keep,
    Object? sourcesError = _keep,
  }) => CatalogState(
    sources: sources ?? this.sources,
    sourceId: sourceId ?? this.sourceId,
    kind: kind ?? this.kind,
    parents: parents ?? this.parents,
    search: search ?? this.search,
    sort: sort ?? this.sort,
    trackSort: trackSort ?? this.trackSort,
    items: items ?? this.items,
    nextCursor: identical(nextCursor, _keep)
        ? this.nextCursor
        : nextCursor as String?,
    total: identical(total, _keep) ? this.total : total as int?,
    loading: loading ?? this.loading,
    loadingMore: loadingMore ?? this.loadingMore,
    error: identical(error, _keep) ? this.error : error as String?,
    sourcesError: identical(sourcesError, _keep)
        ? this.sourcesError
        : sourcesError as String?,
  );
}

const _keep = Object();

class CatalogController extends Notifier<CatalogState> {
  late CatalogBridge _bridge;
  int _generation = 0, _sourcesGeneration = 0;
  final _cache = <String, CatalogState>{};
  final _sourceLocations = <String, CatalogState>{};
  final _sourceHistories = <String, List<CatalogState>>{};
  final _history = <CatalogState>[];
  @override
  CatalogState build() {
    _bridge = ref.watch(catalogBridgeProvider);
    _generation++;
    _sourcesGeneration++;
    _cache.clear();
    _sourceLocations.clear();
    _sourceHistories.clear();
    _history.clear();
    ref.onDispose(() {
      _generation++;
      _sourcesGeneration++;
    });
    return const CatalogState();
  }

  Future<void> refreshSources({bool preserveItems = false}) async {
    final generation = ++_sourcesGeneration;
    try {
      final sources = await _bridge.sources();
      if (!ref.mounted || generation != _sourcesGeneration) return;
      final selected =
          sources.where((s) => s.id == state.sourceId).firstOrNull ??
          sources.where((s) => s.available).firstOrNull ??
          sources.firstOrNull;
      state = state.copyWith(sources: sources, sourcesError: null);
      if (selected == null) return;
      if (state.sourceId != selected.id) {
        await selectSource(selected.id);
      } else {
        await refresh(preserveItems: preserveItems);
      }
    } catch (e) {
      if (ref.mounted && generation == _sourcesGeneration) {
        state = state.copyWith(
          sourcesError: DiagnosticsService.instance.failureMessage(
            e,
            operation: 'catalog',
          ),
        );
      }
    }
  }

  Future<void> selectSource(String id) async {
    if (state.sourceId != null) {
      _sourceLocations[state.sourceId!] = state;
      _sourceHistories[state.sourceId!] = List.of(_history);
    }
    final source = state.sources.firstWhere((s) => s.id == id);
    final restored = _sourceLocations[id];
    _history.clear();
    _history.addAll(_sourceHistories[id] ?? const []);
    _navigate(
      restored?.copyWith(sources: state.sources) ??
          CatalogState(
            sources: state.sources,
            sourceId: id,
            kind: source.browseKinds.contains(MediaKind.track)
                ? MediaKind.track
                : source.browseKinds.firstOrNull ?? MediaKind.track,
            search: state.sourceId == null ? state.search : '',
          ),
    );
    await _load();
  }

  Future<void> selectKind(MediaKind kind) async {
    _history.clear();
    _navigate(
      state.copyWith(
        kind: kind,
        parents: [],
        search: '',
        sort: CatalogSort.default_,
      ),
    );
    await _load();
  }

  Future<void> open(CatalogItem item, {bool replacePath = false}) async {
    if (item.reference.kind == MediaKind.track) return;
    _history.add(state);
    final kind = switch (item.reference.kind) {
      MediaKind.artist =>
        state.source?.browseKinds.contains(MediaKind.album) == true
            ? MediaKind.album
            : MediaKind.track,
      MediaKind.folder => MediaKind.folder,
      _ => MediaKind.track,
    };
    _navigate(
      state.copyWith(
        kind: kind,
        parents: replacePath ? [item] : [...state.parents, item],
        search: '',
        sort: CatalogSort.default_,
      ),
    );
    final generation = _generation;
    unawaited(
      _bridge
          .detail(item.reference)
          .then((detail) {
            if (ref.mounted && generation == _generation) {
              state = state.copyWith(
                parents: [
                  ...state.parents.take(state.parents.length - 1),
                  detail,
                ],
              );
            }
          })
          .catchError((Object e) {
            if (ref.mounted && generation == _generation) {
              state = state.copyWith(
                error: DiagnosticsService.instance.failureMessage(
                  e,
                  operation: 'catalog',
                ),
              );
            }
          }),
    );
    await _load();
  }

  Future<void> childKind(MediaKind kind) async {
    _navigate(state.copyWith(kind: kind));
    await _load();
  }

  /// A tree selection replaces the path; sibling folders are not nested.
  Future<void> selectFolder(List<CatalogItem> path) async {
    if (path.isEmpty ||
        path.any(
          (item) =>
              item.reference.kind != MediaKind.folder ||
              item.reference.sourceInstanceId != state.sourceId,
        )) {
      return;
    }
    _history.clear();
    _navigate(
      state.copyWith(
        parents: List.unmodifiable(path),
        kind: MediaKind.track,
        search: '',
        sort: CatalogSort.default_,
      ),
    );
    await _load();
  }

  Future<void> back() async {
    if (_history.isEmpty) {
      await selectKind(state.parent?.reference.kind ?? state.kind);
      return;
    }
    final previous = _history.removeLast().copyWith(sources: state.sources);
    _navigate(previous);
    await _load();
  }

  Future<void> setSearch(String search) async {
    final kinds = state.source?.searchKinds ?? const <MediaKind>[];
    final kind = search.trim().isNotEmpty && !kinds.contains(state.kind)
        ? kinds.firstOrNull ?? state.kind
        : state.kind;
    _history.clear();
    _navigate(state.copyWith(search: search, kind: kind, parents: []));
    await _load();
  }

  Future<void> setSort(CatalogSort sort) async {
    _navigate(state.copyWith(sort: sort));
    await _load();
  }

  void sortTracks(CatalogTrackColumn column) {
    state = state.copyWith(trackSort: state.trackSort.toggle(column));
  }

  void _navigate(CatalogState next) {
    if (state.sourceId != null && !state.loading && state.error == null) {
      _cacheState();
    }
    _generation++;
    final cached = _cache[next.locationKey];
    state = next.copyWith(
      items: cached?.items ?? [],
      nextCursor: cached?.nextCursor,
      total: cached?.total,
      loading: false,
      loadingMore: false,
      error: null,
    );
  }

  void _cacheState() {
    // An interrupted/incomplete list must resume loading when revisited.
    if (state.loading ||
        state.loadingMore ||
        state.nextCursor != null ||
        state.error != null) {
      return;
    }
    _cache.remove(state.locationKey);
    _cache[state.locationKey] = state;
    while (_cache.length > 64) {
      _cache.remove(_cache.keys.first);
    }
  }

  Future<void> refresh({bool preserveItems = false}) async {
    _generation++;
    _cache.clear();
    await _load(force: true, preserveItems: preserveItems);
  }

  Future<void> _load({bool force = false, bool preserveItems = false}) async {
    if (state.sourceId == null) return;
    if (!force && _cache.containsKey(state.locationKey)) return;
    await _fetch(false, preserveItems: preserveItems);
  }

  Future<void> retryLoading() async {
    if (state.loading || state.loadingMore || state.error == null) return;
    await _fetch(state.nextCursor != null);
  }

  Future<void> _fetch(bool resume, {bool preserveItems = false}) async {
    final source = state.source;
    if (source == null) return;
    if (!source.available) {
      state = state.copyWith(
        error: source.error ?? 'Source unavailable',
        loading: false,
        loadingMore: false,
      );
      return;
    }
    final generation = _generation;
    final q = state.query;
    final keepVisible =
        preserveItems &&
        !resume &&
        state.items.isNotEmpty &&
        state.nextCursor == null &&
        !state.loadingMore;
    final previousItems = state.items;
    final previousTotal = state.total;
    final items = resume ? [...state.items] : <CatalogItem>[];
    final seen = items.map((item) => item.reference).toSet();
    final cursors = <String>{};
    var cursor = resume ? state.nextCursor : null;
    int? total = resume ? state.total : null;
    var published = false;
    final publishTimer = Stopwatch()..start();
    state = state.copyWith(
      loading: !resume,
      loadingMore: resume,
      error: null,
      items: resume || keepVisible ? state.items : [],
      nextCursor: cursor,
      total: keepVisible ? previousTotal : total,
    );
    try {
      do {
        if (cursor != null) cursors.add(cursor);
        final page = await _bridge.browse(
          CatalogQuery(
            sourceInstanceId: q.sourceInstanceId,
            kind: q.kind,
            parent: q.parent,
            search: q.search,
            sort: q.sort,
            limit: q.limit,
            cursor: cursor,
          ),
        );
        if (!ref.mounted || generation != _generation) return;
        final pageRefs = <MediaRef>{};
        if (page.items.any(
              (item) =>
                  seen.contains(item.reference) ||
                  !pageRefs.add(item.reference),
            ) ||
            (page.nextCursor != null && cursors.contains(page.nextCursor))) {
          throw StateError('Collection changed; refresh to continue');
        }
        items.addAll(page.items);
        seen.addAll(pageRefs);
        cursor = page.nextCursor;
        total = page.total?.toInt() ?? total;
        // Show the first results promptly, then throttle publication of large lists.
        // The view remains virtualized; pagination is only a transport detail.
        if ((!keepVisible || cursor == null) &&
            (!published ||
                cursor == null ||
                publishTimer.elapsedMilliseconds >= 100)) {
          state = state.copyWith(
            items: List.unmodifiable(items),
            nextCursor: cursor,
            total: cursor == null ? items.length : total,
            loading: false,
            loadingMore: cursor != null,
          );
          published = true;
          publishTimer.reset();
        }
      } while (cursor != null && ref.mounted && generation == _generation);
      if (ref.mounted && generation == _generation) _cacheState();
    } catch (e) {
      if (ref.mounted && generation == _generation) {
        state = state.copyWith(
          items: keepVisible ? previousItems : List.unmodifiable(items),
          // A refresh continuation belongs to the replacement, not the visible snapshot.
          nextCursor: keepVisible ? null : cursor,
          total: keepVisible ? previousTotal : total,
          loading: false,
          loadingMore: false,
          error: DiagnosticsService.instance.failureMessage(
            e,
            operation: 'catalog',
          ),
        );
      }
    }
  }
}
