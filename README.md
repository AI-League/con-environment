

# Development

First go to this directory, then run:
```sh
nix develop
```

Then to start the servers needed for development run `run-services`. There is just one "dev" environment right now.

## Setup

There are 2 environment files, `.env` and `.envhost` that need to be configured. The `.env` file is for environment variables the local rust needs to set before it can run. It has `DATABASE_URL`, which should point to the database you're using. This changes based on the deployment you have. The `.env` is checked in the corresponding nix shell file (default is `nix/dev_shell.nix`).

The other one `.envhost` is where the actual secrets we need are. These depend on where you are and who you are. Not all are needed, the commercial LLM keys are. We detail below how to fill this out.

```shell
OPENAI_API_KEY=
GEMINI_API_KEY=
ANTHROPIC_API_KEY=

GITHUB_USERNAME=
GHCR_PAT=
DH_UNAME=
DH_PAT=
```

## Nix & Qemu

Make sure you have nix installed and can run flakes. If you're running NixOs, you know what to do. On Arch use pacman,
On MacOS install it from [Determinate Systems](https://docs.determinate.systems/). Once it's installed edit `.config/nix/nix.conf` to have:
```
experimental-features = nix-command flakes
```
This allows for the experimental flake to be used without complaint. 

We use talosctl features that require `qemu`. As this might requir kernel features, we leave it up to you. 