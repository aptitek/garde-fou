## NiceGUI Reactive `asyncio` & Multiprocessing Standards (`GF080`)

### Non-Blocking Event Loop, Worker Context & Storage Security

- Keep `@ui.page`, `async def`, `ui.timer`, and UI callbacks 100% non-blocking: replace `time.sleep`, `requests.*`, `urlopen`, and `subprocess.run` with `await asyncio.sleep`, `httpx.AsyncClient`, or `await run.io_bound(...)` / `await run.cpu_bound(...)` (`NICEGUI-BLOCKING-EVENT-LOOP`).
- Encapsulate all `ui.*` elements inside `@ui.page(...)` routes and store per-client state in `app.storage.user` / `app.storage.tab` rather than module-level globals (`NICEGUI-GLOBAL-UI-OR-STATE-LEAK`).
- Never access `ui.*` or `app.storage.{user,tab,client,browser}` inside `run.io_bound`/`run.cpu_bound` workers or `@app.on_exception` (`NICEGUI-WORKER-CONTEXT-VIOLATION`), keep `ui.html(..., sanitize=True)`, serialize `ui.run_javascript` inputs with `json.dumps`, and read `storage_secret` from `os.environ` (`NICEGUI-SECURITY-XSS-AND-SECRET`).
