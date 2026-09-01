#!/usr/bin/env bun

import { mkdir, rm, stat } from 'node:fs/promises';
import { dirname, isAbsolute, relative, resolve, sep } from 'node:path';
import { unzipSync } from 'fflate';

const DOWNLOAD_URL = 'https://file.garden/ZzSF-2oH_UI6UVla/streamable.com.zip';
const DUMP_DIRECTORY = resolve(import.meta.dir, '..', 'dump');
const DOWNLOAD_URL_FILE = resolve(DUMP_DIRECTORY, 'download-url.txt');

export function longPath(path, platform = process.platform) {
    if (platform !== 'win32') {
        return path;
    }
    if (path.startsWith('\\\\')) {
        return `\\\\?\\UNC\\${path.slice(2)}`;
    }
    return `\\\\?\\${path}`;
}

async function isDirectory(path) {
    try {
        return (await stat(longPath(path))).isDirectory();
    } catch {
        return false;
    }
}

async function hasCurrentDump() {
    if (!(await isDirectory(DUMP_DIRECTORY))) {
        return false;
    }

    const downloadUrlFile = Bun.file(longPath(DOWNLOAD_URL_FILE));
    return (
        (await downloadUrlFile.exists()) &&
        (await downloadUrlFile.text()).trim() === DOWNLOAD_URL
    );
}

function dumpPath(entryName) {
    const path = resolve(DUMP_DIRECTORY, entryName);
    const pathFromDump = relative(DUMP_DIRECTORY, path);

    if (
        pathFromDump === '..' ||
        pathFromDump.startsWith(`..${sep}`) ||
        isAbsolute(pathFromDump)
    ) {
        throw new Error(`Unsafe path in ZIP: ${entryName}`);
    }

    return path;
}

async function main() {
    if (await hasCurrentDump()) {
        console.log(`Dump is current: ${DOWNLOAD_URL}`);
        return;
    }

    await rm(longPath(DUMP_DIRECTORY), { recursive: true, force: true });

    try {
        console.log(`Downloading ${DOWNLOAD_URL}...`);
        const response = await fetch(DOWNLOAD_URL, {
            headers: {
                Accept: '*/*',
                'User-Agent': 'Mozilla/5.0'
            }
        });

        if (!response.ok) {
            throw new Error(
                `Download failed with HTTP ${response.status} ${response.statusText}`
            );
        }

        const archive = new Uint8Array(await response.arrayBuffer());
        const entries = unzipSync(archive);

        console.log(`Extracting to ${DUMP_DIRECTORY}...`);
        await mkdir(longPath(DUMP_DIRECTORY), { recursive: true });

        for (const [entryName, contents] of Object.entries(entries)) {
            if (entryName.endsWith('/')) {
                continue;
            }

            const path = dumpPath(entryName);
            await mkdir(longPath(dirname(path)), { recursive: true });
            await Bun.write(longPath(path), contents);
        }

        await Bun.write(longPath(DOWNLOAD_URL_FILE), `${DOWNLOAD_URL}\n`);
        console.log('Done.');
    } catch (error) {
        await rm(longPath(DUMP_DIRECTORY), { recursive: true, force: true });
        throw error;
    }
}

if (import.meta.main) {
    await main();
}
