// ==UserScript==
// @name         Streamable feature-flag toggles
// @namespace    streamable-rs
// @version      1.0.0
// @description  Locally override feature flags returned to the Streamable dashboard.
// @match        https://streamable.com/*
// @run-at       document-start
// @grant        none
// ==/UserScript==

(() => {
    'use strict';

    const API_URL = 'https://api-f.streamable.com/api/v1/me/flags';
    const API_HOST = 'api-f.streamable.com';
    const API_PATH = '/api/v1/me/flags';
    const STORAGE_KEY = 'streamable-feature-flag-overrides-v1';
    const CONTROLLER_NAME = 'StreamableFeatureFlags';
    const CHANGED_EVENT = 'streamable-feature-flags:changed';

    if (window[CONTROLLER_NAME]) return;

    const originalFetch = window.fetch.bind(window);
    let serverFlags = Object.create(null);
    let overrides = loadOverrides();
    let host;
    let shadow;
    let panel;
    let rows;
    let status;
    let search;

    function own(object, key) {
        return Object.prototype.hasOwnProperty.call(object, key);
    }

    function copyRecord(value) {
        const copy = Object.create(null);
        if (!value || typeof value !== 'object' || Array.isArray(value))
            return copy;
        for (const [key, entry] of Object.entries(value)) copy[key] = entry;
        return copy;
    }

    function loadOverrides() {
        try {
            return copyRecord(
                JSON.parse(localStorage.getItem(STORAGE_KEY) || '{}')
            );
        } catch {
            return Object.create(null);
        }
    }

    function saveOverrides() {
        localStorage.setItem(STORAGE_KEY, JSON.stringify(overrides));
    }

    function flagsRequest(input) {
        try {
            const rawUrl = input instanceof Request ? input.url : String(input);
            const url = new URL(rawUrl, location.href);
            return url.hostname === API_HOST && url.pathname === API_PATH;
        } catch {
            return false;
        }
    }

    function effectiveFlags() {
        const result = Object.create(null);
        for (const [name, serverValue] of Object.entries(serverFlags)) {
            result[name] = own(overrides, name) ? overrides[name] : serverValue;
        }
        return result;
    }

    function emitChanged() {
        window.dispatchEvent(
            new CustomEvent(CHANGED_EVENT, {
                detail: {
                    flags: effectiveFlags(),
                    overrides: copyRecord(overrides),
                    serverFlags: copyRecord(serverFlags)
                }
            })
        );
    }

    function capture(flags) {
        serverFlags = copyRecord(flags);
        renderRows();
        emitChanged();
    }

    function patchedEnvelope(envelope) {
        if (
            !envelope ||
            typeof envelope !== 'object' ||
            Array.isArray(envelope)
        )
            return null;
        if (
            !envelope.flags ||
            typeof envelope.flags !== 'object' ||
            Array.isArray(envelope.flags)
        ) {
            return null;
        }
        capture(envelope.flags);
        return { ...envelope, flags: effectiveFlags() };
    }

    window.fetch = async function streamableFeatureFlagFetch(input, init) {
        const response = await originalFetch(input, init);
        if (!flagsRequest(input)) return response;

        try {
            const envelope = await response.clone().json();
            const patched = patchedEnvelope(envelope);
            if (!patched) return response;

            const headers = new Headers(response.headers);
            headers.delete('content-length');
            headers.delete('content-encoding');
            return new Response(JSON.stringify(patched), {
                headers,
                status: response.status,
                statusText: response.statusText
            });
        } catch (error) {
            console.warn(
                '[StreamableFeatureFlags] Could not inspect flag response',
                error
            );
            return response;
        }
    };

    async function refresh() {
        setStatus('Loading server flags…');
        const response = await originalFetch(API_URL, {
            credentials: 'include'
        });
        if (!response.ok)
            throw new Error(`Flag request failed with HTTP ${response.status}`);
        const envelope = await response.json();
        if (!envelope?.flags || typeof envelope.flags !== 'object') {
            throw new Error('Flag response did not contain a flags object');
        }
        capture(envelope.flags);
        setStatus(`${Object.keys(serverFlags).length} server flags loaded`);
        return list();
    }

    function list() {
        return Object.keys(serverFlags)
            .sort((left, right) => left.localeCompare(right))
            .map((name) => ({
                name,
                server: serverFlags[name],
                effective: own(overrides, name)
                    ? overrides[name]
                    : serverFlags[name],
                overridden: own(overrides, name)
            }));
    }

    function assertKnownFlag(name) {
        if (!own(serverFlags, name)) {
            throw new Error(
                `Unknown flag: ${name}. Call refresh() to load the API list first.`
            );
        }
    }

    function set(name, value) {
        assertKnownFlag(name);
        overrides[name] = value;
        saveOverrides();
        renderRows();
        emitChanged();
        setStatus('Override saved; reload to apply it to the dashboard');
        return value;
    }

    function unset(name) {
        delete overrides[name];
        saveOverrides();
        renderRows();
        emitChanged();
        setStatus('Override removed; reload to apply the server value');
    }

    function clear() {
        overrides = Object.create(null);
        localStorage.removeItem(STORAGE_KEY);
        renderRows();
        emitChanged();
        setStatus('All overrides removed; reload to apply server values');
    }

    function formatValue(value) {
        const encoded = JSON.stringify(value);
        return encoded === undefined ? String(value) : encoded;
    }

    function parseValue(raw) {
        try {
            return JSON.parse(raw);
        } catch {
            throw new Error(
                'Enter a valid JSON value, such as true, false, 2, or "text"'
            );
        }
    }

    function setStatus(message, isError = false) {
        if (!status) return;
        status.textContent = message;
        status.dataset.error = String(isError);
    }

    function makeButton(label, action, className = '') {
        const button = document.createElement('button');
        button.type = 'button';
        button.textContent = label;
        button.className = className;
        button.addEventListener('click', action);
        return button;
    }

    function makeBooleanEditor(name, serverValue) {
        const select = document.createElement('select');
        const choices = [
            ['server', `Server (${String(serverValue)})`],
            ['true', 'On (true)'],
            ['false', 'Off (false)']
        ];
        for (const [value, label] of choices) {
            const option = document.createElement('option');
            option.value = value;
            option.textContent = label;
            select.append(option);
        }
        select.value = own(overrides, name)
            ? String(overrides[name])
            : 'server';
        select.addEventListener('change', () => {
            if (select.value === 'server') unset(name);
            else set(name, select.value === 'true');
        });
        return select;
    }

    function makeJsonEditor(name, serverValue) {
        const wrapper = document.createElement('div');
        wrapper.className = 'json-editor';
        const input = document.createElement('input');
        input.value = formatValue(
            own(overrides, name) ? overrides[name] : serverValue
        );
        input.setAttribute('aria-label', `Override ${name} as JSON`);
        const apply = makeButton('Set', () => {
            try {
                set(name, parseValue(input.value));
            } catch (error) {
                setStatus(error.message, true);
            }
        });
        const reset = makeButton('Reset', () => unset(name));
        wrapper.append(input, apply, reset);
        return wrapper;
    }

    function renderRows() {
        if (!rows) return;
        rows.replaceChildren();
        const query = (search?.value || '').trim().toLowerCase();
        for (const entry of list()) {
            if (query && !entry.name.toLowerCase().includes(query)) continue;
            const row = document.createElement('tr');
            if (entry.overridden) row.dataset.overridden = 'true';

            const name = document.createElement('td');
            const flagName = document.createElement('code');
            flagName.textContent = entry.name;
            name.append(flagName);

            const server = document.createElement('td');
            server.textContent = formatValue(entry.server);

            const editor = document.createElement('td');
            editor.append(
                typeof entry.server === 'boolean'
                    ? makeBooleanEditor(entry.name, entry.server)
                    : makeJsonEditor(entry.name, entry.server)
            );
            row.append(name, server, editor);
            rows.append(row);
        }
    }

    function ensureUi() {
        if (host || !document.documentElement) return;
        host = document.createElement('div');
        host.id = 'streamable-feature-flags-userscript';
        shadow = host.attachShadow({ mode: 'open' });
        shadow.innerHTML = `
      <style>
        :host { all: initial; color-scheme: dark; }
        button, input, select { font: inherit; }
        #launcher { position: fixed; right: 16px; bottom: 16px; z-index: 2147483647;
          width: 44px; height: 44px; border: 0; border-radius: 22px; background: #ff4f64;
          color: white; font: 700 14px system-ui; box-shadow: 0 4px 18px #0008; cursor: pointer; }
        #panel { display: none; position: fixed; inset: 5vh 3vw; z-index: 2147483647;
          overflow: hidden; border: 1px solid #536079; border-radius: 10px; background: #111827;
          color: #e5e7eb; box-shadow: 0 14px 60px #000b; font: 13px system-ui; }
        #panel.open { display: grid; grid-template-rows: auto auto 1fr auto; }
        header, .toolbar, footer { display: flex; gap: 8px; align-items: center; padding: 10px 12px; }
        header { border-bottom: 1px solid #374151; }
        header strong { flex: 1; font-size: 15px; }
        .toolbar input { flex: 1; min-width: 180px; }
        .table-wrap { overflow: auto; }
        table { width: 100%; border-collapse: collapse; }
        th, td { padding: 8px 10px; border-top: 1px solid #263244; text-align: left; vertical-align: middle; }
        th { position: sticky; top: 0; background: #1f2937; }
        tr[data-overridden="true"] { background: #3b2430; }
        code { color: #f9a8d4; }
        td:nth-child(2) { max-width: 260px; overflow-wrap: anywhere; color: #a7f3d0; }
        button, input, select { border: 1px solid #4b5563; border-radius: 5px; background: #1f2937;
          color: #f9fafb; padding: 5px 8px; }
        button { cursor: pointer; }
        button.primary { border-color: #ff4f64; background: #ff4f64; }
        .json-editor { display: flex; gap: 5px; }
        .json-editor input { min-width: 180px; flex: 1; }
        footer { border-top: 1px solid #374151; color: #9ca3af; }
        #status[data-error="true"] { color: #fca5a5; }
      </style>
      <button id="launcher" title="Streamable feature flags">FF</button>
      <section id="panel" role="dialog" aria-label="Streamable feature flags">
        <header><strong>Streamable feature flags</strong><button id="close">Close</button></header>
        <div class="toolbar"><input id="search" type="search" placeholder="Filter flags">
          <button id="refresh">Refresh API list</button><button id="clear">Clear overrides</button>
          <button id="reload" class="primary">Reload and apply</button></div>
        <div class="table-wrap"><table><thead><tr><th>Flag</th><th>Server value</th><th>Local value</th></tr></thead>
          <tbody id="rows"></tbody></table></div>
        <footer id="status">Waiting for the dashboard flag response…</footer>
      </section>`;
        document.documentElement.append(host);

        panel = shadow.querySelector('#panel');
        rows = shadow.querySelector('#rows');
        status = shadow.querySelector('#status');
        search = shadow.querySelector('#search');
        shadow.querySelector('#launcher').addEventListener('click', open);
        shadow.querySelector('#close').addEventListener('click', close);
        shadow
            .querySelector('#refresh')
            .addEventListener('click', () =>
                refresh().catch((error) => setStatus(error.message, true))
            );
        shadow.querySelector('#clear').addEventListener('click', clear);
        shadow
            .querySelector('#reload')
            .addEventListener('click', () => location.reload());
        search.addEventListener('input', renderRows);
        renderRows();
    }

    function open() {
        ensureUi();
        panel?.classList.add('open');
        if (!Object.keys(serverFlags).length) {
            refresh().catch((error) => setStatus(error.message, true));
        }
    }

    function close() {
        panel?.classList.remove('open');
    }

    Object.defineProperty(window, CONTROLLER_NAME, {
        configurable: false,
        enumerable: true,
        value: Object.freeze({
            clear,
            close,
            get: (name) => list().find((entry) => entry.name === name),
            list,
            open,
            refresh,
            set,
            storageKey: STORAGE_KEY,
            unset
        }),
        writable: false
    });

    if (document.readyState === 'loading') {
        document.addEventListener('DOMContentLoaded', ensureUi, { once: true });
    } else {
        ensureUi();
    }
})();
