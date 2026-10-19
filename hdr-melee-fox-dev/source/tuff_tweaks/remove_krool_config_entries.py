import json, sys
path = sys.argv[1]
with open(path, encoding="utf-8") as f:
    cfg = json.load(f)

def removed(p):
    return p.startswith("fighter/krool/motion/") or p.endswith("/c02attacklw3.nuanmb") and p.startswith("fighter/dedede/motion/body/")

n_share = n_new = 0
for section in ("share-to-added", "share-to-vanilla"):
    s = cfg.get(section, {})
    for k in [k for k in s if removed(k)]:
        del s[k]; n_share += 1
nd = cfg.get("new-dir-files", {})
for k in list(nd):
    before = len(nd[k])
    nd[k] = [p for p in nd[k] if not removed(p)]
    n_new += before - len(nd[k])
    if not nd[k]:
        del nd[k]
with open(path, "w", encoding="utf-8", newline="\n") as f:
    json.dump(cfg, f, indent=4)
    f.write("\n")
print(f"removed {n_share} share entries, {n_new} new-dir-files paths")
