#!/bin/env python3
from asyncio import subprocess
from subprocess import run, Popen, PIPE
import shutil
import os
import base64

run("cargo build --release --example demo", shell=True, check=False)
for file in os.listdir("images"):
    if file.endswith(".html"):
        os.remove("images/" + file)

demos = [("simple", 1), ("simple2", 1), ("multiple", 5), ("split_weighted", 1), ("abandonment", 3),
         ("print_during_progress", 11), ("split_each", 1), ("split_summed", 1), ("split_sized", 1), ("indeterminate", 1)]

svgs = [
    Popen(["svg-term", "--command", f"../target/release/examples/demo {demo}",  "--no-cursor",
           "--width", "60", "--height", str(height)], stdout=PIPE)
    for (demo, height) in demos
]

for ((demo, height), svg) in zip(demos, svgs):
    assert svg.stdout is not None
    out = svg.stdout.read()
    svg.wait()
    encoded = base64.b64encode(out)
    with open("images/" + demo + ".html", "wb") as f:
        f.write(b"<img src=\"data:image/svg+xml;base64,")
        f.write(encoded)
        f.write(b"\" />")

# printf "<img src=\"data:image/svg+xml;base64,%s\" />" "$(svg-term --command "../target/release/examples/demo $i" --no-cursor --width 60 --height 1 | base64 --wrap 0)" > "images/$i.svg"&
