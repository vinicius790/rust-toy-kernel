//! CLI do toy-kernel: boot simulado, demo de processos, ps, dump do console.

use clap::{Parser, Subcommand, ValueEnum};
use toy_kernel::programs::ProgId;
use toy_kernel::sched::{Policy, SchedConfig};
use toy_kernel::syscall::SYSCALL_TABLE;
use toy_kernel::{Kernel, KernelConfig, Priority};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum PolicyCli {
    Rr,
    Priority,
}

#[derive(Parser, Debug)]
#[command(
    name = "toy-kernel",
    about = "Simulador educacional de mini-kernel (catálogo 3.1) — host-runnable, sem claim de boot bare-metal",
    version
)]
struct Cli {
    #[command(subcommand)]
    cmd: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Sobe o kernel simulado com um conjunto de programas e roda até idle.
    Run {
        /// Programas iniciais (hello,yielder,sleeper,spawner,writer,brk_demo,counter).
        #[arg(long, default_value = "hello,yielder")]
        progs: String,
        #[arg(long, default_value_t = 3)]
        quantum: u32,
        #[arg(long, value_enum, default_value_t = PolicyCli::Rr)]
        policy: PolicyCli,
        #[arg(long, default_value_t = 256)]
        frames: usize,
        /// Imprime o log de eventos do kernel.
        #[arg(long)]
        log: bool,
    },
    /// Lista a tabela de syscalls.
    Syscalls,
    /// Mostra um tick a tick (máx N) — útil para aulas.
    Trace {
        #[arg(long, default_value = "yielder,yielder")]
        progs: String,
        #[arg(long, default_value_t = 30)]
        ticks: u64,
        #[arg(long, default_value_t = 2)]
        quantum: u32,
    },
    /// Demo completa: vários processos + prioridade.
    Demo,
}

fn parse_progs(s: &str) -> Vec<(ProgId, Priority)> {
    s.split(',')
        .filter(|x| !x.trim().is_empty())
        .enumerate()
        .filter_map(|(i, name)| {
            let p = match name.trim().to_ascii_lowercase().as_str() {
                "hello" => ProgId::Hello,
                "yielder" => ProgId::Yielder,
                "sleeper" => ProgId::Sleeper,
                "spawner" => ProgId::Spawner,
                "writer" => ProgId::Writer,
                "brk_demo" | "brk" => ProgId::BrkDemo,
                "counter" => ProgId::Counter,
                other => {
                    eprintln!("aviso: programa desconhecido '{other}', ignorado");
                    return None;
                }
            };
            Some((p, 5 + (i as u8 % 5)))
        })
        .collect()
}

fn make_kernel(quantum: u32, policy: PolicyCli, frames: usize) -> Kernel {
    let policy = match policy {
        PolicyCli::Rr => Policy::RoundRobin,
        PolicyCli::Priority => Policy::PriorityRr,
    };
    let config = KernelConfig {
        sched: SchedConfig {
            policy,
            quantum,
            max_processes: 64,
        },
        page_frames: frames,
        max_ticks: 10_000,
    };
    Kernel::new(config)
}

fn main() {
    let cli = Cli::parse();
    match cli.cmd {
        Commands::Syscalls => {
            println!("{:>4}  {:<10}  argc  descrição", "nr", "nome");
            for e in SYSCALL_TABLE {
                println!("{:>4}  {:<10}  {:>4}  {}", e.num, e.name, e.argc, e.doc);
            }
        }
        Commands::Run {
            progs,
            quantum,
            policy,
            frames,
            log,
        } => {
            let mut k = make_kernel(quantum, policy, frames);
            let list = parse_progs(&progs);
            if list.is_empty() {
                eprintln!("nenhum programa válido");
                std::process::exit(1);
            }
            if let Err(err) = k.boot(&list) {
                eprintln!("falha no boot: {err}");
                std::process::exit(1);
            }
            let st = k.run_until_idle();
            println!("=== console ===");
            print!("{}", k.console.dump_text());
            println!("=== stats ===");
            println!(
                "ticks={} switches={} preemptions={} yields={} syscalls={} free_pages={} timed_out={}",
                st.ticks,
                st.context_switches,
                st.preemptions,
                st.yields,
                st.syscalls_total,
                st.free_pages,
                st.timed_out
            );
            if log {
                println!("=== log ===");
                for line in &k.log {
                    println!("{line}");
                }
            }
            if st.timed_out {
                eprintln!("kernel: limite de ticks atingido antes de ficar idle");
                std::process::exit(2);
            }
        }
        Commands::Trace {
            progs,
            ticks,
            quantum,
        } => {
            let mut k = make_kernel(quantum, PolicyCli::Rr, 256);
            let list = parse_progs(&progs);
            if list.is_empty() {
                eprintln!("nenhum programa válido");
                std::process::exit(1);
            }
            if let Err(err) = k.boot(&list) {
                eprintln!("falha no boot: {err}");
                std::process::exit(1);
            }
            for _ in 0..ticks {
                let live = k.processes.values().any(|p| {
                    matches!(
                        p.state,
                        toy_kernel::ProcessState::Ready
                            | toy_kernel::ProcessState::Running
                            | toy_kernel::ProcessState::Blocked
                    )
                });
                if !live {
                    break;
                }
                k.tick();
                let cur = k
                    .sched
                    .current
                    .map(|p| {
                        k.processes
                            .get(&p)
                            .map(|pcb| format!("{}({})", pcb.name, p))
                            .unwrap_or_else(|| format!("?{p}"))
                    })
                    .unwrap_or_else(|| "-".into());
                println!(
                    "t={:<4} current={:<16} ready={} qleft={}",
                    k.ticks,
                    cur,
                    k.sched.ready_len(),
                    k.sched.quantum_left
                );
            }
            if k.processes.values().any(|p| {
                matches!(
                    p.state,
                    toy_kernel::ProcessState::Ready
                        | toy_kernel::ProcessState::Running
                        | toy_kernel::ProcessState::Blocked
                )
            }) {
                println!("limite de ticks atingido antes de todos os processos terminarem");
            }
            println!("--- console ---");
            print!("{}", k.console.dump_text());
        }
        Commands::Demo => {
            let mut k = make_kernel(2, PolicyCli::Priority, 256);
            k.boot(&[
                (ProgId::Writer, 2),
                (ProgId::Sleeper, 5),
                (ProgId::Yielder, 3),
                (ProgId::Spawner, 4),
                (ProgId::BrkDemo, 6),
            ])
            .expect("boot");
            let st = k.run_until_idle();
            println!("=== DEMO toy-kernel ===\n");
            println!("--- console ---");
            print!("{}", k.console.dump_text());
            println!("--- ps (final; zumbis já reaped) ---");
            for line in k.ps_lines() {
                println!("{line}");
            }
            println!("\n--- stats ---");
            println!("ticks            : {}", st.ticks);
            println!("context switches : {}", st.context_switches);
            println!("preemptions      : {}", st.preemptions);
            println!("yields           : {}", st.yields);
            println!("syscalls         : {}", st.syscalls_total);
            println!("timed out        : {}", st.timed_out);
            println!(
                "free pages       : {}/{}",
                st.free_pages, k.config.page_frames
            );
            println!("\n(simulação host — ver LIMITATIONS.md)");
            if st.timed_out {
                eprintln!("kernel: limite de ticks atingido antes de ficar idle");
                std::process::exit(2);
            }
        }
    }
}
