import test from 'node:test';
import assert from 'node:assert/strict';
import { createMediaLibrary } from '../src/media-library.mjs';

test('library protocol paginates more than 200 playlists and scopes cursors',async()=>{
  const rows=Array.from({length:405},(_,i)=>({kind:'playlist',playlist_id:String(i),title:`Playlist ${i}`}));
  const library=createMediaLibrary(async({offset,limit})=>rows.slice(offset,offset+limit));
  const discovered=await library({operation:'list-sources'});assert.equal(discovered.protocolVersion,1);
  let cursor=null;const items=[];
  do {const page=await library({operation:'browse',instanceId:'netease',input:{sourceInstanceId:'123',kind:'playlist',parent:null,search:'',sort:'default',limit:200,cursor}});items.push(...page.items);cursor=page.nextCursor;}while(cursor);
  assert.equal(items.length,405);assert.equal(items[0].reference.id,'0');
  assert.equal((await library({operation:'get-detail',instanceId:'netease',input:items[0].reference})).title,'Playlist 0');
  await assert.rejects(()=>library({operation:'browse',instanceId:'other',input:{}}),/instance/);
});

test('track search is translated and exact string IDs are retained',async()=>{
  let seen;
  const library=createMediaLibrary(async input=>{seen=input;return [{kind:'track',track_id:'001',title:'A',cover:{kind:'url',value:'https://example.test/cover'}}];});
  const page=await library({operation:'browse',instanceId:'netease',input:{sourceInstanceId:'9',kind:'track',parent:null,search:'A',sort:'default',limit:200,cursor:null}});
  assert.equal(seen.action,'search');assert.equal(seen.keywords,'A');assert.equal(page.items[0].reference.id,'001');assert.equal(page.nextCursor,null);assert.equal(page.items[0].artworkUrl,'https://example.test/cover');
});
