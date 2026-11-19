import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import fs from 'node:fs';

const run=promisify(execFile);
export const APPLESCRIPT=`on run argv
set outputPath to item 1 of argv
set pngData to the clipboard as «class PNGf»
set outputFile to open for access POSIX file outputPath with write permission
set eof outputFile to 0
write pngData to outputFile
close access outputFile
end run`;

export async function importClipboardImage(destination,platform=process.platform) {
  if(platform!=='darwin')throw new Error('Clipboard image import currently requires macOS');
  await run('/usr/bin/osascript',['-e',APPLESCRIPT,destination],{timeout:15000,maxBuffer:1024*1024});
  fs.chmodSync(destination,0o600);
  return destination;
}
