import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

const root = process.env.LOCAL_AGENTS_ROOT ?? path.resolve(fileURLToPath(new URL('../../', import.meta.url)));

export function managedInference(body, signal, spawnChild = spawn) {
  return new Promise((resolve, reject) => {
    const pythonBin = `${root}/shared/runtime/venv/bin/python`;
    const harnessScript = `${root}/shared/harness/chat_inference.py`;
    const child = spawnChild(pythonBin, [harnessScript], {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: {
        ...process.env,
        LOCAL_AGENTS_ROOT: root,
        LOCAL_AGENTS_LOW_MEMORY: '1',
        LOCAL_AGENTS_TOLERATE_WARNING_GIB: '2.0',
        LOCAL_AGENTS_CONTEXT_SIZE: process.env.LOCAL_AGENTS_CONTEXT_SIZE || '24576',
        LOCAL_AGENTS_MAX_SWAP_GROWTH_GIB: process.env.LOCAL_AGENTS_MAX_SWAP_GROWTH_GIB || '3.0',
      }
    });
    let output = '', errorText = '', stopped = false, killTimer;
    const cancel = () => {
      stopped = true;
      child.kill('SIGTERM');
      killTimer = setTimeout(() => child.kill('SIGKILL'), 20000);
      killTimer.unref();
    };
    signal?.addEventListener('abort', cancel, { once: true });
    if (signal?.aborted) cancel();
    child.stdout.on('data', data => {
      output += data;
      if (output.length > 48 * 1024 * 1024) cancel();
    });
    child.stderr.on('data', data => {
      errorText = (errorText + data).slice(-8192);
    });
    child.stdin.on('error', () => {});
    child.on('error', reject);
    child.on('close', code => {
      clearTimeout(killTimer);
      signal?.removeEventListener('abort', cancel);
      if (stopped) return reject(new Error('Local inference cancelled'));
      if (code !== 0) return reject(new Error(`Local inference exited ${code}: ${errorText}`));
      try {
        const value = JSON.parse(output);
        resolve({
          status: value.status,
          contentType: value.content_type,
          text: Buffer.from(value.body, 'base64').toString('utf8'),
        });
      } catch (error) {
        reject(error);
      }
    });
    child.stdin.end(JSON.stringify(body));
  });
}
