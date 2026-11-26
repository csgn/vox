#!/usr/bin/env python3
"""Generate docs/src/tour/data.js for the interactive Code Tour.

Reads every crates/*/src/**/*.rs file (std library only, no third-party Python packages) and
extracts crates, files, types, functions, doc comments, signatures, and a call graph detected
from the source. Flows (walkthroughs) come from docs/tour-flows.json; every function they
reference must exist, or this script fails.

Usage:
    scripts/gen-code-tour.py           # write docs/src/tour/data.js
    scripts/gen-code-tour.py --check   # exit 1 if data.js is out of date (used in CI)
"""

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "docs/src/tour/data.js"
FLOWS = ROOT / "docs/tour-flows.json"

# Variable / field names whose type is known, per file (fallback: "*"). Used to resolve
# calls like `self.doc.insert(` to `Document::insert`.
RECEIVERS = {
    "*": {
        "doc": "Document",
        "view": "Viewport",
        "registers": "Registers",
        "history": "History",
        "editor": "Editor",
        "term": "Terminal",
        "s": "Surface",
        "w": "CharWidth",
        "width": "CharWidth",
        "clipboard": "Clipboard",
        "storage": "TextStorage",
    },
    "crates/vox-core/src/document.rs": {"text": "TextStorage"},
    "crates/vox-core/src/motion.rs": {"text": "Document"},
    "crates/vox-core/src/search.rs": {"text": None},
}

FN_RE = re.compile(r"^(\s*)(pub(?:\([a-z]+\))? )?(const )?fn (\w+)")
TYPE_RE = re.compile(r"^(pub(?:\([a-z]+\))? )?(struct|enum|trait|type) (\w+)")
IMPL_RE = re.compile(r"^impl(?:<[^>]*>)? (?:([\w:]+) for )?(\w+)")


def crate_info(crate_dir):
    toml = (crate_dir / "Cargo.toml").read_text()
    deps_section = re.search(r"\[dependencies\](.*?)(\n\[|\Z)", toml, re.S)
    vox, third = [], []
    if deps_section:
        for line in deps_section.group(1).splitlines():
            m = re.match(r"([\w-]+)\s*=", line.strip())
            if m:
                (vox if m.group(1).startswith("vox") else third).append(m.group(1))
    lib = crate_dir / "src/lib.rs"
    main = crate_dir / "src/main.rs"
    root = lib if lib.exists() else main
    doc = " ".join(
        l.strip()[3:].strip() for l in root.read_text().splitlines() if l.strip().startswith("//!")
    )
    return {"deps": vox, "third_party": third, "doc": doc, "binary": main.exists()}


def doc_above(lines, i):
    """Collect `///` lines above line i, skipping attributes."""
    out = []
    j = i - 1
    while j >= 0:
        s = lines[j].strip()
        if s.startswith("///"):
            out.append(s[3:].strip())
        elif not s.startswith("#["):
            break
        j -= 1
    return " ".join(reversed(out))


def block_end(lines, i):
    """Index of the line closing the block that opens at or after line i."""
    depth = 0
    opened = False
    for k in range(i, len(lines)):
        code = re.sub(r'"(\\.|[^"\\])*"|\'(\\.|[^\'\\])\'|//.*', "", lines[k])
        for ch in code:
            if ch == "{":
                depth += 1
                opened = True
            elif ch == "}":
                depth -= 1
                if opened and depth == 0:
                    return k
        if not opened and code.rstrip().endswith(";"):
            return k
    return len(lines) - 1


def parse_file(crate, path, rel):
    text = path.read_text()
    lines = text.splitlines()
    # Stop at the test module.
    for i, l in enumerate(lines):
        if l.startswith("#[cfg(test)]"):
            lines = lines[:i]
            break
    module = "" if path.name in ("lib.rs", "main.rs") else path.stem
    fns, types = [], []
    owner = None  # (type, trait) of the current impl / trait block
    owner_end = -1
    i = 0
    while i < len(lines):
        line = lines[i]
        if i > owner_end:
            owner = None
        m = IMPL_RE.match(line)
        if m:
            owner = (m.group(2), m.group(1).split("::")[-1] if m.group(1) else None, False)
            owner_end = block_end(lines, i)
            i += 1
            continue
        m = TYPE_RE.match(line)
        if m:
            kind, name = m.group(2), m.group(3)
            types.append({
                "id": f"{crate}::{name}",
                "name": name,
                "kind": kind,
                "crate": crate,
                "file": rel,
                "line": i + 1,
                "public": m.group(1) == "pub ",
                "doc": doc_above(lines, i),
            })
            if kind == "trait":
                owner = (name, None, m.group(1) == "pub ")
                owner_end = block_end(lines, i)
            i += 1
            continue
        m = FN_RE.match(line)
        if m:
            indent, vis, _, name = m.groups()
            end = block_end(lines, i)
            sig_lines = []
            for k in range(i, end + 1):
                sig_lines.append(lines[k].strip())
                if "{" in lines[k] or lines[k].rstrip().endswith(";"):
                    break
            sig = " ".join(sig_lines).split("{")[0].rstrip(" ;")
            sig = re.sub(r"\(\s+", "(", re.sub(r",\s*\)", ")", sig))
            in_owner = owner is not None and len(indent) > 0
            if in_owner:
                fid = f"{crate}::{owner[0]}::{name}"
                qual = f"{owner[0]}::{name}"
            else:
                qual = f"{module}::{name}" if module else name
                fid = f"{crate}::{qual}"
            body = "\n".join(lines[i + 1:end + 1])
            fns.append({
                "id": fid,
                "name": name,
                "qual": qual,
                "crate": crate,
                "file": rel,
                "line": i + 1,
                # Trait methods (declarations and impls) are as visible as the trait.
                "public": vis == "pub " or (in_owner and (owner[1] is not None or owner[2])),
                "owner": owner[0] if in_owner else None,
                "trait": owner[1] if in_owner else None,
                "decl_only": sig_lines[-1].rstrip().endswith(";") and "{" not in sig_lines[-1],
                "sig": sig,
                "doc": doc_above(lines, i),
                "_body": body,
                "_module": module,
            })
            # Skip nested lines of free functions; methods are inside an impl we track.
            i = end + 1 if not in_owner else i + 1
            continue
        i += 1
    return fns, types, len(text.splitlines())


def resolve_calls(fns):
    by_qual = {}
    for f in fns:
        by_qual.setdefault(f["qual"], []).append(f)
    by_name_free = {}
    for f in fns:
        if not f["owner"]:
            by_name_free.setdefault(f["name"], []).append(f)
    trait_names = {f["owner"] for f in fns if f["owner"] and any(
        g["trait"] == f["owner"] for g in fns)}

    def lookup(qual, crate=None):
        cands = by_qual.get(qual, [])
        if crate:
            same = [c for c in cands if c["crate"] == crate]
            if same:
                return same
        return cands

    for f in fns:
        body = re.sub(r"//.*", "", f.pop("_body"))
        module = f.pop("_module")
        recv = dict(RECEIVERS["*"])
        recv.update(RECEIVERS.get(f["file"], {}))
        calls = []

        def add(targets):
            for t in targets:
                if t["id"] != f["id"] and t["id"] not in calls:
                    calls.append(t["id"])

        # self.method(  -> Owner::method
        for m in re.finditer(r"\bself\.(\w+)\(", body):
            if f["owner"]:
                add(lookup(f"{f['owner']}::{m.group(1)}", f["crate"]))
        # recv.method( / self.field.method(
        for m in re.finditer(r"\b(?:self\.)?(\w+)\.(\w+)\(", body):
            var, meth = m.groups()
            if var == "self":
                continue
            ty = recv.get(var)
            if ty:
                add(lookup(f"{ty}::{meth}"))
        # Type::func( or module::func( or other_crate::func(
        for m in re.finditer(r"\b([A-Za-z_]\w*)::(\w+)\(", body):
            prefix, name = m.groups()
            add(lookup(f"{prefix}::{name}"))
            crate = prefix.replace("_", "-")
            add([g for g in by_qual.get(name, []) if g["crate"] == crate])
        # bare func(
        for m in re.finditer(r"(?<![\w.:])([a-z_]\w*)\(", body):
            name = m.group(1)
            cands = by_name_free.get(name, [])
            local = [c for c in cands if c["crate"] == f["crate"] and c["file"] == f["file"]]
            if local:
                add(local)
            elif len(cands) == 1:
                add(cands)
        f["calls"] = calls

    # Trait impls without their own docs inherit the trait method's doc.
    decls = {(f["owner"], f["name"]): f for f in fns if f["decl_only"]}
    std_docs = {
        "Default": "Default value (std `Default` trait).",
        "Display": "Text form, used by `to_string()` (std `Display` trait).",
        "Debug": "Debug formatting (std `Debug` trait).",
        "Drop": "Runs automatically when the value goes out of scope (std `Drop` trait).",
    }
    for f in fns:
        if not f["doc"] and f["trait"]:
            d = decls.get((f["trait"], f["name"]))
            f["doc"] = d["doc"] if d else std_docs.get(f["trait"], "")

    ids = {f["id"]: f for f in fns}
    for f in fns:
        f["called_by"] = []
    for f in fns:
        for c in f["calls"]:
            ids[c]["called_by"].append(f["id"])
    # Trait declarations: link to implementations.
    for f in fns:
        if f["decl_only"] or (f["owner"] in trait_names and not f["trait"]):
            f["implemented_by"] = [
                g["id"] for g in fns if g["trait"] == f["owner"] and g["name"] == f["name"]
            ]
    return fns


def main():
    crates = {}
    fns, types = [], []
    for cdir in sorted((ROOT / "crates").iterdir()):
        if not (cdir / "Cargo.toml").exists():
            continue
        name = cdir.name
        info = crate_info(cdir)
        files = []
        for path in sorted((cdir / "src").rglob("*.rs")):
            rel = str(path.relative_to(ROOT))
            f, t, n = parse_file(name, path, rel)
            fns += f
            types += t
            files.append({"path": rel, "lines": n})
        info["files"] = files
        crates[name] = info
    for c in crates.values():
        c["used_by"] = []
    for name, c in crates.items():
        for d in c["deps"]:
            if d in crates:
                crates[d]["used_by"].append(name)
    for c in crates.values():
        # sdk: a library no vox crate uses (compiled into plugins, not into vox).
        c["layer"] = (
            "binary" if c["binary"]
            else "sdk" if not c["used_by"]
            else "adapter" if c["third_party"]
            else "core"
        )

    # std `Debug` formatting impls are noise in the tour.
    fns = [f for f in fns if f["trait"] != "Debug"]
    fns = resolve_calls(fns)
    ids = {f["id"] for f in fns} | {t["id"] for t in types}

    flows = json.loads(FLOWS.read_text())
    missing = [
        (fl["title"], st["fn"]) for fl in flows for st in fl["steps"] if st["fn"] not in ids
    ]
    if missing:
        for title, fid in missing:
            print(f"error: flow '{title}' references unknown item {fid}", file=sys.stderr)
        sys.exit(1)

    data = {"crates": crates, "functions": fns, "types": types, "flows": flows}
    out = (
        "// Generated by scripts/gen-code-tour.py. Do not edit by hand.\n"
        "window.VOX_TOUR = " + json.dumps(data, indent=1, ensure_ascii=False) + ";\n"
    )
    if "--check" in sys.argv:
        if not OUT.exists() or OUT.read_text() != out:
            print("error: docs/src/tour/data.js is out of date; run scripts/gen-code-tour.py",
                  file=sys.stderr)
            sys.exit(1)
        print("ok: code tour data is up to date")
        return
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(out)
    print(f"wrote {OUT.relative_to(ROOT)}: {len(crates)} crates, {len(fns)} functions, "
          f"{len(types)} types, {len(flows)} flows")


if __name__ == "__main__":
    main()
