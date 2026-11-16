# Core examples

These examples are for **SDK and client authors**: they talk to `nativehtmlkit-host` over wire v1
directly, with only the Python standard library (3.10+). Application developers should start from
the Python or PowerShell SDK examples instead.

| Example | What it teaches |
|---|---|
| [`wire-v1/wire_client.py`](wire-v1/wire_client.py) | A minimal single-threaded client: launch variables, Unix socket / named pipe, framing, `host.hello`, id correlation, answering `{allow}` host requests, notification queue, manifest writing, relaying host stderr. Imported by the others. |
| [`wire-v1/notes_app.py`](wire-v1/notes_app.py) | A complete "Quick notes" app: window, document, `app.ready`, `submit`/`invalid`/`click` events, `get_value`/`insert_html`/`set_text`, escaping untrusted text, quit decision, `app.shutdown`. The [tutorial](../docs/tutorials/first-wire-client.md) walks through it. |
| [`wire-v1/live_bitmap.py`](wire-v1/live_bitmap.py) | A live signal chart: `bitmap.create`, mapping the transport file, writing RGBA8 frames into slots, `bitmap.present` sequencing, `bitmap.close`. |

Each script is both launcher and backend: run it, and it starts the host, which starts the script
again as its backend.

```bash
export NATIVEHTMLKIT_HOST_BINARY=/path/to/nativehtmlkit-host    # Core 1.0.3
python3 examples/wire-v1/notes_app.py                            # real window (desktop session)
python3 examples/wire-v1/notes_app.py --headless --self-test     # automated check, no desktop
python3 examples/wire-v1/notes_app.py --headless --self-test --transcript session.jsonl
python3 examples/wire-v1/live_bitmap.py --headless --self-test
```

`--self-test` drives the app through the public protocol (there is no synthetic input in headless
mode), verifies the resulting document state, quits through `app.request_quit`, and exits 0 only if
every check passed. `python3 scripts/docs/check-docs.py --host PATH` runs all examples.

Verified in headless mode with the Core 1.0.3 hosts on Linux x64 (Unix socket) and Windows x64
(named pipe, Python 3.13): every self-test passed and no host process or bitmap transport file was
left behind. Without `--headless` the scripts open real windows; that mode needs a desktop session
and was not exercised for this documentation release.
