#!/bin/sh
# Native Sway IPC captures the destination and summons the existing window.
exec "${1:-cliprust}" --show
