"""Check relative Markdown file links; external sites and anchors are out of scope."""
import pathlib
import re
import subprocess
import sys

root = pathlib.Path(__file__).resolve().parent.parent
paths = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z"], cwd=root).decode().split("\0")
errors = []
for name in set(paths):
    path = root / name
    if path.suffix != ".md" or not path.is_file():
        continue
    for link in re.findall(r"\]\(([^\s)]+)\)", path.read_text()):
        if ":" in link or link.startswith("#"):
            continue
        target = link.split("#", 1)[0]
        if target and not (path.parent / target).exists():
            errors.append(f"{name}: missing {target}")
print("\n".join(errors) if errors else "Relative Markdown file links: PASS")
sys.exit(bool(errors))
