
# Structure & Objective

This repo sets up a local, virtual k8 cluster with [talos](https://docs.siderolabs.com/talos/v1.11/overview/what-is-talos) that is close to the production hardware we use for conferences. The objective is to have all the infrastructure-as-code here to set up a talos cluster from scratch with random hardware by running a few commands.

In setup we have:

- k8: charts + helm charts
- nix: environment to have all the cli tools and a non-poluted k8 cluster
- scripts: setup scripts we need for configuring the cluster
- talos: patches for the talos cluster
- tests: Testing scripts to make sure the cluster is running.



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
OPENAI_API_KEY=         # Optional
GEMINI_API_KEY=         # Optional
ANTHROPIC_API_KEY=      # Optional

GITHUB_USERNAME=        # Required for k8 env
GHCR_PAT=               # Required for k8 env
DH_UNAME=               # Best to have for k8 env
DH_PAT=                 # Best to have for k8 env
```

## Nix & Qemu

Make sure you have nix installed and can run flakes. If you're running NixOs, you know what to do. On Arch use pacman,
On MacOS install it from [Determinate Systems](https://docs.determinate.systems/). Once it's installed edit `.config/nix/nix.conf` to have:
```
experimental-features = nix-command flakes
```
This allows for the experimental flake to be used without complaint. 

We use talosctl features that require `qemu`. As this might require kernel features, we leave it up to you. 

## State

In development, there should be no valuable state inside the k8 cluster. It downloads and configures and then we delete it and relaunch. This is slow as we need to wait for the cluster to boot and then for the 

To speed up development a little, the dev shell is configured to use container repository proxies that store the images in `.data/repo`. These are containers that run alongside the virtual talos cluster. Currently we have: docker k8s gcr ghcr and quay. 

### Crashes

If your process compose session crashes while you've got the cluster running you can continue to work with it. However, the shutdown/clean up process will not occur. The next time you need to boot "nix run" it will probably not work. In a dev shell you can destroy the cluster and then delete the `.data/talos` (you will need sudo). Destroying the repository proxies is something like this:
```
docker rm registry-docker registry-k8s registry-gcr registry-ghcr registry-quay
```

Worst case scenario is: delete `.data` and reboot your machine. 