# AssemblySim

Event-driven assembly line simulator written in Rust. It models time as discrete events, handles machine/staff availability, and provides a Ratatui terminal UI for monitoring queues, status, and progress.

## What’s Included

- Discrete event engine with priority scheduling and automatic time advancement.
- Machine buckets (multiple identical machines per step) and staff assignment with skills.
- Material pipelines driven by per-step queues; items advance when a step completes.
- Terminal UI (Ratatui) with metrics, per-step queues, and tabbed status for machines/staff.
- JSON-configurable scenarios with item count, machines, staff, and process steps.
- Tests for core simulation components.

## Quick Start

```bash
cargo run -- --config test.json
```

Controls (in the UI):
- `space` – play/pause
- `n` – step once
- `tab` – switch status tab (Machines/Staff)
- `q` – quit

The sim auto-pauses when all items are finished (finished goods == items).

## Config Format (JSON)

```json
{
  "items": 100,
  "machines": [
    { "id": 0, "name": "Dough Station", "staff_required": 1 },
    { "id": 1, "name": "Oven", "staff_required": 1, "count": 2 },
    { "id": 2, "name": "Decorator Table", "staff_required": 1, "count": 2 },
    { "id": 3, "name": "Cooling Rack", "is_automated": true }
  ],
  "staff": [
    { "id": 0, "name": "User1", "role": { "id": 0, "name": "Floater", "machine_ids": [] } },
    { "id": 1, "name": "User2", "role": { "id": 1, "name": "Floater", "machine_ids": [] } }
  ],
  "processes": [
    { "machine_id": 0, "duration": 12 },
    { "machine_id": 1, "duration": 30 },
    { "machine_id": 2, "duration": 15 },
    { "machine_id": 3, "duration": 10 }
  ]
}
```

Notes:
- `items` = number of items to push through all steps.
- Each `processes` entry is a step in order; `machine_id` refers to a bucket in `machines`.
- `count` lets you define multiple identical machines in a bucket.
- If `is_automated` is false (default), staff must be available for the full duration.

## UI Layout

- **Metrics**: elapsed time, machines/staff counts, idle time, finished goods, controls.
- **Status (tabbed)**: Machines (busy/idle, waiting reason) or Staff (busy/idle, waiting).
- **Process Queues**: one card per step showing queue length, busy/total machines, duration.

## Project Structure

- `src/main.rs` – CLI + TUI runner, config loading, pipeline orchestration.
- `src/logger.rs` – logging helper.
- `src/model/` – core simulation types:
  - `time.rs` – event queue, simulator.
  - `staff_scheduling.rs` – production simulator with staff/machines.
  - `machine.rs`, `staff.rs`, `simulation_example.rs`, etc.
- Docs & guides: `START_HERE.md`, `SUMMARY.md`, `TIME_SIMULATION_GUIDE.md`, `VISUAL_GUIDE.md`, `WHAT_CHANGED.md`, `PRACTICAL_EXAMPLES.rs`.

## Running Tests

```bash
cargo test
```

## Key Concepts

- **Discrete event simulation**: jump to next event time instead of ticking every unit.
- **Min-heap ordering**: earliest events processed first.
- **Machine buckets**: multiple identical machines per step.
- **Staff assignment**: respects required staff count and machine skills; released on completion.
- **Material flow**: step completion enqueues the next step; queues drive work, not per-item threads.

## Common Tweaks

- Speed: adjust `tick_rate` and time step logic in `step_simulation` (defaults: 50 ms UI tick, up to +10 mins per sim tick).
- Visualization: edit `draw_process_queues` or `draw_status_tabs` in `main.rs`.
- Config: add machines with `count`, add staff with restricted `machine_ids`, change `items` to scale load.
