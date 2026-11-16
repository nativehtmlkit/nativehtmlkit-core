#!/usr/bin/env python3
"""Regenerate docs/reference/html-css-profile.md by interrogating a real nativehtmlkit-host.

Every table in the generated page is the host's own answer, obtained over wire v1 in headless
mode: the ``css_profile`` op, ``get_computed_style`` for a curated list of property names and
units, computed colours for selector and media-query probes, ``document.load`` outcomes for
markup the hosted profile accepts or rejects, and roles from ``get_accessibility_tree``.

    python3 scripts/docs/generate-html-css-reference.py --host PATH            # rewrite the page
    python3 scripts/docs/generate-html-css-reference.py --host PATH --check    # fail if stale

``--host`` defaults to $NATIVEHTMLKIT_HOST_BINARY. Only the Python standard library and the
example wire client in ``examples/wire-v1/`` are used. Exit status: 0 ok, 1 stale/failed, 2 usage.
"""
from __future__ import annotations

import argparse
import difflib
import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "examples" / "wire-v1"))
import wire_client  # noqa: E402

OUTPUT = ROOT / "docs" / "reference" / "html-css-profile.md"

# Property names asked of get_computed_style. The host classifies each one; nothing here is a claim.
PROPERTIES = {
    "Box model and sizing": "display box-sizing width height min-width min-height max-width max-height "
        "margin margin-top margin-inline margin-block padding padding-top padding-inline padding-block "
        "aspect-ratio inline-size block-size overflow overflow-x overflow-y overflow-clip-margin "
        "visibility field-sizing contain contain-intrinsic-size content-visibility container-type container-name",
    "Positioning": "position top right bottom left inset inset-inline z-index float clear isolation "
        "anchor-name position-anchor position-area position-try",
    "Flexbox and grid": "flex flex-direction flex-wrap flex-flow flex-grow flex-shrink flex-basis order "
        "justify-content justify-items justify-self align-content align-items align-self place-content "
        "place-items place-self gap row-gap column-gap grid-template-columns grid-template-rows "
        "grid-template-areas grid-auto-flow grid-auto-columns grid-auto-rows grid-column grid-row grid-area",
    "Multi-column and fragmentation": "columns column-count column-width column-gap column-rule column-span "
        "column-fill break-before break-after break-inside orphans widows",
    "Tables and lists": "table-layout border-collapse border-spacing caption-side empty-cells vertical-align "
        "list-style list-style-type list-style-position list-style-image counter-reset counter-increment counter-set",
    "Text and fonts": "color font font-family font-size font-weight font-style font-stretch font-variant "
        "font-variant-caps font-variant-numeric font-feature-settings font-variation-settings "
        "font-optical-sizing font-kerning font-synthesis font-size-adjust line-height letter-spacing "
        "word-spacing text-align text-align-last text-indent text-transform text-decoration "
        "text-decoration-line text-decoration-style text-decoration-color text-decoration-thickness "
        "text-underline-offset text-underline-position text-decoration-skip-ink text-shadow text-overflow "
        "text-wrap text-wrap-mode text-wrap-style white-space white-space-collapse word-break overflow-wrap "
        "hyphens hyphenate-character tab-size line-clamp -webkit-line-clamp direction unicode-bidi "
        "writing-mode text-orientation text-combine-upright -webkit-text-stroke -webkit-text-fill-color "
        "initial-letter quotes",
    "Backgrounds, borders and effects": "background background-color background-image background-position "
        "background-size background-repeat background-clip background-origin background-attachment "
        "background-blend-mode border border-width border-style border-color border-radius border-image "
        "border-top-left-radius outline outline-offset box-shadow opacity filter backdrop-filter "
        "mix-blend-mode clip-path mask mask-image shape-outside object-fit object-position "
        "image-rendering image-orientation accent-color caret-color color-scheme appearance corner-shape",
    "Transforms, transitions and animations": "transform transform-origin translate rotate scale perspective "
        "transform-style backface-visibility transition transition-property transition-duration "
        "transition-timing-function transition-delay transition-behavior animation animation-name "
        "animation-duration animation-timing-function animation-iteration-count animation-direction "
        "animation-fill-mode animation-play-state animation-delay animation-timeline view-transition-name",
    "Interaction and scrolling": "cursor pointer-events user-select touch-action resize scroll-behavior "
        "scroll-snap-type scroll-snap-align scroll-margin scroll-padding overscroll-behavior "
        "scrollbar-width scrollbar-color scrollbar-gutter will-change zoom print-color-adjust",
}

UNITS = "px em rem ex ch cap ic lh rlh % vw vh vmin vmax svh lvh dvh cqw cqh cqi cqb cqmin cqmax cm mm Q in pt pc".split()

SELECTORS = [
    ("type, class, id", "p.k#s1"), ("attribute [a=b]", "[data-x='1']"), ("attribute [a^=b]", "[data-y^='ab']"),
    ("descendant / child", "main > section p.d"), ("sibling + and ~", "h2 + p.sib ~ p"),
    (":is() / :where()", ":is(.a, .b):where(p)"), (":not()", "p:not(.zzz).n"),
    (":has()", "section:has(> .marker) .h"), (":nth-child(An+B of S)", "li:nth-child(2 of .o)"),
    (":first-child / :last-of-type", "ol > li:first-child"), (":empty", "span.e:empty"), (":root", ":root"),
    (":checked", "input:checked"), (":disabled", "button:disabled"), (":required", "input:required"),
    (":placeholder-shown", "input:placeholder-shown"), (":invalid", "input.v:invalid"),
    (":indeterminate (progress without value)", "progress:indeterminate"), (":read-only (textarea readonly)", "textarea:read-only"), (":read-write (input)", "input.rw:read-write"),
    (":dir(rtl)", "p:dir(rtl)"), (":lang(de)", "p:lang(de)"), (":link / :any-link", "a:any-link"),
    (":visited (privacy: never matches)", "a:visited"), (":defined", "p.def:defined"),
    ("nesting (&)", "div.nest { & > p { color: rgb(1, 2, 3); } }"),
    ("@scope with explicit root", "@scope (.scope-root) { p { color: rgb(1, 2, 3); } }"),
]
SELECTOR_BODY = """<main><section><p class="k" id="s1">x</p><p class="d">x</p>
<span class="marker"></span><p class="h">x</p></section></main><div data-x="1">x</div><div data-y="abc">x</div>
<h2>h</h2><p class="sib">x</p><p class="sib2">x</p><p class="a">x</p><p class="n">x</p>
<ul><li class="o">1</li><li>2</li><li class="o">3</li></ul><ol><li>1</li></ol><span class="e"></span>
<input type="checkbox" checked><button disabled>b</button><input class="req" required value="v">
<input class="ph" placeholder="p"><input class="v" required><progress></progress><input class="rw">
<textarea readonly></textarea><p dir="rtl">r</p><p lang="de">d</p><a href="x">l</a><p class="def">x</p>
<div class="nest"><p>x</p></div><div class="scope-root"><p>x</p></div>"""
SELECTOR_TARGET = {  # selector label -> CSS selector that picks the element the probe styles
    "type, class, id": "#s1", "attribute [a=b]": "[data-x]", "attribute [a^=b]": "[data-y]",
    "descendant / child": ".d", "sibling + and ~": ".sib2", ":is() / :where()": ".a", ":not()": ".n",
    ":has()": ".h", ":nth-child(An+B of S)": "ul > li:nth-child(3)", ":first-child / :last-of-type": "ol > li",
    ":empty": ".e", ":root": "html", ":checked": "input[checked]", ":disabled": "button",
    ":required": ".req", ":placeholder-shown": ".ph", ":invalid": ".v", ":indeterminate (progress without value)": "progress", ":read-write (input)": ".rw",
    ":read-only (textarea readonly)": "textarea", ":dir(rtl)": "[dir=rtl]", ":lang(de)": "[lang=de]", ":link / :any-link": "a",
    ":visited (privacy: never matches)": "a", ":defined": ".def", "nesting (&)": ".nest > p",
    "@scope with explicit root": ".scope-root > p",
}

MEDIA = ["(min-width: 100px)", "(max-width: 100px)", "(orientation: landscape)", "(aspect-ratio > 1)",
         "(resolution: 1dppx)", "(hover: hover)", "(pointer: fine)", "(prefers-color-scheme: light)",
         "(prefers-color-scheme: dark)", "(prefers-reduced-motion: reduce)", "(prefers-contrast: more)",
         "(forced-colors: active)", "print", "screen"]

MARKUP = [  # (label, body markup) loaded as a whole document
    ("plain document", "<p>ok</p>"),
    ("<script>", "<script>1</script>"), ("<iframe>", "<iframe></iframe>"), ("<object>", "<object></object>"),
    ("<embed>", "<embed>"), ("on* attribute", "<p onclick='x'>p</p>"), ("srcdoc attribute", "<div srcdoc='x'></div>"),
    ("contenteditable=\"true\"", "<div contenteditable='true'>e</div>"),
    ("contenteditable=\"false\"", "<div contenteditable='false'>e</div>"),
    ("duplicate id", "<p id='a'></p><p id='a'></p>"), ("id with a space", "<p id='a b'></p>"),
    ("<canvas>", '<canvas id="x"></canvas>'), ("<video>", '<video id="x"></video>'), ("<audio controls>", '<audio id="x" controls></audio>'),
    ("<template>", "<template><p>t</p></template>"), ("<svg> inline", "<svg width='10' height='10'><rect width='5' height='5'/></svg>"),
    ("<math>", "<math><mi>x</mi></math>"), ("custom element", "<my-widget>w</my-widget>"),
    ("<noscript>", "<noscript>n</noscript>"), ("<link rel=stylesheet> (missing file)", "<link rel='stylesheet' href='missing.css'>"),
    ("<img> with data: URL", "<img alt='a' src='data:image/gif;base64,R0lGODlhAQABAAAAACw='>"),
    ("<img src=https://...>", "<img alt='a' src='https://example.com/x.png'>"),
]
INPUT_TYPES = ("text search tel url email password number range checkbox radio file button submit reset "
               "image hidden color date time month week datetime-local").split()

A11Y_HTML = [
    ("button", "<button id=x>b</button>"), ("a href", "<a id=x href=y>l</a>"), ("a (no href)", "<a id=x>l</a>"),
    ("h2", "<h2 id=x>h</h2>"), ("p", "<p id=x>p</p>"), ("ul", "<ul id=x><li>i</li></ul>"),
    ("ol > li", "<ol><li id=x>i</li></ol>"), ("img alt", "<img id=x alt=pic src='data:image/gif;base64,R0lGODlhAQABAAAAACw='>"),
    ("input text", "<input id=x>"), ("input checkbox", "<input id=x type=checkbox>"), ("input radio", "<input id=x type=radio>"),
    ("input range", "<input id=x type=range>"), ("input number", "<input id=x type=number>"),
    ("input search", "<input id=x type=search>"), ("input password", "<input id=x type=password>"),
    ("input file", "<input id=x type=file>"), ("textarea", "<textarea id=x></textarea>"),
    ("select", "<select id=x><option>a</option></select>"), ("select size=3", "<select id=x size=3><option>a</option></select>"),
    ("progress", "<progress id=x value=1 max=2></progress>"), ("meter", "<meter id=x value=1></meter>"),
    ("table", "<table id=x><tr><td>c</td></tr></table>"), ("th", "<table><tr><th id=x>h</th></tr></table>"),
    ("nav", "<nav id=x>n</nav>"), ("main", "<main id=x>m</main>"), ("header (top level)", "<header id=x>h</header>"),
    ("footer (top level)", "<footer id=x>f</footer>"), ("aside", "<aside id=x>a</aside>"), ("section", "<section id=x>s</section>"),
    ("article", "<article id=x>a</article>"), ("form", "<form id=x></form>"), ("fieldset", "<fieldset id=x><legend>l</legend></fieldset>"),
    ("details", "<details id=x><summary>s</summary>d</details>"), ("summary", "<details><summary id=x>s</summary>d</details>"),
    ("dialog (open)", "<dialog id=x open>d</dialog>"), ("hr", "<hr id=x>"), ("figure", "<figure id=x>f</figure>"),
    ("blockquote", "<blockquote id=x>q</blockquote>"), ("dl", "<dl id=x><dt>t</dt><dd>d</dd></dl>"),
    ("label", "<label id=x>l</label>"), ("output", "<output id=x>o</output>"), ("div", "<div id=x>d</div>"),
]
ARIA_ROLES = ("alert alertdialog article banner button cell checkbox columnheader combobox complementary "
              "contentinfo dialog document figure form grid gridcell group heading img link list listbox "
              "listitem log main menu menubar menuitem menuitemcheckbox menuitemradio meter navigation note "
              "option paragraph progressbar radio radiogroup region row rowheader search searchbox separator "
              "slider spinbutton status switch tab table tablist tabpanel toolbar tooltip tree treegrid treeitem "
              "feed marquee timer math definition term").split()

PROBE = "rgb(1, 2, 3)"


# ---------------------------------------------------------------------------------- backend
def collect(session: wire_client.Session) -> dict:
    window = session.request("window.create", {"title": "profile", "width": 800, "height": 600})
    wid = window["window_id"]

    def load(html_text: str) -> wire_client.Document:
        loaded = session.request("document.load", {"window_id": wid, "html": html_text})
        return wire_client.Document(session, wid, loaded["document_id"])

    doc = load("<!doctype html><html><body><div id=t>t</div></body></html>")
    session.request("app.ready")
    out: dict = {"host_version": session.hello["host_version"],
                 "capabilities_css_profile": session.hello["capabilities"]["css_profile"]}
    out["profile"] = doc.command("css_profile")

    names = [n for group in PROPERTIES.values() for n in group.split()]
    styles: dict = {}
    for start in range(0, len(names), 64):  # MAX_STYLE_PROPERTIES = 64 per request
        chunk = names[start:start + 64]
        styles.update(doc.command("get_computed_style", element="t", properties=chunk)["properties"])
    out["properties"] = styles

    units = {}
    for unit in UNITS:
        probe = load(f"<!doctype html><style>:root{{line-height:20px}}</style><div style='container-type:size;width:400px;height:300px'>"
                     f"<div id=u style='line-height:20px;width:10{unit}'>u</div></div>")
        value = probe.command("get_computed_style", element="u", properties=["width"])["properties"]["width"]
        units[unit] = value.get("value")
    out["units"] = units

    selectors = {}
    for label, selector in SELECTORS:
        rule = selector if "{" in selector else f"{selector} {{ color: {PROBE}; }}"
        probe = load(f"<!doctype html><html><head><style>{rule}</style></head><body>{SELECTOR_BODY}</body></html>")
        node = probe.command("query_nodes", selector=SELECTOR_TARGET[label], limit=1)["nodes"]
        color = probe.command("get_computed_style", node=node[0]["node"], properties=["color"])["properties"]["color"]
        selectors[label] = {"selector": selector, "matched": color.get("value") == PROBE}
    out["selectors"] = selectors

    media = {}
    for query in MEDIA:
        probe = load(f"<!doctype html><style>@media {query} {{ #m {{ color: {PROBE}; }} }}</style><p id=m>m</p>")
        matched = probe.command("get_computed_style", element="m", properties=["color"])["properties"]["color"]["value"] == PROBE
        probe.command("set_environment", color_scheme="dark", reduced_motion="reduce")
        after = probe.command("get_computed_style", element="m", properties=["color"])["properties"]["color"]["value"] == PROBE
        media[query] = {"default": matched, "dark_reduce_override": after}
    out["media"] = media

    def try_load(body: str) -> dict:
        try:
            probe = load(f"<!doctype html><html><body>{body}</body></html>")
            return {"loaded": True, "doc": probe}
        except wire_client.WireError as error:
            return {"loaded": False, "code": error.code, "message": error.message}

    markup = {}
    for label, body in MARKUP:
        result = try_load(body)
        entry = {k: v for k, v in result.items() if k != "doc"}
        if result["loaded"] and 'id="x"' in body:  # report the laid-out box of placeholders
            rect = result["doc"].command("get_rect", element="x")["rect"]
            entry["box"] = f"{rect['width']:g}x{rect['height']:g} px box"
        markup[label] = entry
    out["markup"] = markup

    inputs = {}
    for kind in INPUT_TYPES:
        result = try_load(f"<input id=x type={kind}>")
        entry = {k: v for k, v in result.items() if k != "doc"}
        if result["loaded"]:
            entry["role"] = role_of(result["doc"])
        inputs[kind] = entry
    out["inputs"] = inputs

    out["a11y_html"] = {label: role_of(load(f"<!doctype html><body>{body}</body>")) for label, body in A11Y_HTML}
    out["a11y_aria"] = {role: role_of(load(f"<!doctype html><body><div id=x role={role}>r</div></body>"))
                        for role in ARIA_ROLES}
    return out


def role_of(doc: wire_client.Document) -> str | None:
    tree = doc.command("get_accessibility_tree")["tree"]
    for node in tree["nodes"]:
        if node.get("html_id") == "x":
            return node["role"]
    return None  # element excluded from the accessibility tree


def backend_main(result_path: str) -> int:
    session = wire_client.connect_from_environment()
    try:
        data = collect(session)
        Path(result_path).write_text(json.dumps(data, indent=1, sort_keys=True), encoding="utf-8")
        session.request("app.request_quit")
        while session.next_message()["method"] != "app.shutdown":
            pass
    except wire_client.Disconnected:
        pass
    finally:
        session.close()
    return 0


# --------------------------------------------------------------------------------- rendering
def yes(flag: bool) -> str:
    return "yes" if flag else "no"


def render(data: dict) -> str:
    profile = data["profile"]
    lines = [
        "<!-- GENERATED by scripts/docs/generate-html-css-reference.py from a live host. Do not edit. -->",
        "# HTML and CSS profile (generated reference)",
        "",
        f"Generated from **nativehtmlkit-host {data['host_version']}** in headless mode. CSS profile "
        f"`{profile['version']}` (`host.hello` capabilities `css_profile`: `{data['capabilities_css_profile']}`).",
        "Every value below is the host's own answer to a wire-v1 request; see the "
        "[HTML and CSS support guide](../guides/html-css-support.md) for explanations and caveats.",
        "Headless mode exercises parsing, style, layout and document state, not native painting, input or "
        "screen readers.",
        "",
        "Regenerate: `python3 scripts/docs/generate-html-css-reference.py --host PATH` (`--check` verifies).",
        "",
        "## 1. CSS profile features (`css_profile` op)",
        "",
        "The profile lists the features that differ from \"everything Stylo parses\" or that need a "
        "decision. States: **on** (enabled), **off** (disabled by the profile; declarations are dropped), "
        "**unavailable** (the pinned engine cannot parse it), **filtered** (removed or reported by the "
        "profile filter). A property or selector that does not appear here is governed by section 2/4.",
        "",
    ]
    groups: dict[str, list] = {}
    for feature in profile["features"]:
        groups.setdefault(feature["id"].split(".")[1], []).append(feature)
    lines += ["| Group | on | off | unavailable | filtered |", "|---|---|---|---|---|"]
    for group in sorted(groups):
        cells = []
        for state in ("on", "off", "unavailable", "filtered"):
            ids = [f"`{f['id'].split('.', 2)[2]}`" for f in groups[group] if f["state"] == state]
            cells.append(", ".join(ids) or "")
        lines.append(f"| {group} | " + " | ".join(cells) + " |")
    other = sorted({f["state"] for f in profile["features"]} - {"on", "off", "unavailable", "filtered"})
    if other:
        lines += ["", f"Other states reported: {', '.join(other)}."]

    lines += ["", "## 2. CSS properties (`get_computed_style`)", "",
              "Each name was asked of `get_computed_style` on a `<div>`. **supported**: the host computes it "
              "(this does not prove every value renders natively); **unsupported**: excluded by the profile "
              "(feature id shown; declarations are parsed and ignored); **invalid**: not a property name the "
              "engine knows. *inh* marks inherited properties.", ""]
    for group, names in PROPERTIES.items():
        supported, other_rows = [], []
        for name in names.split():
            entry = data["properties"][name]
            if entry["status"] == "supported":
                supported.append(f"`{name}`" + (" *inh*" if entry.get("inherited") else ""))
            else:
                detail = entry.get("feature") or entry.get("reason") or ""
                other_rows.append(f"`{name}` ({entry['status']}{': ' + detail if detail else ''})")
        lines.append(f"- **{group}** — supported: {', '.join(supported)}.")
        if other_rows:
            lines.append(f"  Not supported: {', '.join(other_rows)}.")

    lines += ["", "## 3. Length units", "",
              "`width: 10<unit>` on an element with `line-height: 20px` (root `line-height: 20px`, font-size 16px) "
              "inside a 400x300px size container, in an 800x600 viewport; the computed `width` (`auto` means "
              "the declaration was dropped).", "", "| Unit | Computed width |", "|---|---|"]
    lines += [f"| `{unit}` | `{value}` |" for unit, value in data["units"].items()]

    lines += ["", "## 4. Selectors", "",
              "A rule `selector { color: rgb(1, 2, 3) }` and a matching element; *matched* means the computed "
              "colour changed.", "", "| Selector feature | Probe | Matched |", "|---|---|---|"]
    for label, entry in data["selectors"].items():
        probe = entry["selector"].replace("|", "\\|")
        lines.append(f"| {label} | `{probe}` | {yes(entry['matched'])} |")

    lines += ["", "## 5. Media queries", "",
              "*Default*: a fresh headless document (800x600 CSS px, scale 1, no native preferences). "
              "*Override*: after `set_environment {color_scheme: \"dark\", reduced_motion: \"reduce\"}`.", "",
              "| Query | Default | Override |", "|---|---|---|"]
    for query, entry in data["media"].items():
        lines.append(f"| `{query}` | {yes(entry['default'])} | {yes(entry['dark_reduce_override'])} |")

    lines += ["", "## 6. Markup accepted by `document.load`", "",
              "Each snippet was loaded as a whole document. A rejected document fails the load with the "
              "error shown and the previous document stays in place.", "",
              "| Markup | Loads | Error | Notes |", "|---|---|---|---|"]
    for label, entry in data["markup"].items():
        note = ""
        if "box" in entry:
            note = entry["box"]
        error = "" if entry["loaded"] else f"{entry['code']}: {entry['message']}"
        lines.append(f"| `{label}` | {yes(entry['loaded'])} | {error} | {note} |")

    lines += ["", "## 7. `<input type>` values", "", "| type | Loads | Accessibility role / error |", "|---|---|---|"]
    for kind, entry in data["inputs"].items():
        detail = entry.get("role") if entry["loaded"] else f"{entry['code']}: {entry['message']}"
        lines.append(f"| `{kind}` | {yes(entry['loaded'])} | {detail} |")

    lines += ["", "## 8. Accessibility roles", "",
              "Role of the element with `id=x` in `get_accessibility_tree` (AccessKit role names; *excluded* "
              "means the element has no node of its own).", "", "### HTML elements", "",
              "| Element | Role |", "|---|---|"]
    lines += [f"| `{label}` | {role or '*excluded*'} |" for label, role in data["a11y_html"].items()]
    lines += ["", "### ARIA `role` attribute on a `<div>`", "", "| role | Mapped role |", "|---|---|"]
    lines += [f"| `{role}` | {mapped or '*excluded*'} |" for role, mapped in data["a11y_aria"].items()]
    return "\n".join(lines) + "\n"


# --------------------------------------------------------------------------------- launcher
def launcher_main(args: argparse.Namespace) -> int:
    host = args.host or os.environ.get("NATIVEHTMLKIT_HOST_BINARY")
    if not host or not os.path.isfile(host):
        print("Pass --host PATH or set NATIVEHTMLKIT_HOST_BINARY.", file=sys.stderr)
        return 2
    with tempfile.TemporaryDirectory(prefix="nhk-profile-") as folder:
        result = os.path.join(folder, "result.json")
        manifest = wire_client.write_manifest(folder, app_id="org.nativehtmlkit.DocsProfile", name="profile",
                                              program=sys.executable,
                                              args=[os.path.abspath(__file__), "--backend", result])
        process = subprocess.run([host, "run", "--app", manifest, "--headless"],
                                 stdin=subprocess.DEVNULL, capture_output=True, timeout=300)
        for line in process.stderr.splitlines():
            text = wire_client.relay_host_stderr(line)
            if text:
                print(text, file=sys.stderr)
        if process.returncode != 0 or not os.path.exists(result):
            print(f"host run failed with status {process.returncode}", file=sys.stderr)
            return 1
        data = json.loads(Path(result).read_text(encoding="utf-8"))
    if args.json:
        Path(args.json).write_text(json.dumps(data, indent=1, sort_keys=True), encoding="utf-8")
    text = render(data)
    if args.check:
        current = OUTPUT.read_text(encoding="utf-8") if OUTPUT.exists() else ""
        if current != text:
            sys.stdout.writelines(difflib.unified_diff(current.splitlines(True), text.splitlines(True),
                                                       str(OUTPUT), "live host"))
            print("STALE: docs/reference/html-css-profile.md differs from the live host", file=sys.stderr)
            return 1
        print("OK: docs/reference/html-css-profile.md matches the live host")
        return 0
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    OUTPUT.write_text(text, encoding="utf-8")
    print(f"wrote {OUTPUT.relative_to(ROOT)}")
    return 0


if __name__ == "__main__":
    if len(sys.argv) == 3 and sys.argv[1] == "--backend":
        raise SystemExit(backend_main(sys.argv[2]))
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--host", help="nativehtmlkit-host binary (default: $NATIVEHTMLKIT_HOST_BINARY)")
    parser.add_argument("--check", action="store_true", help="fail if the committed page is stale")
    parser.add_argument("--json", help="also write the raw answers to this JSON file")
    raise SystemExit(launcher_main(parser.parse_args()))
