#!/usr/bin/env python3
from datetime import datetime
from string import Template
from pathlib import Path
import os
import re
import argparse
import unicodedata
import subprocess

AMMONITE_DIR = Path(__file__).absolute().parent.parent
TEMPLATE_DOSSIER_PATH = AMMONITE_DIR / "templates" / "dossier.md"
USER_DIR = AMMONITE_DIR.parent
USER_DOSSIERS_DIR = USER_DIR / "dossiers"


def main():
    args = parse_args()

    dossier_name = " ".join(args.dossier_name)
    dossier_path = USER_DOSSIERS_DIR / dossier_name_to_filename(dossier_name)
    if os.path.exists(dossier_path):
        print(f"ERROR: a dossier already exists at path '{dossier_path}'")
        exit(1)

    dossier_template = read_template(TEMPLATE_DOSSIER_PATH)
    creation_date = datetime.now().astimezone().strftime("%Y-%m-%d %H:%M:%S")
    description = prompt_for_dossier_description()
    link = input("Link to issue (e.g. Jira). If none, press Enter: ")
    if link:
        link = f"- [Link to task]({link})\n"

    contents = dossier_template.substitute(
        {
            "DOSSIER_NAME": dossier_name,
            "CREATION_DATE": creation_date,
            "DESCRIPTION": description,
            "DOSSIER_LINK": link,
        }
    )

    USER_DOSSIERS_DIR.mkdir(parents=True, exist_ok=True)
    write_dossier(dossier_path, contents)
    git_commit(dossier_path, dossier_name)


def parse_args():
    parser = argparse.ArgumentParser(description="Create a new dossier")
    parser.add_argument(
        "name", nargs="+", help="Dossier name (no need to quote if it has spaces)"
    )
    return parser.parse_args()


def read_template(path: Path):
    with open(path, "r", encoding="utf-8") as f:
        return Template(f.read())


def prompt_for_dossier_description():
    print("Enter dossier's description. When done, write a line containing only '.'")
    lines = []
    while True:
        try:
            line = input()
        except EOFError:
            break
        if line == ".":
            break
        lines.append(line)

    return "\n".join(lines)


def dossier_name_to_filename(name: str, replacement: str = "_") -> str:
    WINDOWS_RESERVED = {
        "CON",
        "PRN",
        "AUX",
        "NUL",
        *(f"COM{i}" for i in range(1, 10)),
        *(f"LPT{i}" for i in range(1, 10)),
    }

    # Normalize and strip non-ASCII
    name = unicodedata.normalize("NFKD", name)
    name = name.encode("ascii", "ignore").decode("ascii")

    # Replace anything that is not a safe character
    name = re.sub(r"[^A-Za-z0-9._-]+", replacement, name)

    # Remove leading/trailing dots and spaces (Windows)
    name = name.strip(" .")

    # Avoid Windows reserved device names
    stem = name.split(".")[0].upper()
    if stem in WINDOWS_RESERVED:
        name = f"_{name}"

    # Avoid an empty result
    name = name or "unnamed"
    return f"{name}.md"


def write_dossier(path: Path, contents: str):
    with open(path, "x") as f:
        f.write(contents)


def git_commit(path: Path, dossier_name: str):
    subprocess.run(["git", "add", path])
    subprocess.run(
        ["git", "commit", "--only", path, "-m", f"New dossier {dossier_name}"]
    )


if __name__ == "__main__":
    main()
