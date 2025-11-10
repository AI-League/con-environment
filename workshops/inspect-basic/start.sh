#!/bin/bash
source /home/coder/venv/bin/activate
exec code-server --bind-addr 0.0.0.0:9000 --auth none --disable-telemetry /home/coder/workspace