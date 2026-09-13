// Generate parity fixtures from the actual TypeScript reference, not a second hand-written model.
import { build } from '../legacy-electron/node_modules/esbuild/lib/main.js';
import { writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
const root=fileURLToPath(new URL('../legacy-electron/',import.meta.url));
const result=await build({stdin:{contents:`
export { splitCue, mergeCues } from './src/components/video-editor/captionOps.ts';
export { segmentCuesIntoPhrases, endsSentence } from './electron/ipc/captions/segment.ts';
export { parseSilenceIntervals } from './electron/ipc/captions/silence.ts';
export { buildInteractionZoomSuggestions } from './src/components/video-editor/timeline/zoomSuggestionUtils.ts';
export { calculateMp4SourceDimensions, calculateMp4ExportDimensions } from './src/components/video-editor/exportDimensions.ts';
export { parseWhisperJsonCues } from './electron/ipc/captions/parser.ts';
export { buildActiveCaptionLayout } from './src/components/video-editor/captionLayout.ts';
export { computePaddedLayout } from './src/components/video-editor/videoPlayback/layoutUtils.ts';
export { computeRegionStrength } from './src/components/video-editor/videoPlayback/zoomRegionUtils.ts';
export { createSpringState, stepSpringValue, getCursorSpringConfig } from './src/components/video-editor/videoPlayback/motionSmoothing.ts';
`,resolveDir:root,loader:'ts'},bundle:true,write:false,format:'esm',platform:'node',alias:{'@':root+'src'},plugins:[{name:'unused-renderer-types',setup(b){b.onResolve({filter:/^pixi\.js$/},()=>({path:'pixi',namespace:'stub'}));b.onLoad({filter:/.*/,namespace:'stub'},()=>({contents:'export class Application{}; export class Graphics{}; export class Sprite{};'}));}}]});
const reference=await import('data:text/javascript;base64,'+Buffer.from(result.outputFiles[0].text).toString('base64'));
const crop={x:0,y:0,width:1,height:1};const geometries=[];
for(const [width,height,videoWidth,videoHeight]of[[1920,1080,1920,1080],[1080,1920,1920,1080],[1024,1024,1080,1920]]){
 for(const padding of [0,20,{top:200,bottom:20,left:5,right:40,linked:false},{top:10,bottom:25,left:15,right:50,linked:true}]){
  for(const cropRegion of [crop,{x:0.1,y:0.2,width:0.6,height:0.7}]){const input={width,height,videoWidth,videoHeight,padding,cropRegion};geometries.push({input,result:reference.computePaddedLayout(input)});}
 }
}
const springs=[];
for(const smoothing of[0.1,0.5,0.67,1.5,2]){const config=reference.getCursorSpringConfig(smoothing);const state=reference.createSpringState();const values=[];for(let i=0;i<90;i++){const target=i<10?0.2:i<40?0.8:0.35;values.push(reference.stepSpringValue(state,target,1000/60,config));}springs.push({smoothing,config,values});}
const zooms=[];for(const [startMs,endMs]of[[1000,5000],[1000,1500]]){const region={id:'z',startMs,endMs,depth:3,focus:{cx:0.5,cy:0.5}};for(let time=0;time<=6500;time+=125){zooms.push({region,time,result:reference.computeRegionStrength(region,time)});}}
const captions=[];
const cueSets=[
 [{id:'a',startMs:100,endMs:2100,text:'Native captions balance lines and change pages as the words are spoken.'},{id:'b',startMs:2300,endMs:4100,text:'Next phrase\nwith a forced line'}],
 [{id:'a',startMs:0,endMs:1400,text:'one two three',words:[{text:'one',startMs:0,endMs:400},{text:'two',startMs:450,endMs:800,leadingSpace:true},{text:'three',startMs:1000,endMs:1400,leadingSpace:true}]},{id:'b',startMs:2400,endMs:3600,text:'A pause here'}]
];
for(const cues of cueSets)for(const maxRows of [1,2,4])for(const animationStyle of ['none','fade','rise','pop'])for(const timeMs of [0,100,160,400,1050,1360,1490,2090,2200,2380,3510,4100]){
 const settings={maxRows,animationStyle};const result=reference.buildActiveCaptionLayout({cues,timeMs,settings,maxWidthPx:180,measureText:t=>t.length*9});
 captions.push({cues,timeMs,settings,result:result?{page:result.visiblePageIndex,opacity:result.opacity,translate_y:result.translateY,scale:result.scale,lines:result.visibleLines.map(l=>({width:l.width,text:l.words.map(w=>(w.leadingSpace?' ':'')+w.text).join('')}))}:null});
}
const dimensions=[];
for(const [width,height] of [[1920,1080],[1080,1920],[2559,1439],[640,480]]) for(const aspectRatio of ['native','16:9','9:16','1:1','4:3','3:4']) for(const cropRegion of [{width:1,height:1},{width:.63,height:.82}]) for(const quality of ['medium','good','high','source']){
 const base=reference.calculateMp4SourceDimensions(width,height,aspectRatio,cropRegion);
 dimensions.push({width,height,aspectRatio,cropRegion,quality,result:reference.calculateMp4ExportDimensions(base.width,base.height,quality)});
}
const transcriptions=[];
for(const tokens of [
 [{text:' Hello',offsets:{from:0,to:200}},{text:' world',offsets:{from:220,to:420}},{text:'!',offsets:{from:420,to:440}}],
 [{text:' native',offsets:{from:0,to:200}},{text:'ly',offsets:{from:210,to:300}},{text:'  fluent words ',offsets:{from:350,to:600}}],
 [{text:'invalid',offsets:{from:20,to:10}}],[],[{text:'你好',offsets:{from:0,to:100}},{text:'世界',offsets:{from:100,to:200}}]
]){const input={transcription:[{offsets:{from:0,to:1000},text:'fallback text',tokens}]};transcriptions.push({input,result:reference.parseWhisperJsonCues(JSON.stringify(input))});}
const autozooms=[];
for(const clickTimes of [[],[0],[500,1500,6500],[1000,3400,6001,8501],[200,300,400]])for(const reserve of [[],[{start:0,end:2000}]]){
 const cursorTelemetry=[{timeMs:0,cx:0.5,cy:0.5,interactionType:'move'},...clickTimes.map((timeMs,i)=>({timeMs,cx:.2+i*.12,cy:.3+i*.03,interactionType:i===1?'double-click':'click'})),{timeMs:10000,cx:.7,cy:.6,interactionType:'move'}];
 const input={cursorTelemetry,totalMs:10000,defaultDurationMs:2000,reservedSpans:reserve};autozooms.push({input,result:reference.buildInteractionZoomSuggestions(input).suggestions});
}
const segments=[];
const texts=['Hello there. How are you? Fine!', 'Mr. Smith met Dr. Jones. Good morning.', 'Okay. Great. Yes!', '你好。 世界！ Hello 😀. Testing words.', 'one two three four five six seven eight nine ten', 'A word without punctuation'];
for(const text of texts) for(const wordMode of [0,1,2]) for(const gap of [0,400,699,700,1600]) for(const silences of [[],[{startMs:0,endMs:1200},{startMs:3400,endMs:5200}],[{startMs:1000,endMs:30000}]]) {
 const tokens=text.split(' '); const all=tokens.map((text,i)=>({text,startMs:1000+i*(300+gap),endMs:1300+i*(300+gap),...(i?{leadingSpace:true}:{})}));
 const halfway=Math.max(1,Math.floor(all.length/2));const pieces=[all.slice(0,halfway),all.slice(halfway)].filter(a=>a.length);
 const cues=pieces.map((ws,i)=>({id:'old'+i,startMs:ws[0].startMs,endMs:ws.at(-1).endMs,text:ws.map(w=>w.text).join(' '),...(wordMode===1||(wordMode===2&&i===0)?{words:ws}:{})}));
 segments.push({cues,silences,result:reference.segmentCuesIntoPhrases(cues,silences)});
}
const sentenceEnds=['Mr.','U.S.','J.','No.','e.g.','etc.','hello.”','你好。','😀!','word','...','quoted?)'].map(text=>({text,result:reference.endsSentence(text)}));
const captionEdits=[];
for(const timed of [true,false]) for(const time of [0,180,500,1100,1900,2500]) {
 const cues=[{id:'a',startMs:0,endMs:2000,text:'hello native world',...(timed?{words:[{text:'hello',startMs:0,endMs:200},{text:'native',startMs:400,endMs:900,leadingSpace:true},{text:'world',startMs:1200,endMs:2000,leadingSpace:true}]}:{})},{id:'b',startMs:2200,endMs:4000,text:'another phrase'}];
 captionEdits.push({cues,time,split:reference.splitCue(cues,'a',time).map((c,i)=>({...c,id:'canonical-'+i})),merge:reference.mergeCues(cues,'a','b').map((c,i)=>({...c,id:'canonical-'+i}))});
}
await writeFile(new URL('../tests/reference-fixtures.json',import.meta.url),JSON.stringify({geometries,springs,zooms,captions,dimensions,transcriptions,autozooms,segments,sentenceEnds,captionEdits},null,2)+'\n');
console.log(`Generated ${geometries.length} geometry, ${springs.length} spring and ${zooms.length} zoom cases from production source.`);
