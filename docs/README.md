# NativeHTMLKit Core documentation

Documentation for the native host (Core 1.0.3, wire protocol v1), organised by what you need.

| | For app developers (using an SDK or the CLI) | For SDK and client authors |
|---|---|---|
| **Tutorial** — learn by doing | Start with your SDK's tutorial ([Python](https://github.com/nativehtmlkit/nativehtmlkit-python), [PowerShell](https://github.com/nativehtmlkit/nativehtmlkit-powershell), [CLI](https://github.com/nativehtmlkit/nativehtmlkit-cli)) | [Your first wire-v1 client](tutorials/first-wire-client.md) |
| **How-to** — solve a task | [HTML and CSS support](guides/html-css-support.md) (incl. debugging a page) | [Writing an SDK](../protocol/SDK-AUTHORING.md); [examples](../examples/README.md) |
| **Reference** — look it up | [Events](guides/events.md); [generated HTML/CSS profile](reference/html-css-profile.md) | [Protocol reference](../protocol/README.md); [document ops](../protocol/reference/document-commands.md); [generated wire tables](../protocol/reference/wire-tables.md); [contract `protocol/v1/`](../protocol/v1/) |
| **Explanation** — understand it | [Architecture](ARCHITECTURE.md) | [API surface](API.md); [compatibility policy](../protocol/COMPATIBILITY.md); [conformance record](../protocol/CONFORMANCE-ROADMAP.md) |

Building and testing Core itself: [BUILD.md](BUILD.md).

## Keeping these pages accurate

Two references are generated, never edited by hand, and checked:

```bash
python3 scripts/docs/generate-wire-tables.py --check                      # from protocol/v1/
python3 scripts/docs/generate-html-css-reference.py --host PATH --check   # asks a live host
python3 scripts/docs/check-docs.py --host PATH                            # everything, incl. examples
```

`PATH` is a `nativehtmlkit-host` binary of the release being documented (headless mode is used; no
desktop is needed).
