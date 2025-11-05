# Command Line Usage

This document contains the help content for the `ixa` command-line program.

## `ixa`

Default cli arguments for ixa runner

**Usage:** `ixa [OPTIONS]`

###### **Options:**

* `-r`, `--random-seed <RANDOM_SEED>` — Random seed

  Default value: `0`
* `-c`, `--config <CONFIG>` — Optional path for a global properties config file
* `-o`, `--output <OUTPUT_DIR>` — Optional path for report output
* `--prefix <FILE_PREFIX>` — Optional prefix for report files
* `-f`, `--force-overwrite` — Overwrite existing report files?
* `-l`, `--log-level <LOG_LEVEL>` — Enable logging
* `-v`, `--verbose` — Increase logging verbosity (-v, -vv, -vvv, etc.)

   | Level   | ERROR | WARN | INFO | DEBUG | TRACE |
   |---------|-------|------|------|-------|-------|
   | Default |   ✓   |      |      |       |       |
   | -v      |   ✓   |  ✓   |  ✓   |       |       |
   | -vv     |   ✓   |  ✓   |  ✓   |   ✓   |       |
   | -vvv    |   ✓   |  ✓   |  ✓   |   ✓   |   ✓   |
* `--warn` — Set logging to WARN level. Shortcut for `--log-level warn`
* `--debug` — Set logging to DEBUG level. Shortcut for `--log-level DEBUG`
* `--trace` — Set logging to TRACE level. Shortcut for `--log-level TRACE`
* `-d`, `--debugger <DEBUGGER>` — Set a breakpoint at a given time and start the debugger. Defaults to t=0.0
* `-w`, `--web <WEB>` — Enable the Web API at a given time. Defaults to t=0.0
* `-t`, `--timeline-progress-max <TIMELINE_PROGRESS_MAX>` — Enable the timeline progress bar with a maximum time
* `--no-stats` — Suppresses the printout of summary statistics at the end of the simulation



