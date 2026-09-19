#!/usr/bin/env python3
from datetime import datetime
from string import Template
from pathlib import Path
import os
import re
import argparse
import unicodedata

AMMONITE_DIR = Path(__file__).absolute().parent.parent
TASK_TEMPLATE_PATH = AMMONITE_DIR / "templates" / "task.md"
USER_DIR = AMMONITE_DIR.parent
USER_TASKS_DIR = USER_DIR / "tasks"


def main():
    args = parse_args()

    task_name = " ".join(args.task_name)
    task_path = USER_TASKS_DIR / task_name_to_filename(task_name)
    if os.path.exists(task_path):
        print(f"ERROR: a task already exists at path '{task_path}'")
        exit(1)

    task_template = read_template(TASK_TEMPLATE_PATH)
    creation_date = datetime.now().astimezone().strftime("%Y-%m-%d %H:%M:%S")
    description = prompt_for_task_description()
    task_link = input("Link to issue (e.g. Jira). If none, press Enter: ")
    if task_link:
        task_link = f"- [Link to task]({task_link})\n"

    contents = task_template.substitute(
        {
            "TASK_NAME": task_name,
            "CREATION_DATE": creation_date,
            "DESCRIPTION": description,
            "TASK_LINK": task_link,
        }
    )

    USER_TASKS_DIR.mkdir(parents=True, exist_ok=True)
    write_task(task_path, contents)
    print(f"New task created at {task_path}")


def parse_args():
    parser = argparse.ArgumentParser(description="Create a new task")
    parser.add_argument(
        "task_name", nargs="+", help="Task name (no need to quote if it has spaces)"
    )
    return parser.parse_args()


def read_template(path: Path):
    with open(path, "r", encoding="utf-8") as f:
        return Template(f.read())


def prompt_for_task_description():
    print("Enter task's description. When done, write a line containing only '.'")
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


def task_name_to_filename(name: str, replacement: str = "_") -> str:
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
    return name or "unnamed"


def write_task(path: Path, contents: str):
    with open(path, "x") as f:
        f.write(contents)


if __name__ == "__main__":
    main()
