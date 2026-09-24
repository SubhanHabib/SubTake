// Export the same MIT-licensed Phosphor icons used by the reference React editor.
import { createRequire } from 'node:module';
const require = createRequire(new URL('../legacy-electron/package.json', import.meta.url));
const React = require('react');
const { renderToStaticMarkup } = require('react-dom/server');
import * as icons from '../legacy-electron/node_modules/@phosphor-icons/react/dist/index.es.js';
import { mkdirSync, writeFileSync, copyFileSync } from 'node:fs';
const out = new URL('../assets/icons/', import.meta.url); mkdirSync(out,{recursive:true});
for (const name of ['FolderOpen','ArrowCounterClockwise','ArrowClockwise','FloppyDisk','Sparkle','Cursor','Camera','ClosedCaptioning','Gear','SpeakerHigh','SlidersHorizontal','Export','CaretDown','CaretLeft','Plus','Play','Pause','SkipBack','SkipForward','Crop','MagnifyingGlassPlus','MagnifyingGlassMinus','MagicWand','Scissors','TextT','Image','ArrowUpRight','Drop','MusicNotes','Record','Stop','Stack','Question','Magnet','ArrowsOutSimple','Selection','LinkSimple','Monitor','Check','FilmStrip','Microphone','MicrophoneSlash','VideoCamera','VideoCameraSlash','Timer','DotsSixVertical','DotsThree','EyeSlash','Trash','X','TextAa','TextAlignLeft','Tag','GridFour','BoundingBox','Flashlight','NumberCircleOne','PlusSquare','Keyboard']) {
  for (const weight of ['regular','fill']) {
    writeFileSync(new URL(`${name}-${weight}.svg`,out),renderToStaticMarkup(React.createElement(icons[name],{size:24,color:'#ffffff',weight})));
  }
}
copyFileSync(new URL('../legacy-electron/node_modules/@phosphor-icons/react/LICENSE',import.meta.url),new URL('LICENSE',out));
