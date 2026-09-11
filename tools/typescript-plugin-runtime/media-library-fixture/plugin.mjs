const id='dev.stellatune.test.media-library';
const instanceIds=['server-a','server-b'];
const kinds=['track','album','artist','folder','playlist'];
export default {
  descriptor:{id,apiVersion:2,capabilities:['library','source']},
  async invoke(request){
    const {operation,input:q,instanceId}=request;
    if(operation==='list-sources')return {protocolVersion:1,instances:instanceIds.map(instanceId=>({instanceId,name:instanceId,resolverCapabilityId:'source',browseKinds:kinds,searchKinds:kinds,sorts:['default','title']}))};
    if(!instanceIds.includes(instanceId))throw new Error('Missing or wrong source instance');
    if(operation==='resolve')return {source:{kind:'http',url:`http://127.0.0.1:1/${instanceId}/${encodeURIComponent(q.trackId)}`,headers:{'X-Library':instanceId}},media:{codecHint:'flac'},capabilities:{seekable:true}};
    const item=(kind,key)=>({reference:{sourceInstanceId:q.sourceInstanceId,kind,id:key},title:`${instanceId} ${kind} ${key}`,artist:'Artist',album:'Album',artistRefs:[]});
    if(operation==='get-detail')return item(q.kind,q.id);
    if(operation!=='browse')throw new Error('Unsupported operation');
    if(q.search==='slow')await new Promise(resolve=>setTimeout(resolve,500));
    const fingerprint=JSON.stringify([q.sourceInstanceId,q.kind,q.parent,q.search,q.sort,q.limit]);
    const cursor=q.cursor?JSON.parse(q.cursor):{fingerprint,offset:0};
    if(cursor.fingerprint!==fingerprint)throw new Error('Wrong cursor scope');
    const count=q.kind==='track'?405:205;
    const end=Math.min(count,cursor.offset+q.limit);
    return {items:Array.from({length:end-cursor.offset},(_,n)=>item(q.kind,String(n+cursor.offset+1).padStart(3,'0'))),total:count,nextCursor:end<count?JSON.stringify({fingerprint,offset:end}):null};
  }
};
