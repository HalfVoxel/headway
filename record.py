#!/bin/env python3
from asyncio import subprocess
from subprocess import run, Popen, PIPE
import shutil
import os
import base64
import re

demos = [("simple", 1), ("message", 1), ("multiple", 5), ("split_weighted", 1), ("abandonment", 3),
         ("print_during_progress", 11), ("split_each", 1), ("split_summed", 1), ("split_sized", 1), ("indeterminate", 1)]

# Fill with dummy files to allow cargo to compile everything
for (demo, _) in demos:
    with open("images/" + demo + ".html", "wb") as f:
        f.write(b"temp")

run("cargo build --release --examples", shell=True, check=False)
for file in os.listdir("images"):
    if file.endswith(".html"):
        os.remove("images/" + file)


svgs = [
    Popen(["svg-term", "--command", f"../target/release/examples/{demo}",  "--no-cursor",
           "--width", "60", "--height", str(height)], stdout=PIPE)
    for (demo, height) in demos
]

for ((demo, height), svg) in zip(demos, svgs):
    assert svg.stdout is not None
    out = svg.stdout.read().decode('utf-8')
    svg.wait()

    # Add a delay at the end of the animation
    end_delay = 2
    duration = float(re.search("animation-duration:\s*([\d\.]+)s", out).group(1))
    new_duration = duration + end_delay
    multiplier = duration / new_duration
    out = re.sub(r"}([\d\.]+)%", lambda m: f"}}{round(float(m.group(1))*multiplier, 2)}%", out)
    out = re.sub(r"animation-duration:\s*([\d\.]+)s",
                 lambda m: f"animation-duration: {new_duration}s", out)
    out = re.sub(r"to(\{.*?\})",
                 fr"{round(100*multiplier,2)}%\1 to\1", out)

    # with open("images/" + demo + ".svg", "wb") as f:
    #     f.write(out.encode('utf-8'))

    encoded = base64.b64encode(out.encode('utf-8'))
    with open("images/" + demo + ".html", "wb") as f:
        f.write(b"<img src=\"data:image/svg+xml;base64,")
        f.write(encoded)
        f.write(b"\" />")

# printf "<img src=\"data:image/svg+xml;base64,%s\" />" "$(svg-term --command "../target/release/examples/demo $i" --no-cursor --width 60 --height 1 | base64 --wrap 0)" > "images/$i.svg"&
