mod logger;
mod model;

use std::{
    collections::HashMap,
    env, fs,
    path::Path,
    time::{Duration, Instant},
};

use crossterm::{
    event::{self, Event as CEvent, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use logger::{LogLevel, Logger};
use model::machine::MachineType;
use model::staff::{Role, Staff};
use model::staff_scheduling::ProductionSimulator;
use model::time::{Event, EventType, SimulationTime, Simulator};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::Line,
    widgets::{Block, Borders, Paragraph, Tabs, Wrap},
    Terminal,
};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct SimulationConfig {
    machines: Vec<MachineConfig>,
    staff: Vec<StaffConfig>,
    processes: Vec<ProcessConfig>,
    #[serde(default = "default_items")]
    items: u32,
}

#[derive(Debug, Deserialize)]
struct MachineConfig {
    id: u32,
    name: String,
    #[serde(default)]
    staff_required: Option<u32>,
    #[serde(default)]
    is_automated: Option<bool>,
    /// Number of identical machines in this bucket (e.g., 2 ovens)
    #[serde(default)]
    count: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct StaffConfig {
    id: u32,
    name: String,
    role: RoleConfig,
}

#[derive(Debug, Deserialize)]
struct RoleConfig {
    id: u32,
    name: String,
    #[serde(default)]
    machine_ids: Vec<u32>,
}

#[derive(Debug, Deserialize)]
struct ProcessConfig {
    machine_id: u32,
    #[serde(default)]
    process_id: Option<u32>,
    /// How long the process runs
    duration: u32,
}

fn default_items() -> u32 {
    1
}

fn main() {
    let logger = Logger::new(LogLevel::Debug);
    let args: Vec<String> = env::args().collect();

    if let Some(config_path) = parse_config_path(&args) {
        if let Err(err) = run_tui_with_config(&config_path, &logger) {
            logger.error(&format!("Failed to run simulation from config: {}", err));
            std::process::exit(1);
        }
    } else {
        logger.info("No config file provided - running built-in examples");
        run_examples(&logger);
        logger.info("\nSimulation complete");
    }
}

fn parse_config_path(args: &[String]) -> Option<String> {
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--config" | "-c" => return iter.next().cloned(),
            path => return Some(path.to_string()),
        }
    }
    None
}

struct App {
    production: ProductionSimulator,
    playing: bool,
    tick_rate: Duration,
    last_tick: Instant,
    title: String,
    machine_buckets: HashMap<u32, Vec<u32>>,
    machine_to_bucket: HashMap<u32, u32>,
    job_queues: HashMap<u32, Vec<PendingJob>>,
    steps: Vec<ProcessConfig>,
    items: u32,
    next_pid: u32,
    process_meta: HashMap<u32, (usize, u32)>, // process_id -> (step_index, item_id)
    finished_goods: u32,
    status_tab: usize,
}

fn run_tui_with_config(config_path: &str, logger: &Logger) -> Result<(), Box<dyn std::error::Error>> {
    let LoadedSim {
        production,
        machine_buckets,
        machine_to_bucket,
        steps,
        items,
    } = load_simulation_from_config(config_path, logger)?;
    let mut app = App {
        production,
        playing: true,
        tick_rate: Duration::from_millis(50),
        last_tick: Instant::now(),
        title: format!("AssemblySim - {}", config_path),
        machine_buckets,
        machine_to_bucket,
        job_queues: HashMap::new(),
        steps,
        items,
        next_pid: 0,
        process_meta: HashMap::new(),
        finished_goods: 0,
        status_tab: 0,
    };

    // Seed initial jobs for the first step for all items
    if let Some(first_step) = app.steps.get(0) {
        let bucket = first_step.machine_id;
        let duration = first_step.duration;
        let queue = app.job_queues.entry(bucket).or_default();
        for item_id in 0..app.items {
            queue.push(PendingJob {
                duration,
                step_index: 0,
                item_id,
            });
        }
        try_start_jobs(&mut app, bucket, 0);
    }

    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run_app(&mut terminal, &mut app);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

struct LoadedSim {
    production: ProductionSimulator,
    machine_buckets: HashMap<u32, Vec<u32>>,
    machine_to_bucket: HashMap<u32, u32>,
    steps: Vec<ProcessConfig>,
    items: u32,
}

fn load_simulation_from_config(
    config_path: &str,
    logger: &Logger,
) -> Result<LoadedSim, Box<dyn std::error::Error>> {
    logger.info(&format!("Loading simulation config from {}", config_path));

    let path = Path::new(config_path);
    if !path.exists() {
        return Err(format!("Config file not found at {}", config_path).into());
    }

    let contents = fs::read_to_string(path)?;
    let config: SimulationConfig = serde_json::from_str(&contents)?;

    let mut production = ProductionSimulator::new();
    let mut machine_buckets: HashMap<u32, Vec<u32>> = HashMap::new();
    let mut next_machine_id: u32 = 0;
    let mut machine_to_bucket: HashMap<u32, u32> = HashMap::new();

    for machine_cfg in &config.machines {
        let count = machine_cfg.count.unwrap_or(1);
        for _ in 0..count {
            let machine_id = next_machine_id;
            next_machine_id += 1;

            let machine = if machine_cfg.is_automated.unwrap_or(false) {
                MachineType::automated(machine_id, &machine_cfg.name)
            } else {
                let staff_needed = machine_cfg.staff_required.unwrap_or(1);
                MachineType::new(machine_id, &machine_cfg.name, staff_needed)
            };

            production.add_machine(machine);
            machine_buckets
                .entry(machine_cfg.id)
                .or_default()
                .push(machine_id);
            machine_to_bucket.insert(machine_id, machine_cfg.id);
        }
    }

    for staff_cfg in &config.staff {
        let role = if staff_cfg.role.machine_ids.is_empty() {
            Role::new(staff_cfg.role.id, &staff_cfg.role.name)
        } else {
            Role::specialist(
                staff_cfg.role.id,
                &staff_cfg.role.name,
                staff_cfg.role.machine_ids.clone(),
            )
        };
        let staff = Staff::new(staff_cfg.id, &staff_cfg.name, role);
        production.add_staff(staff);
    }

    Ok(LoadedSim {
        production,
        machine_buckets,
        machine_to_bucket,
        steps: config.processes,
        items: config.items,
    })
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> Result<(), Box<dyn std::error::Error>> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        let timeout = app
            .tick_rate
            .checked_sub(app.last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let CEvent::Key(KeyEvent { code, kind: KeyEventKind::Press, .. }) = event::read()? {
                match code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char(' ') => app.playing = !app.playing,
                    KeyCode::Char('n') => {
                        step_simulation(app);
                    }
                    KeyCode::Tab => {
                        app.status_tab = (app.status_tab + 1) % 2;
                    }
                    KeyCode::BackTab => {
                        app.status_tab = app.status_tab.saturating_sub(1) % 2;
                    }
                    _ => {}
                }
            }
        }

        if app.last_tick.elapsed() >= app.tick_rate {
            if app.playing {
                step_simulation(app);
                if sim_complete(app) {
                    app.playing = false;
                }
            }
            app.last_tick = Instant::now();
        }
    }
}

fn step_simulation(app: &mut App) {
    let current = app.production.simulator.elapsed_time();
    let mut target_time = current + 10;
    if let Some(next_event) = app.production.simulator.peek_next_event() {
        let next_time = next_event.time.as_minutes();
        if next_time > current && next_time < target_time {
            target_time = next_time;
        }
    }

    // Rebalance stuck staff/machines before progressing time
    rebalance(app, target_time);

    // Process all events due up to target_time
    loop {
        if let Some(event) = app.production.simulator.peek_next_event() {
            if event.time.as_minutes() <= target_time {
                let evt = app.production.simulator.step().unwrap();
                handle_event(app, evt);
                continue;
            }
        }
        break;
    }

    // Advance clock to target_time if nothing else happened
    if app.production.simulator.elapsed_time() < target_time {
        app.production
            .simulator
            .set_time(SimulationTime::new(target_time));
    }

    // Rebalance again after time advancement
    rebalance(app, target_time);

    // Continuously attempt to start queued jobs on all buckets
    let buckets: Vec<u32> = app.machine_buckets.keys().cloned().collect();
    for bucket in buckets {
        try_start_jobs(app, bucket, target_time);
    }

    app.production.finalize_idle_time(target_time);
}

fn handle_event(app: &mut App, event: Event) {
    let production = &mut app.production;
    match event.event_type {
        EventType::ProcessComplete {
            machine_id,
            process_id,
        } => {
            if let Some(machine) = production.machines.get_mut(machine_id as usize) {
                // Immediately free any staff still marked on this machine
                let current_time = event.time.as_minutes();
                let releasing: Vec<u32> = machine.assigned_staff.clone();
                for staff_id in releasing {
                    if let Some(staff_member) = production.staff.iter_mut().find(|s| s.id == staff_id)
                    {
                        staff_member.release_from_machine(current_time);
                    }
                }
                machine.is_operating = false;
                machine.assigned_staff.clear();
                machine.waiting_for = Some("Next process".to_string());
            }
            if let Some((step_idx, item_id)) = app.process_meta.remove(&process_id) {
                let next_step = step_idx + 1;
                if let Some(step) = app.steps.get(next_step) {
                    let bucket = step.machine_id;
                    let duration = step.duration;
                    let queue = app.job_queues.entry(bucket).or_default();
                    queue.push(PendingJob {
                        duration,
                        step_index: next_step,
                        item_id,
                    });
                    try_start_jobs(app, bucket, event.time.as_minutes());
                } else {
                    // Finished goods
                    app.finished_goods += 1;
                    // After freeing staff, try to start waiting work anywhere
                    let current_time = event.time.as_minutes();
                    let buckets: Vec<u32> = app.machine_buckets.keys().cloned().collect();
                    for bucket in buckets {
                        try_start_jobs(app, bucket, current_time);
                    }
                }
            }
        }
        EventType::StaffReleased {
            staff_id,
            machine_id,
        } => {
            if let Some(staff_member) = production.staff.iter_mut().find(|s| s.id == staff_id) {
                staff_member.release_from_machine(production.simulator.elapsed_time());
            }
            if let Some(machine) = production.machines.get_mut(machine_id as usize) {
                machine.assigned_staff.retain(|&id| id != staff_id);
                if machine.assigned_staff.is_empty() {
                    machine.is_operating = false;
                    machine.waiting_for = Some("Next process".to_string());
                }
            }

            if let Some(bucket) = app.machine_to_bucket.get(&machine_id).cloned() {
                try_start_jobs(app, bucket, event.time.as_minutes());
            }
        }
        EventType::StaffUnavailable { .. } => {
            // Nothing to update in state, but could surface in UI later
        }
        _ => {}
    }
}

fn rebalance(app: &mut App, current_time: u32) {
    // Free staff whose availability time has passed or whose machine isn't running
    for staff in &mut app.production.staff {
        if !staff.is_available && current_time >= staff.available_at {
            staff.release_from_machine(current_time);
        }
        if !staff.is_available {
            if let Some(machine_id) = staff.current_machine {
                let should_release = app
                    .production
                    .machines
                    .get(machine_id as usize)
                    .map(|m| !m.is_operating || !m.assigned_staff.contains(&staff.id))
                    .unwrap_or(true);
                if should_release {
                    staff.release_from_machine(current_time);
                }
            }
        }
    }

    // Clear any machines marked idle but still holding staff
    for machine in &mut app.production.machines {
        if !machine.is_operating && !machine.assigned_staff.is_empty() {
            for staff_id in machine.assigned_staff.drain(..) {
                if let Some(staff_member) = app.production.staff.iter_mut().find(|s| s.id == staff_id) {
                    staff_member.release_from_machine(current_time);
                }
            }
        }
    }
}

fn sim_complete(app: &App) -> bool {
    app.finished_goods >= app.items
        || (app.job_queues.values().all(|q| q.is_empty())
            && app
                .production
                .machines
                .iter()
                .all(|m| !m.is_operating && m.assigned_staff.is_empty())
            && app
                .production
                .staff
                .iter()
                .all(|s| s.is_available))
}

fn bucket_display_name(app: &App, bucket_id: u32) -> String {
    if let Some(list) = app.machine_buckets.get(&bucket_id) {
        if let Some(first) = list.first() {
            if let Some(machine) = app.production.machines.get(*first as usize) {
                let base = if machine.machine.name.trim().is_empty() {
                    format!("Bucket {}", bucket_id)
                } else {
                    machine.machine.name.clone()
                };
                if list.len() > 1 {
                    return format!("{} (x{})", base, list.len());
                }
                return base;
            }
        }
    }
    format!("Bucket {}", bucket_id)
}

fn draw_process_queues(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let rows = app.steps.len().max(1) as u16;
    let constraints: Vec<Constraint> = (0..rows).map(|_| Constraint::Length(5)).collect();
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    for (idx, step) in app.steps.iter().enumerate() {
        if idx >= areas.len() {
            break;
        }
        let bucket = step.machine_id;
        let name = bucket_display_name(app, bucket);
        let queue_len = app.job_queues.get(&bucket).map(|q| q.len()).unwrap_or(0);
        let busy_machines = app
            .machine_buckets
            .get(&bucket)
            .map(|ids| {
                ids.iter()
                    .filter(|&&id| app.production.machines.get(id as usize).map(|m| m.is_operating).unwrap_or(false))
                    .count()
            })
            .unwrap_or(0);
        let bucket_size = app
            .machine_buckets
            .get(&bucket)
            .map(|list| list.len())
            .unwrap_or(1);

        let text = vec![
            Line::from(format!("Queue: {}", queue_len)),
            Line::from(format!("Machines busy: {} / {}", busy_machines, bucket_size)),
            Line::from(format!("Duration: {} mins", step.duration)),
        ];
        let block = Block::default()
            .borders(Borders::ALL)
            .title(name)
            .style(Style::default().fg(Color::White));
        let para = Paragraph::new(text)
            .style(Style::default().fg(Color::White))
            .block(block);
        f.render_widget(para, areas[idx]);
    }
}

fn draw_status_tabs(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let tabs_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
        .split(area);

    let titles = vec![Line::from("Machines"), Line::from("Staff")];
    let tabs = Tabs::new(titles)
        .select(app.status_tab)
        .block(Block::default().borders(Borders::ALL).title("Status"))
        .highlight_style(Style::default().fg(Color::Yellow));
    f.render_widget(tabs, tabs_area[0]);

    match app.status_tab {
        0 => {
            let mut machine_lines = Vec::new();
            for machine in &app.production.machines {
                let status = if machine.is_operating { "Busy" } else { "Idle" };
                let waiting = machine
                    .waiting_for
                    .as_deref()
                    .unwrap_or(if machine.is_operating { "" } else { "Next task" });
                let name = app
                    .machine_to_bucket
                    .get(&machine.machine.id)
                    .map(|b| bucket_display_name(app, *b))
                    .unwrap_or_else(|| format!("Machine {}", machine.machine.id));
                machine_lines.push(Line::from(format!(
                    "{} (ID {}): {} | Waiting: {}",
                    name,
                    machine.machine.id,
                    status,
                    if waiting.is_empty() { "-" } else { waiting }
                )));
            }
            let para = Paragraph::new(machine_lines)
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true });
            f.render_widget(para, tabs_area[1]);
        }
        _ => {
            let mut staff_lines = Vec::new();
            for staff in &app.production.staff {
                let status = if staff.is_available { "Available" } else { "Busy" };
                let waiting = if staff.is_available {
                    "Assignment".to_string()
                } else {
                    staff
                        .current_machine
                        .map(|m| format!("Machine {}", m))
                        .unwrap_or_else(|| "Task".to_string())
                };
                staff_lines.push(Line::from(format!(
                    "{} - {}: {} | Idle: {} | Waiting: {}",
                    staff.id,
                    staff.name,
                    status,
                    staff.idle_time,
                    waiting
                )));
            }
            let para = Paragraph::new(staff_lines)
                .block(Block::default().borders(Borders::ALL))
                .wrap(Wrap { trim: true });
            f.render_widget(para, tabs_area[1]);
        }
    }
}
fn draw_ui(f: &mut ratatui::Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(35), Constraint::Percentage(65)].as_ref())
        .split(f.size());

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(12), Constraint::Min(0)].as_ref())
        .split(chunks[0]);

    draw_metrics(f, left[0], app);
    draw_status_tabs(f, left[1], app);
    draw_process_queues(f, chunks[1], app);
}

fn draw_metrics(f: &mut ratatui::Frame, area: Rect, app: &App) {
    let elapsed = app.production.simulator.elapsed_time();
    let operating = app
        .production
        .machines
        .iter()
        .filter(|m| m.is_operating)
        .count();
    let total_idle: u32 = app.production.staff.iter().map(|s| s.idle_time).sum();
    let playing_text = if app.playing { "Playing" } else { "Paused" };

    let lines = vec![
        Line::from(app.title.clone()),
        Line::from(format!("Mode: {}", playing_text)),
        Line::from(format!("Elapsed: {} mins", elapsed)),
        Line::from(format!(
            "Machines: {} total | {} active",
            app.production.machines.len(),
            operating
        )),
        Line::from(format!("Staff: {}", app.production.staff.len())),
        Line::from(format!("Total idle mins: {}", total_idle)),
        Line::from(format!("Finished goods: {}", app.finished_goods)),
        Line::from("Controls:"),
        Line::from("  space - play/pause"),
        Line::from("  n     - step once"),
        Line::from("  tab   - switch status tab"),
        Line::from("  q     - quit"),
    ];

    let metrics = Paragraph::new(lines)
        .block(Block::default().borders(Borders::ALL).title("Metrics"))
        .wrap(Wrap { trim: true });

    f.render_widget(metrics, area);
}


#[derive(Clone)]
struct PendingJob {
    duration: u32,
    step_index: usize,
    item_id: u32,
}

fn try_start_jobs(app: &mut App, bucket_id: u32, current_time: u32) {
    let Some(queue) = app.job_queues.get_mut(&bucket_id) else { return };
    if queue.is_empty() {
        return;
    }

    let Some(machine_ids) = app.machine_buckets.get(&bucket_id) else { return };

    // Try to start as many queued jobs as there are free machines and staff
    while !queue.is_empty() {
        // pick the job furthest along in the process (highest step_index)
        let best_idx = queue
            .iter()
            .enumerate()
            .max_by_key(|(_, job)| (job.step_index, std::cmp::Reverse(job.item_id)))
            .map(|(idx, _)| idx)
            .unwrap();

        // find an idle machine in this bucket
        let Some(&machine_id) = machine_ids
            .iter()
            .find(|&&m_id| app.production.machines.get(m_id as usize).map(|m| !m.is_operating).unwrap_or(false))
        else {
            break; // no idle machines
        };

        let job = queue.remove(best_idx);
        let pid = app.next_pid;
        app.next_pid += 1;
        app.process_meta.insert(pid, (job.step_index, job.item_id));

        let started = app
            .production
            .try_start_process(machine_id, pid, job.duration, current_time);

        if started {
            if let Some(machine) = app.production.machines.get_mut(machine_id as usize) {
                machine.waiting_for = None;
            }
        } else {
            // Could not start (likely staff unavailable) — mark machine as waiting for staff and requeue
            if let Some(machine) = app.production.machines.get_mut(machine_id as usize) {
                machine.waiting_for = Some("Staff".to_string());
            }
            queue.push(job);
            break;
        }
    }
}

fn run_examples(logger: &Logger) {
    logger.debug("Application started");
    logger.info("System initialized");

    // ============================================================
    // Example 1: Simple simulation without staff
    // ============================================================
    logger.info("\n=== Example 1: Event Scheduling (without staff) ===");
    {
        let mut sim = Simulator::new();

        sim.schedule_event(
            SimulationTime::new(10),
            EventType::ProcessStart {
                machine_id: 0,
                process_id: 1,
            },
        );
        sim.schedule_event(
            SimulationTime::new(25),
            EventType::ProcessComplete {
                machine_id: 0,
                process_id: 1,
            },
        );
        sim.schedule_event(
            SimulationTime::new(5),
            EventType::MaterialArrival { material_id: 1 },
        );

        logger.info("Events scheduled, starting simulation...");
        sim.run_all(|sim, event| {
            logger.info(&format!("Time {}: {:?}", sim.elapsed_time(), event.event_type));
        });
    }

    // ============================================================
    // Example 2: Production with staff scheduling
    // ============================================================
    logger.info("\n=== Example 2: Staff Scheduling ===");
    {
        let mut prod = ProductionSimulator::new();

        // Create machines
        logger.info("Setting up production line...");
        let cnc_machine = MachineType::new(0, "CNC Machine", 1); // Needs 1 staff
        let assembly = MachineType::new(1, "Assembly Station", 2); // Needs 2 staff
        let conveyor = MachineType::automated(2, "Conveyor Belt"); // Automated, no staff

        prod.add_machine(cnc_machine);
        prod.add_machine(assembly);
        prod.add_machine(conveyor);

        // Create staff
        logger.info("Hiring staff...");

        // General operator (can work anywhere)
        let operator_role = Role::new(0, "General Operator");
        let john = Staff::new(0, "John", operator_role.clone());
        prod.add_staff(john);

        // CNC specialist (can only work on CNC)
        let cnc_specialist_role = Role::specialist(1, "CNC Specialist", vec![0]);
        let jane = Staff::new(1, "Jane", cnc_specialist_role);
        prod.add_staff(jane);

        // Assembly workers
        let assembler_role = Role::new(2, "Assembler");
        let bob = Staff::new(2, "Bob", assembler_role.clone());
        let alice = Staff::new(3, "Alice", assembler_role);
        prod.add_staff(bob);
        prod.add_staff(alice);

        logger.info(&prod.get_status());

        // ============================================================
        // Schedule production: Item 0 through CNC -> Assembly -> Conveyor
        // ============================================================
        logger.info("\n--- Scheduling Item 0 ---");

        // Item 0: CNC (needs Jane)
        let success = prod.try_start_process(0, 0, 15, 0);
        if success {
            logger.info("Item 0: CNC process started (Jane assigned)");
        } else {
            logger.warning("Item 0: CNC process failed - staff unavailable");
        }

        // After CNC completes (time 15), try assembly
        // But we need to manually release Jane and assign Bob+Alice
        prod.staff[1].release_from_machine(15); // Release Jane

        // ============================================================
        // Schedule production: Item 1 - parallel processing
        // ============================================================
        logger.info("\n--- Scheduling Item 1 (Parallel) ---");

        // Item 1 on CNC at time 15 (Jane just became available)
        let success = prod.try_start_process(0, 1, 15, 15);
        if success {
            logger.info("Item 1: CNC process started at time 15 (Jane assigned)");
        }

        // Item 0 on Assembly at time 15 (needs Bob and Alice)
        let success = prod.try_start_process(1, 0, 20, 15);
        if success {
            logger.info("Item 0: Assembly started at time 15 (Bob & Alice assigned)");
        } else {
            logger.warning("Item 0: Assembly failed - staff unavailable");
        }

        // Run the simulation
        logger.info("\n--- Running Simulation ---");
        prod.simulator.run_all(|sim, event| match &event.event_type {
            EventType::ProcessStart {
                machine_id,
                process_id,
            } => {
                logger.info(&format!(
                    "Time {}: Process {} started on machine {}",
                    sim.elapsed_time(),
                    process_id,
                    machine_id
                ));
            }
            EventType::ProcessComplete {
                machine_id,
                process_id,
            } => {
                logger.info(&format!(
                    "Time {}: Process {} completed on machine {}",
                    sim.elapsed_time(),
                    process_id,
                    machine_id
                ));
            }
            EventType::StaffAssigned {
                staff_id,
                machine_id,
                ..
            } => {
                logger.info(&format!(
                    "Time {}: Staff {} assigned to machine {}",
                    sim.elapsed_time(),
                    staff_id,
                    machine_id
                ));
            }
            EventType::StaffReleased {
                staff_id,
                machine_id,
            } => {
                logger.info(&format!(
                    "Time {}: Staff {} released from machine {}",
                    sim.elapsed_time(),
                    staff_id,
                    machine_id
                ));
            }
            EventType::StaffUnavailable {
                machine_id,
                process_id,
            } => {
                logger.warning(&format!(
                    "Time {}: Process {} DELAYED - no staff available for machine {}",
                    sim.elapsed_time(),
                    process_id,
                    machine_id
                ));
            }
            _ => {
                logger.debug(&format!(
                    "Time {}: {:?}",
                    sim.elapsed_time(),
                    event.event_type
                ));
            }
        });

        prod.finalize_idle_time(prod.simulator.elapsed_time());
        logger.info("\n--- Final Status ---");
        logger.info(&prod.get_status());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn parse_config_path_supports_flags_and_positionals() {
        let args = vec![
            "assemblysim".to_string(),
            "--config".to_string(),
            "path/a.json".to_string(),
        ];
        assert_eq!(parse_config_path(&args), Some("path/a.json".to_string()));

        let args = vec![
            "assemblysim".to_string(),
            "-c".to_string(),
            "path/b.json".to_string(),
        ];
        assert_eq!(parse_config_path(&args), Some("path/b.json".to_string()));

        let args = vec!["assemblysim".to_string(), "path/c.json".to_string()];
        assert_eq!(parse_config_path(&args), Some("path/c.json".to_string()));

        let args = vec!["assemblysim".to_string()];
        assert_eq!(parse_config_path(&args), None);
    }

    #[test]
    fn load_simulation_from_config_builds_production_state() {
        let logger = Logger::new(LogLevel::Error);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("assemblysim_test_{}.json", timestamp));

        let config = serde_json::json!({
            "machines": [{
                "id": 0,
                "name": "Cutter",
                "staff_required": 1,
                "count": 2
            }],
            "staff": [{
                "id": 0,
                "name": "Alex",
                "role": {
                    "id": 0,
                    "name": "Operator",
                    "machine_ids": []
                }
            }],
            "processes": [{
                "machine_id": 0,
                "process_id": 5,
                "duration": 12
            }],
            "items": 3
        });

        std::fs::write(&path, serde_json::to_string(&config).unwrap()).unwrap();

        let loaded = load_simulation_from_config(path.to_str().unwrap(), &logger).unwrap();
        assert_eq!(loaded.production.machines.len(), 2);
        assert_eq!(loaded.machine_buckets.get(&0).unwrap().len(), 2);
        assert_eq!(loaded.machine_to_bucket.get(&0), Some(&0));
        assert_eq!(loaded.machine_to_bucket.get(&1), Some(&0));
        assert_eq!(loaded.production.staff.len(), 1);
        assert_eq!(loaded.steps.len(), 1);
        assert_eq!(loaded.items, 3);

        let _ = std::fs::remove_file(path);
    }
}
