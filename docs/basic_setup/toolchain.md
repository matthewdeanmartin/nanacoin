# Toolchain Setup

ESP-IDF is not a single program. It is a C SDK, a CMake build system, a
cross-compiler, a Python virtualenv full of tooling, and a set of environment
variables that glue them together. **Nothing works until those variables are
set**, and they are set per terminal window.

Almost every "it doesn't work" on Windows traces back to that.

## Use PowerShell, not Git Bash

Not a preference. `idf_tools.py` contains this:

```python
if 'MSYSTEM' in os.environ:
    fatal('MSys/Mingw is not supported. ...')
    raise SystemExit(1)
```

Git Bash always sets `MSYSTEM`. **ESP-IDF cannot be built from Git Bash.**

Worse, `MSYSTEM` is inherited by child processes. A PowerShell window launched
from Git Bash — or from an IDE that was itself launched from Git Bash — carries
it, and fails the same way while looking like a clean PowerShell session.

Check any window before trusting it:

```powershell
"MSYSTEM = [$env:MSYSTEM]"
```

Want `MSYSTEM = []`. Anything else, clear it:

```powershell
Remove-Item Env:MSYSTEM -ErrorAction SilentlyContinue
```

## Activating

The installer ships `export.ps1`, which sets the variables. It must be
**dot-sourced** — run with a leading `.` and a space — so the variables land in
*your* shell rather than a child process that immediately exits:

```powershell
. C:\Espressif\frameworks\esp-idf-v5.5.3\export.ps1
```

!!! danger "The leading dot is part of the command"

    Without it, the script runs in a subprocess, prints a cheerful
    `Done! You can now compile ESP-IDF projects.`, and changes **nothing** in
    your shell. This failure is silent and looks exactly like success.

The installer also creates a Start-menu shortcut (**ESP-IDF PowerShell**) and
`C:\Espressif\idf_cmd_init.bat`. Both open a pre-activated shell and sidestep
all of this. If a hand-rolled script misbehaves, fall back to the shortcut —
it is the vendor-supported path.

## The wrapper script

This repo has `idf-env.ps1` at its root, which handles the environment problems
found on this machine:

```powershell
. C:\github\microcontroller\idf-env.ps1
```

It does four things:

1. **Clears `MSYSTEM`**, so an inherited value cannot abort the build
2. **Strips Git Bash's unix tools from PATH**, which otherwise confuse CMake
3. **Puts the bundled Python 3.11 ahead of the system Python** during activation
4. **Verifies the result** and warns loudly if `python` resolves outside the
   venv

Expected output ends with:

```text
Done! You can now compile ESP-IDF projects.
```

## Verifying activation

`Done!` is not sufficient proof. Check what `python` resolves to:

```powershell
(Get-Command python).Source
```

| Result | Meaning |
|--------|---------|
| `C:\Espressif\python_env\idf5.5_py3.11_env\Scripts\python.exe` | Correct — the virtualenv |
| `C:\Espressif\tools\idf-python\3.11.2\python.exe` | Wrong — bare interpreter, no packages |
| `...\AppData\Local\Programs\Python\Python312\python.exe` | Wrong — never activated |

This matters because `idf.py` begins with `#!/usr/bin/env python`, so it runs
under **whichever `python` is first on PATH**. Point that at the wrong
interpreter and you get:

```text
No module named 'click'
This usually means that "idf.py" was not spawned within an ESP-IDF shell
environment or the python virtual environment used by "idf.py" is corrupted.
```

The second half of that message is usually a red herring. The virtualenv is
rarely corrupt. Almost always it is the first half: wrong shell environment,
wrong Python.

## Why two Pythons

The installer lays down two separate Python installations, and the distinction
causes real confusion:

| Path | What it is |
|------|------------|
| `C:\Espressif\tools\idf-python\3.11.2\` | The bare interpreter. **No packages.** |
| `C:\Espressif\python_env\idf5.5_py3.11_env\` | The virtualenv. Has `click`, `esptool`, everything. |

The bare interpreter exists only to *create* the virtualenv. The virtualenv is
what actually runs the tools. If the bare one ends up ahead on PATH, everything
resolves to an interpreter with no packages installed.

!!! note "A version-mismatch trap"

    If a system-wide Python 3.12 is installed, `export.ps1` may detect it,
    then look for `idf5.5_py3.12_env` and report:

    ```text
    ERROR: ESP-IDF Python virtual environment
    "C:\Espressif\python_env\idf5.5_py3.12_env\Scripts\python.exe" not found.
    Please run the install script to set it up before proceeding.
    ```

    The install is **not** broken. The venv exists as `idf5.5_py3.11_env`; the
    wrong Python was detected. Putting the bundled 3.11 first on PATH before
    activating resolves it — which is what `idf-env.ps1` does.

## Per-window, every window

Activation lasts exactly as long as the terminal window. Open a new one
tomorrow, run `idf.py`, and get `not recognized as the name of a cmdlet` — that
is not a regression, just an unactivated shell.

```powershell
. C:\github\microcontroller\idf-env.ps1
```
