import subprocess
import sys
import os

commands = [
    ["python", "scripts/build_flavors.py", "small-personal", "--with-gpu", "--reuse-data"],
    ["python", "scripts/build_flavors.py", "small-personal", "--with-gpu", "--only-vulkan", "--reuse-data"],
    ["python", "scripts/build_flavors.py", "small-personal", "--with-gpu", "--only-cuda", "--reuse-data"],
    ["python", "scripts/build_flavors.py", "medium-personal", "--with-gpu", "--reuse-data"],
    ["python", "scripts/build_flavors.py", "medium-personal", "--with-gpu", "--only-vulkan", "--reuse-data"],
    ["python", "scripts/build_flavors.py", "medium-personal", "--with-gpu", "--only-cuda", "--reuse-data"],
]

for i, cmd in enumerate(commands):
    print(f"\n==================================================")
    print(f"RUNNING BUILD {i+1}/6: {' '.join(cmd)}")
    print(f"==================================================")
    rc = subprocess.call(cmd)
    if rc != 0:
        print(f"ERROR: Command {' '.join(cmd)} failed with exit code {rc}")
        sys.exit(rc)

print("\nSUCCESS: All 6 installer flavors built successfully!")
