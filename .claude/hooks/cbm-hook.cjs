const { spawnSync } = require('node:child_process');

const command = process.platform === 'win32' ? 'npx.cmd' : 'npx';

try {
    spawnSync(command, ['-y', 'codebase-memory-mcp@0.10.8', 'hook-augment'], {
        stdio: ['inherit', 'inherit', 'ignore'],
        windowsHide: true
    });
} catch {
    // CBM's generated wrappers are intentionally fail-open.
}

process.exit(0);
