# Bundled Playwright is stripped in `dist/` — every browser-backed provider is dead on npm installs

**Version:** 3.8.49 (global npm install)
**Platform:** Windows 11, Node 24.16.0
**Affected providers:** `gemini-web`, `claude-web` (browser fallback), `duckduckgo-web` (browser fallback), `grok-web` — anything that reaches `import("playwright")`

## Symptom

Any request to a browser-backed provider fails. `gemini-web` returns:

```
[503] Gemini Web requires the Playwright Chromium browser, which is not installed.
Run `npx playwright install chromium` on the host (or rebuild the Docker image with browsers).
```

Playwright **is** installed, and the browsers **are** installed. The message is wrong.

## Root cause 1 — `dist/node_modules` copies are incomplete

The build strips files from the vendored Playwright packages. Comparing the complete
top-level copies with the bundled ones (identical versions, 1.62.0):

| Package | Complete | Bundled in `dist/` | Missing |
|---|---|---|---|
| `playwright-core` | 111 files | 85 files | `browsers.json`, `bin/`, `cli.js`, `index.d.ts`, `types/` |
| `playwright` | 14 top-level entries | 4 | **`lib/`**, `types/`, `cli.js`, `test.*`, `jsx-runtime.*` |

`playwright/index.js` requires `./lib/…`, which does not exist in the bundled copy, so
the import fails before any browser launch. The first failure surfaced as:

```
Failed to load external module playwright: Error: Cannot find module
'…\omniroute\dist\node_modules\playwright\node_modules\playwright-core\browsers.json'
```

## Root cause 2 — restoring the files is not sufficient

After copying every missing file from the complete 1.62.0 copies into the bundled ones
(so both are byte-for-byte complete, 111/111 and 14/14) and installing the exact Chromium
revision the bundled `browsers.json` requests (1234), **the failure persists unchanged**.

The module works fine outside the server, from the same directory the server runs in:

```bash
cd <omniroute>/dist
node --input-type=module -e "const pw=await import('playwright'); \
  const b=await pw.chromium.launch({headless:true}); console.log(b.version()); await b.close()"
# -> 151.0.7922.34
```

Inside the server it fails in **~50 ms** — no launch is attempted. `playwright` appears
only twice in the emitted chunk containing the `gemini-web` executor, both times inside
the error-handling regex; the actual `import("playwright")` is resolved through
Turbopack's runtime external-module loader, which is where it breaks.

The comment in `services/browserPool.ts` shows the team already hit this class of problem:

> Keep this computed: cloakbrowser is an optional runtime enhancer, and a literal dynamic
> import with the package name makes Turbopack resolve it during route compilation.

`executors/gemini-web.ts` uses exactly the literal form that comment warns about:

```ts
const { chromium } = await import("playwright");
```

## Root cause 3 — the real error is unrecoverable by users

`executors/gemini-web.ts` catches the error, tests it against
`isMissingBrowserExecutable()`, and replaces it with the "not installed" message. The
original message is never logged — not to stdout under `omniroute serve --log`, not to
`~/.omniroute/logs/app.log`, not to `call_logs.error_summary`. Users are told to install
something that is already installed, with no way to see what actually went wrong.

## Reproduction

1. `npm i -g omniroute@latest` on Windows
2. `npx playwright install chromium`
3. Connect a `gemini-web` account
4. `POST /v1/chat/completions` with `{"model":"gemini-web/gemini-3.1-pro"}`
5. → 503 "browser not installed", ~50 ms, no browser process spawned

## Suggested fixes

1. **Stop stripping the vendored Playwright packages**, or mark `playwright` as a true
   runtime external resolved from the package root rather than bundling a partial copy.
2. **Use the computed-specifier trick** already used for `cloakbrowser` in
   `browserPool.ts`, in `gemini-web.ts` and any other executor importing `playwright`
   literally.
3. **Log `rawMessage` before remapping it.** A one-line `log?.error?.` in the
   `isMissingBrowserExecutable` branch would have made this a five-minute diagnosis
   instead of a multi-hour one.
4. Consider making the 503 message conditional on actually verifying the executable is
   absent (`chromium.executablePath()` + `fs.existsSync`), so it cannot fire when the
   browser is present.

## Impact

On a stock Windows npm install, browser-backed providers cannot work at all, and the
error message actively misdirects. For users who chose OmniRoute specifically for its
web-session providers, the feature is unavailable with no local remedy.
