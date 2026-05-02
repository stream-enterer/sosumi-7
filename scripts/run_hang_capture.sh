#!/usr/bin/env bash
# Phase B capture launcher. Opens /tmp/em_instr.log on fd 9 in append
# mode and passes its number to the binary via EM_INSTR_FD. The shell
# (and any cdylib that calls libc::write on the same fd) sees one
# kernel file description, so O_APPEND atomic-write semantics apply
# across all writers.
set -euo pipefail

LOG=/tmp/em_instr.log
rm -f "$LOG"
exec 9>>"$LOG"
EM_INSTR_FD=9 exec ./target/release/eaglemode "$@"
