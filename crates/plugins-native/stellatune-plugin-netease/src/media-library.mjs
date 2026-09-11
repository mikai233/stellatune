// Converts the service's existing playlist/search endpoints to catalog protocol 1.
export function createMediaLibrary(listItems) {
  const details = new Map();
  function item(row, sourceInstanceId) {
    const reference={sourceInstanceId,kind:row.kind,id:String(row.track_id??row.playlist_id??row.item_id)};
    const result={reference,title:row.title??'',artist:row.artist??null,album:row.album??null,durationMs:row.duration_ms??null,
      trackCount:row.track_count??null,artworkUrl:typeof row.cover==='string'?row.cover:row.cover?.kind==='url'?row.cover.value:null,artistRefs:[]};
    details.set(JSON.stringify(reference),result);
    if(details.size>2000)details.delete(details.keys().next().value);
    return result;
  }
  return async request => {
    if(request.operation==='list-sources')return {protocolVersion:1,instances:[{instanceId:'netease',name:'Netease Cloud Music',resolverCapabilityId:'netease-source',browseKinds:['playlist','track'],searchKinds:['track'],sorts:['default']}]};
    if(request.instanceId!=='netease')throw new Error('Unknown Netease library instance');
    const q=request.input;
    if(request.operation==='get-detail') {
      const cached=details.get(JSON.stringify(q));
      if(cached)return cached;
      if(q.kind==='track')return {reference:q,title:q.id,artistRefs:[]};
      if(q.kind==='playlist'){
        for(let offset=0;;offset+=200){const rows=await listItems({action:'list_playlists',offset,limit:200});const found=rows.find(r=>String(r.playlist_id)===q.id);if(found)return item(found,q.sourceInstanceId);if(rows.length<200)break;}
      }
      throw new Error('Catalog item not found');
    }
    if(request.operation!=='browse')throw new Error('Unsupported media-library operation');
    if(q.sort!=='default')throw new Error('Unsupported catalog sort');
    const identity=JSON.stringify([q.sourceInstanceId,q.kind,q.parent??null,q.search,q.sort,q.limit]);
    const cursor=q.cursor?JSON.parse(q.cursor):{identity,offset:0};
    if(cursor.identity!==identity||!Number.isSafeInteger(cursor.offset)||cursor.offset<0)throw new Error('Invalid catalog cursor');
    let action;
    if(q.kind==='playlist'&&!q.parent&&!q.search)action='list_playlists';
    else if(q.kind==='track'&&q.parent?.kind==='playlist')action='playlist_tracks';
    else if(q.kind==='track'&&q.search&&!q.parent)action='search';
    else if(q.kind==='track'&&!q.parent&&!q.search)return {items:[],nextCursor:null};
    else throw new Error('Unsupported catalog browse');
    const input={action,playlist_id:q.parent?.id,keywords:q.search,offset:cursor.offset,limit:q.limit};
    // Playlist filtering is not exposed as a search capability.
    if(q.parent&&q.search)throw new Error('Search is supported at the library root');
    const rows=await listItems(input);
    const hasMore=rows.length===q.limit&&(await listItems({...input,offset:cursor.offset+rows.length,limit:1})).length>0;
    return {items:rows.map(row=>item(row,q.sourceInstanceId)),nextCursor:hasMore?JSON.stringify({identity,offset:cursor.offset+rows.length}):null};
  };
}
