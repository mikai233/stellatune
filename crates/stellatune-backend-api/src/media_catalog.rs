//! Provider-neutral browsing. Playback identities remain owned by PlayerCatalog.
use crate::{
    player_service::{
        identity::{
            ProviderId, ProviderTrackIdentityInput, ProviderTrackKeyInput, SourceInstanceId,
        },
        metadata::{TrackCover, TrackCoverKind, TrackPresentation},
        service::PlayerService,
        source::SourceResolverSpec,
    },
    runtime::TypeScriptSourceResolver,
};
use anyhow::{Result, anyhow, bail};
use serde_json::{Value, json};
use sqlx::Row;
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::Arc,
};
use stellatune_library::catalog::{
    CatalogItem, CatalogPage, CatalogQuery, CatalogSort, LibrarySource, LocalCatalog, MediaKind,
    MediaRef, PluginLibraries,
};
use stellatune_plugins::typescript::{TypeScriptRuntime, manifest::TypeScriptCapabilityKind};
use tokio_util::sync::CancellationToken;

pub struct MediaCatalogService {
    local: LocalCatalog,
    player: Arc<PlayerService>,
    runtime: Arc<TypeScriptRuntime>,
    collections: tokio::sync::Mutex<CollectionJobs>,
}

#[derive(Default)]
struct CollectionJobs {
    running: HashMap<String, CancellationToken>,
    cancelled_before_start: VecDeque<String>,
}

/// Plugin Web UI commands use the same source identity as native catalog browsing.
pub async fn ensure_catalog_provider_track(
    player: &PlayerService,
    runtime: Arc<TypeScriptRuntime>,
    plugin_id: &str,
    catalog_capability: &str,
    resolver_capability: &str,
    instance_id: &str,
    key: &str,
) -> Result<crate::player_service::identity::TrackId> {
    let registrations = runtime.registered_plugins().await;
    let plugin = registrations
        .iter()
        .find(|p| p.manifest.id == plugin_id)
        .ok_or_else(|| anyhow!("plugin unavailable"))?;
    if !plugin
        .manifest
        .capabilities
        .iter()
        .any(|c| c.id == catalog_capability && c.kind == TypeScriptCapabilityKind::MediaLibrary)
        || !plugin.manifest.capabilities.iter().any(|c| {
            c.id == resolver_capability && c.kind == TypeScriptCapabilityKind::SourceResolver
        })
    {
        bail!("invalid library or resolver capability");
    }
    let response = runtime
        .invoke(
            plugin_id,
            catalog_capability,
            None,
            "list-sources",
            json!({"protocolVersion":1}),
            None,
        )
        .await?;
    let libraries: PluginLibraries = serde_json::from_value(response.value)?;
    if libraries.protocol_version != 1 {
        bail!("unsupported media-library protocol");
    }
    let instance = libraries
        .instances
        .into_iter()
        .find(|i| i.instance_id == instance_id && i.resolver_capability_id == resolver_capability)
        .ok_or_else(|| anyhow!("unknown library instance"))?;
    if let Some(error) = instance.error {
        bail!("{error}");
    }
    let provider = ProviderId::new(serde_json::to_string(&(
        plugin_id,
        catalog_capability,
        instance_id,
    ))?)?;
    let spec = SourceResolverSpec::new(
        plugin_id,
        resolver_capability,
        json!({"instanceId":instance_id,"catalogCapabilityId":catalog_capability}).to_string(),
    )?;
    let source = player
        .ensure_plugin_source(
            provider,
            spec,
            Arc::new(TypeScriptSourceResolver::new(
                runtime,
                plugin_id,
                resolver_capability,
            )),
        )
        .await?;
    Ok(player
        .ensure_track(ProviderTrackIdentityInput {
            source_instance_id: source.get(),
            provider_key: ProviderTrackKeyInput::Text(key.into()),
        })
        .await?)
}

impl MediaCatalogService {
    pub fn new(
        local: LocalCatalog,
        player: Arc<PlayerService>,
        runtime: Arc<TypeScriptRuntime>,
    ) -> Self {
        Self {
            local,
            player,
            runtime,
            collections: Default::default(),
        }
    }

    pub async fn list_sources(&self) -> Result<Vec<LibrarySource>> {
        let local_id = self
            .player
            .catalog
            .ensure_local_source()
            .await?
            .get()
            .to_string();
        let mut sources = vec![LibrarySource {
            id: local_id,
            name: "Local library".into(),
            local: true,
            available: true,
            error: None,
            browse_kinds: vec![
                MediaKind::Track,
                MediaKind::Album,
                MediaKind::Artist,
                MediaKind::Folder,
                MediaKind::Playlist,
            ],
            search_kinds: vec![
                MediaKind::Track,
                MediaKind::Album,
                MediaKind::Artist,
                MediaKind::Folder,
                MediaKind::Playlist,
            ],
            sorts: vec![CatalogSort::Default, CatalogSort::Title],
        }];
        let mut saved: HashMap<String, LibrarySource> = HashMap::new();
        for row in sqlx::query("SELECT descriptor_json FROM media_library_sources")
            .fetch_all(&self.player.catalog.pool)
            .await?
        {
            let mut source: LibrarySource =
                serde_json::from_str(&row.get::<String, _>("descriptor_json"))?;
            source.available = false;
            source.error = Some("Source unavailable; enable or update its plugin".into());
            saved.insert(source.id.clone(), source);
        }
        for plugin in self.runtime.registered_plugins().await {
            for cap in plugin
                .manifest
                .capabilities
                .iter()
                .filter(|c| c.kind == TypeScriptCapabilityKind::MediaLibrary)
            {
                let result = async {
                    let response = self.runtime.invoke(&plugin.manifest.id,&cap.id,None,"list-sources",json!({"protocolVersion":1}),None).await?;
                    let discovered: PluginLibraries = serde_json::from_value(response.value)?;
                    if discovered.protocol_version!=1 { bail!("unsupported media-library protocol; update plugin"); }
                    let mut seen=HashSet::new();
                    for instance in discovered.instances {
                        if instance.error.is_none() && (!instance.sorts.contains(&CatalogSort::Default) ||
                            (!instance.browse_kinds.contains(&MediaKind::Track) && instance.browse_kinds.iter().any(|kind|matches!(kind,MediaKind::Album|MediaKind::Artist|MediaKind::Playlist)))) {
                            bail!("media-library requires default ordering and track browsing for music collections");
                        }
                        if instance.instance_id.is_empty() || instance.name.trim().is_empty() || !seen.insert(instance.instance_id.clone()) { bail!("invalid or duplicate library instance"); }
                        if !plugin.manifest.capabilities.iter().any(|c| c.id==instance.resolver_capability_id && c.kind==TypeScriptCapabilityKind::SourceResolver) { bail!("library instance references an unknown resolver"); }
                        let provider = ProviderId::new(serde_json::to_string(&(&plugin.manifest.id,&cap.id,&instance.instance_id))?)?;
                        let config=json!({"instanceId":instance.instance_id,"catalogCapabilityId":cap.id});
                        let spec=SourceResolverSpec::new(&plugin.manifest.id,&instance.resolver_capability_id,config.to_string())?;
                        let id=self.player.ensure_plugin_source(provider,spec,Arc::new(TypeScriptSourceResolver::new(self.runtime.clone(),&plugin.manifest.id,&instance.resolver_capability_id))).await?;
                        let source=LibrarySource {id:id.get().to_string(),name:instance.name,local:false,available:instance.error.is_none(),error:instance.error,browse_kinds:instance.browse_kinds,search_kinds:instance.search_kinds,sorts:instance.sorts};
                        sqlx::query("INSERT INTO media_library_sources(source_id,descriptor_json) VALUES(?,?) ON CONFLICT(source_id) DO UPDATE SET descriptor_json=excluded.descriptor_json")
                            .bind(id.get() as i64).bind(serde_json::to_string(&source)?).execute(&self.player.catalog.pool).await?;
                        saved.insert(source.id.clone(),source);
                    }
                    Ok::<_,anyhow::Error>(())
                }.await;
                if let Err(error) = result {
                    tracing::warn!(plugin=%plugin.manifest.id,capability=%cap.id,%error,"catalog discovery failed");
                    // Keep failures visible even before an instance has ever been discovered.
                    sources.push(LibrarySource {
                        id: format!("unavailable:{}:{}", plugin.manifest.id, cap.id),
                        name: format!("{} / {}", plugin.manifest.name, cap.display_name),
                        local: false,
                        available: false,
                        error: Some(error.to_string()),
                        browse_kinds: vec![],
                        search_kinds: vec![],
                        sorts: vec![],
                    });
                }
            }
            if plugin
                .manifest
                .capabilities
                .iter()
                .any(|c| c.kind == TypeScriptCapabilityKind::NetworkControl)
                && !plugin
                    .manifest
                    .capabilities
                    .iter()
                    .any(|c| c.kind == TypeScriptCapabilityKind::MediaLibrary)
            {
                if plugin.manifest.capabilities.iter().any(|c|c.kind==TypeScriptCapabilityKind::SourceResolver) {
                    sources.push(LibrarySource {id:format!("upgrade:{}",plugin.manifest.id),name:plugin.manifest.name.clone(),local:false,available:false,error:Some("Update this source plugin to media-library protocol 1 for native browsing".into()),browse_kinds:vec![],search_kinds:vec![],sorts:vec![]});
                }
            }
        }
        let mut remote: Vec<_> = saved.into_values().collect();
        remote.sort_by(|a, b| a.name.cmp(&b.name).then(a.id.cmp(&b.id)));
        sources.extend(remote);
        Ok(sources)
    }

    async fn source(&self, id: &str) -> Result<crate::player_service::source::SourceCatalogEntry> {
        let entry = self
            .player
            .catalog
            .source(SourceInstanceId::new(id.parse()?)?)
            .await?;
        if entry.tombstoned {
            bail!("source unavailable");
        }
        Ok(entry)
    }

    async fn invoke(&self, source_id: &str, operation: &str, input: Value) -> Result<Value> {
        let source = self.source(source_id).await?;
        let spec = source
            .resolver
            .ok_or_else(|| anyhow!("not a plugin source"))?;
        let config: Value = serde_json::from_str(&spec.config_json)?;
        let capability = config["catalogCapabilityId"]
            .as_str()
            .ok_or_else(|| anyhow!("source requires an updated media-library plugin"))?;
        let instance = config["instanceId"]
            .as_str()
            .ok_or_else(|| anyhow!("source instance missing"))?;
        Ok(self
            .runtime
            .invoke(
                &spec.plugin_id,
                capability,
                Some(instance.to_owned()),
                operation,
                input,
                None,
            )
            .await?
            .value)
    }

    pub async fn browse(&self, q: CatalogQuery) -> Result<CatalogPage> {
        q.validate()?;
        let entry = self.source(&q.source_instance_id).await?;
        if entry.resolver.is_none() {
            return self.local.browse(&q).await;
        }
        let raw: String = sqlx::query_scalar(
            "SELECT descriptor_json FROM media_library_sources WHERE source_id=?",
        )
        .bind(entry.id.get() as i64)
        .fetch_one(&self.player.catalog.pool)
        .await?;
        let descriptor: LibrarySource = serde_json::from_str(&raw)?;
        let kinds = if q.search.trim().is_empty() {
            &descriptor.browse_kinds
        } else {
            &descriptor.search_kinds
        };
        if !kinds.contains(&q.kind) || !descriptor.sorts.contains(&q.sort) {
            bail!("unsupported catalog operation");
        }
        let mut page: CatalogPage = serde_json::from_value(
            self.invoke(&q.source_instance_id, "browse", serde_json::to_value(&q)?)
                .await?,
        )?;
        if page.items.len() > q.limit as usize
            || page
                .next_cursor
                .as_ref()
                .is_some_and(|c| c.is_empty() || Some(c) == q.cursor.as_ref() || c.len() > 16384)
        {
            bail!("invalid catalog pagination response");
        }
        let mut seen = HashSet::new();
        for item in &mut page.items {
            self.validate_item(item, &q.source_instance_id)?;
            if item.reference.kind != q.kind || !seen.insert(item.reference.clone()) {
                bail!("invalid or duplicate catalog item");
            }
        }
        Ok(page)
    }

    fn validate_item(&self, item: &mut CatalogItem, source: &str) -> Result<()> {
        if item.reference.id.is_empty() || item.reference.id.len() > 512 {
            bail!("invalid media ID");
        }
        // Only the host assigns source identity and local-file access.
        item.reference.source_instance_id = source.into();
        item.local_path = None;
        item.local_track_id = None;
        if let Some(reference) = &mut item.album_ref {
            reference.source_instance_id = source.into();
            if reference.kind != MediaKind::Album {
                bail!("invalid album reference");
            }
        }
        for reference in &mut item.artist_refs {
            reference.source_instance_id = source.into();
            if reference.kind != MediaKind::Artist {
                bail!("invalid artist reference");
            }
        }
        if let Some(url) = &item.artwork_url {
            let parsed = url::Url::parse(url)?;
            if !matches!(parsed.scheme(), "http" | "https") {
                bail!("artwork must be HTTP");
            }
        }
        Ok(())
    }

    pub async fn detail(&self, reference: MediaRef) -> Result<CatalogItem> {
        let source = self.source(&reference.source_instance_id).await?;
        if source.resolver.is_none() {
            return self.local.detail(&reference).await;
        }
        let mut item: CatalogItem = serde_json::from_value(
            self.invoke(
                &reference.source_instance_id,
                "get-detail",
                serde_json::to_value(&reference)?,
            )
            .await?,
        )?;
        self.validate_item(&mut item, &reference.source_instance_id)?;
        if item.reference != reference {
            bail!("detail response references another item");
        }
        Ok(item)
    }

    pub async fn prepare_tracks(&self, items: Vec<CatalogItem>) -> Result<Vec<u64>> {
        let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
        for (index, item) in items.iter().enumerate() {
            if item.reference.kind != MediaKind::Track {
                bail!("only tracks can enter the queue");
            }
            groups
                .entry(item.reference.source_instance_id.clone())
                .or_default()
                .push(index);
        }
        let mut result = vec![0; items.len()];
        let mut metadata = Vec::new();
        for (source_id, positions) in groups {
            let source = self.source(&source_id).await?;
            let local = source.resolver.is_none();
            let ids = if local {
                let keys = positions
                    .iter()
                    .map(|i| items[*i].reference.id.parse::<i64>())
                    .collect::<Result<Vec<_>, _>>()?;
                self.player.ensure_local_tracks(&keys).await?
            } else {
                let keys = positions
                    .iter()
                    .map(|i| items[*i].reference.id.clone())
                    .collect::<Vec<_>>();
                self.player
                    .catalog
                    .ensure_provider_text_tracks(source.id, &keys)
                    .await?
            };
            for (index, id) in positions.into_iter().zip(ids) {
                result[index] = id.get();
                let item = &items[index];
                if !local {
                    metadata.push((
                        id,
                        TrackPresentation {
                            title: Some(item.title.clone()),
                            artist: item.artist.clone(),
                            album: item.album.clone(),
                            duration_ms: item.duration_ms.and_then(|v| u64::try_from(v).ok()),
                            cover: item.artwork_url.clone().map(|value| TrackCover {
                                kind: TrackCoverKind::Url,
                                value,
                                mime: None,
                            }),
                        },
                    ));
                }
            }
        }
        self.player.store_track_presentations(&metadata).await?;
        Ok(result)
    }

    pub async fn collect_tracks(
        &self,
        mut q: CatalogQuery,
        request_id: String,
    ) -> Result<Vec<CatalogItem>> {
        if q.kind != MediaKind::Track || request_id.is_empty() {
            bail!("invalid track collection request");
        }
        let token = CancellationToken::new();
        {
            let mut jobs = self.collections.lock().await;
            if jobs.running.contains_key(&request_id) {
                bail!("collection already running");
            }
            if let Some(index) = jobs
                .cancelled_before_start
                .iter()
                .position(|id| id == &request_id)
            {
                jobs.cancelled_before_start.remove(index);
                bail!("collection cancelled");
            }
            jobs.running.insert(request_id.clone(), token.clone());
        }
        q.cursor = None;
        q.limit = 200;
        let work = async {
            let mut items = Vec::new();
            let mut cursors = HashSet::new();
            let mut refs = HashSet::new();
            loop {
                let page = self.browse(q.clone()).await?;
                for item in page.items {
                    if !refs.insert(item.reference.clone()) {
                        bail!("collection changed during pagination; refresh and retry");
                    }
                    items.push(item);
                }
                q.cursor = page.next_cursor;
                if items.len() > 100_000 {
                    bail!("collection exceeds 100,000 tracks; select a smaller collection");
                }
                if q.cursor.is_none() {
                    break;
                }
                if !cursors.insert(q.cursor.clone()) {
                    bail!("catalog cursor cycle");
                }
            }
            Ok(items)
        };
        let result = tokio::select! {result=work=>result, _=token.cancelled()=>Err(anyhow!("collection cancelled"))};
        self.collections.lock().await.running.remove(&request_id);
        result
    }
    pub async fn cancel_collection(&self, request_id: String) {
        let mut jobs = self.collections.lock().await;
        if let Some(token) = jobs.running.get(&request_id) {
            token.cancel();
        } else {
            jobs.cancelled_before_start.push_back(request_id);
            while jobs.cancelled_before_start.len() > 256 {
                jobs.cancelled_before_start.pop_front();
            }
        }
    }
}
