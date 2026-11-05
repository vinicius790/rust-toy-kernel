//! Testes de integração do simulador toy-kernel (≥15).
//! Roda no host via `cargo test` — sem QEMU.

use toy_kernel::mem::{pages_for_brk, PageAllocator, PAGE_SIZE};
use toy_kernel::pcb::{ProcessState, UserOp};
use toy_kernel::programs::ProgId;
use toy_kernel::ramfs::RamFs;
use toy_kernel::sched::{Policy, SchedConfig};
use toy_kernel::syscall::{self, errno, nr, SYSCALL_TABLE};
use toy_kernel::{Console, Kernel, KernelConfig};

fn cfg_rr(quantum: u32) -> KernelConfig {
    KernelConfig {
        sched: SchedConfig {
            policy: Policy::RoundRobin,
            quantum,
            max_processes: 64,
        },
        page_frames: 64,
        max_ticks: 5_000,
    }
}

fn cfg_prio(quantum: u32) -> KernelConfig {
    KernelConfig {
        sched: SchedConfig {
            policy: Policy::PriorityRr,
            quantum,
            max_processes: 64,
        },
        page_frames: 64,
        max_ticks: 5_000,
    }
}

// 1 -----------------------------------------------------------------------
#[test]
fn boot_spawns_ready_processes() {
    let mut k = Kernel::new(cfg_rr(3));
    k.boot(&[(ProgId::Hello, 5), (ProgId::Yielder, 5)]).unwrap();
    assert_eq!(k.processes.len(), 2);
    assert!(k.processes.values().all(|p| p.state == ProcessState::Ready));
    assert_eq!(k.sched.ready_len(), 2);
}

// 2 -----------------------------------------------------------------------
#[test]
fn hello_writes_to_console_and_exits() {
    let mut k = Kernel::new(cfg_rr(3));
    k.boot(&[(ProgId::Hello, 5)]).unwrap();
    let st = k.run_until_idle();
    let text = k.console.dump_text();
    assert!(text.contains("hello from userland"), "got: {text}");
    assert!(text.contains("[kernel] boot"));
    assert!(st.syscalls_total >= 2); // write + exit
    assert_eq!(k.processes.len(), 0); // reaped
}

// 3 -----------------------------------------------------------------------
#[test]
fn yield_causes_context_switch_between_two_yielders() {
    let mut k = Kernel::new(cfg_rr(10));
    k.boot(&[(ProgId::Yielder, 5), (ProgId::Yielder, 5)])
        .unwrap();
    let st = k.run_until_idle();
    assert!(st.context_switches >= 2, "switches={}", st.context_switches);
    assert!(st.yields >= 2, "yields={}", st.yields);
    let text = String::from_utf8_lossy(&k.console.written);
    assert!(text.contains("yielder A"));
    assert!(text.contains("yielder B"));
}

// 4 -----------------------------------------------------------------------
#[test]
fn sleep_blocks_then_wakes() {
    let mut k = Kernel::new(cfg_rr(3));
    k.boot(&[(ProgId::Sleeper, 5)]).unwrap();
    // roda alguns ticks: deve bloquear
    for _ in 0..3 {
        k.tick();
    }
    let blocked = k
        .processes
        .values()
        .any(|p| p.state == ProcessState::Blocked);
    assert!(blocked, "esperava sleeper bloqueado");
    let st = k.run_until_idle();
    let text = String::from_utf8_lossy(&k.console.written);
    assert!(text.contains("sleeping"));
    assert!(text.contains("awake"));
    assert!(st.ticks >= 5);
}

// 5 -----------------------------------------------------------------------
#[test]
fn spawn_syscall_creates_child() {
    let mut k = Kernel::new(cfg_rr(3));
    k.boot(&[(ProgId::Spawner, 5)]).unwrap();
    let st = k.run_until_idle();
    let text = String::from_utf8_lossy(&k.console.written);
    assert!(text.contains("spawned child"));
    assert!(text.contains("hello from userland"));
    assert!(st.syscalls_total >= 3);
}

// 6 -----------------------------------------------------------------------
#[test]
fn getpid_returns_nonzero() {
    let mut k = Kernel::new(cfg_rr(3));
    let ops = vec![
        UserOp::Syscall {
            num: nr::GETPID,
            a0: 0,
            a1: 0,
            a2: 0,
        },
        UserOp::Syscall {
            num: nr::EXIT,
            a0: 0,
            a1: 0,
            a2: 0,
        },
    ];
    let pid = k.spawn_process("getpid_test", 0, 5, ops).unwrap();
    assert!(pid >= 1);
    // despacha getpid manualmente
    let ret = k.dispatch_syscall(pid, nr::GETPID, 0, 0, 0);
    assert_eq!(ret.0, pid as i64);
}

// 7 -----------------------------------------------------------------------
#[test]
fn invalid_syscall_returns_einval() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k.spawn_process("x", 0, 5, vec![UserOp::Halt]).unwrap();
    let ret = k.dispatch_syscall(pid, 9999, 0, 0, 0);
    assert_eq!(ret.0, errno::EINVAL);
}

// 8 -----------------------------------------------------------------------
#[test]
fn syscall_table_has_at_least_12_entries() {
    assert!(SYSCALL_TABLE.len() >= 12);
    assert!(syscall::lookup(nr::WRITE).is_ok());
    assert!(syscall::lookup(nr::NICE).is_ok());
    assert!(syscall::lookup(0).is_err());
}

// 9 -----------------------------------------------------------------------
#[test]
fn preemption_on_quantum_exhaustion() {
    let mut k = Kernel::new(cfg_rr(1)); // quantum=1 → preempt agressivo
                                        // dois counters com Nop longos
    let long = vec![
        UserOp::Nop { ticks: 20 },
        UserOp::Syscall {
            num: nr::EXIT,
            a0: 0,
            a1: 0,
            a2: 0,
        },
    ];
    k.spawn_process("a", 0, 5, long.clone()).unwrap();
    k.spawn_process("b", 0, 5, long).unwrap();
    let st = k.run_until_idle();
    assert!(
        st.preemptions >= 1,
        "preemptions={} switches={}",
        st.preemptions,
        st.context_switches
    );
}

// 10 ----------------------------------------------------------------------
#[test]
fn priority_scheduler_runs_higher_first() {
    let mut k = Kernel::new(cfg_prio(5));
    // prio 1 (alta) escreve "high"; prio 20 escreve depois
    let high = vec![
        UserOp::Syscall {
            num: nr::WRITE,
            a0: 1,
            a1: 1, // yielder A
            a2: 0,
        },
        UserOp::Syscall {
            num: nr::EXIT,
            a0: 0,
            a1: 0,
            a2: 0,
        },
    ];
    let low = vec![
        UserOp::Syscall {
            num: nr::WRITE,
            a0: 1,
            a1: 2, // yielder B
            a2: 0,
        },
        UserOp::Syscall {
            num: nr::EXIT,
            a0: 0,
            a1: 0,
            a2: 0,
        },
    ];
    // enfileira low primeiro, depois high — priority deve inverter
    k.spawn_process("low", 0, 20, low).unwrap();
    k.spawn_process("high", 0, 1, high).unwrap();
    k.run_until_idle();
    let text = String::from_utf8_lossy(&k.console.written);
    let a = text.find("yielder A").expect("A");
    let b = text.find("yielder B").expect("B");
    assert!(a < b, "alta prioridade deveria escrever antes: {text}");
}

// 11 ----------------------------------------------------------------------
#[test]
fn brk_allocates_pages_and_oom_on_limit() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k.spawn_process("brk", 0, 5, vec![UserOp::Halt]).unwrap();
    // define limite baixo
    k.processes.get_mut(&pid).unwrap().brk_limit = 4096;
    let ok = k.dispatch_syscall(pid, nr::BRK, 4096, 0, 0);
    assert_eq!(ok.0, 0);
    assert_eq!(k.processes[&pid].brk, 4096);
    let fail = k.dispatch_syscall(pid, nr::BRK, 8192, 0, 0);
    assert_eq!(fail.0, errno::ENOMEM);
}

// 12 ----------------------------------------------------------------------
#[test]
fn kill_terminates_target() {
    let mut k = Kernel::new(cfg_rr(3));
    let victim = k
        .spawn_process(
            "victim",
            0,
            5,
            vec![UserOp::Nop { ticks: 100 }, UserOp::Halt],
        )
        .unwrap();
    let killer = k
        .spawn_process(
            "killer",
            0,
            5,
            vec![
                UserOp::Syscall {
                    num: nr::KILL,
                    a0: victim as i64,
                    a1: 0,
                    a2: 0,
                },
                UserOp::Syscall {
                    num: nr::EXIT,
                    a0: 0,
                    a1: 0,
                    a2: 0,
                },
            ],
        )
        .unwrap();
    let _ = killer;
    k.run_until_idle();
    // victim deve ter sido reaped (zumbi limpo) ou não existir
    assert!(!k.processes.contains_key(&victim));
}

// 13 ----------------------------------------------------------------------
#[test]
fn open_read_close_ramfs() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k.spawn_process("fs", 0, 5, vec![UserOp::Halt]).unwrap();
    // path_table[0] = /etc/motd
    let fd = k.dispatch_syscall(pid, nr::OPEN, 0, 0, 0);
    assert!(fd.0 >= 3, "fd={}", fd.0);
    let n = k.dispatch_syscall(pid, nr::READ, fd.0, 0, 0);
    assert!(n.0 > 0);
    let c = k.dispatch_syscall(pid, nr::CLOSE, fd.0, 0, 0);
    assert_eq!(c.0, 0);
}

#[test]
fn open_close_reuses_fd_without_growing_table() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k
        .spawn_process("fd-reuse", 0, 5, vec![UserOp::Halt])
        .unwrap();

    for _ in 0..1_000 {
        let fd = k.dispatch_syscall(pid, nr::OPEN, 0, 0, 0);
        assert_eq!(fd.0, 3);
        assert_eq!(k.dispatch_syscall(pid, nr::CLOSE, fd.0, 0, 0).0, 0);
    }

    assert_eq!(k.processes[&pid].fds.len(), 4);
}

#[test]
fn open_returns_emfile_at_per_process_fd_limit() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k
        .spawn_process("fd-limit", 0, 5, vec![UserOp::Halt])
        .unwrap();

    for expected_fd in 3..toy_kernel::pcb::MAX_PROCESS_FDS {
        assert_eq!(
            k.dispatch_syscall(pid, nr::OPEN, 0, 0, 0).0,
            expected_fd as i64
        );
    }
    assert_eq!(
        k.dispatch_syscall(pid, nr::OPEN, 0, 0, 0).0,
        toy_kernel::syscall::errno::EMFILE
    );
    assert_eq!(
        k.processes[&pid].fds.len(),
        toy_kernel::pcb::MAX_PROCESS_FDS
    );
}

// 14 ----------------------------------------------------------------------
#[test]
fn console_scrolls_and_tracks_written() {
    let mut c = Console::new();
    for _ in 0..30 {
        c.write_bytes(b"line\n");
    }
    assert!(!c.written.is_empty());
    assert!(c.cursor_row < toy_kernel::console::VGA_ROWS);
    let dump = c.dump_text();
    assert!(dump.contains("line"));
}

// 15 ----------------------------------------------------------------------
#[test]
fn page_allocator_alloc_and_free_pid() {
    let mut pa = PageAllocator::new(8);
    assert_eq!(pa.free_pages(), 8);
    let ids = pa.alloc(3, 42).unwrap();
    assert_eq!(ids.len(), 3);
    assert_eq!(pa.free_pages(), 5);
    assert!(pa.alloc(10, 1).is_err());
    let freed = pa.free_pid(42);
    assert_eq!(freed, 3);
    assert_eq!(pa.free_pages(), 8);
}

// 16 ----------------------------------------------------------------------
#[test]
fn pages_for_brk_math() {
    assert_eq!(pages_for_brk(0, 0), 0);
    assert_eq!(pages_for_brk(0, 1), 1);
    assert_eq!(pages_for_brk(0, PAGE_SIZE), 1);
    assert_eq!(pages_for_brk(0, PAGE_SIZE + 1), 2);
    assert_eq!(pages_for_brk(PAGE_SIZE, PAGE_SIZE * 2), 1);
}

// 17 ----------------------------------------------------------------------
#[test]
fn ramfs_motd_and_create() {
    let mut fs = RamFs::new();
    assert!(fs.exists("/etc/motd"));
    let m = fs.read("/etc/motd").unwrap();
    assert!(!m.is_empty());
    fs.create("/tmp/x", b"abc");
    fs.append("/tmp/x", b"de").unwrap();
    assert_eq!(fs.read("/tmp/x").unwrap(), b"abcde");
}

// 18 ----------------------------------------------------------------------
#[test]
fn process_limit_enforced() {
    let mut cfg = cfg_rr(3);
    cfg.sched.max_processes = 2;
    let mut k = Kernel::new(cfg);
    k.spawn_process("a", 0, 5, vec![UserOp::Halt]).unwrap();
    k.spawn_process("b", 0, 5, vec![UserOp::Halt]).unwrap();
    let err = k.spawn_process("c", 0, 5, vec![UserOp::Halt]);
    assert!(err.is_err());
}

// 19 ----------------------------------------------------------------------
#[test]
fn getppid_and_nice() {
    let mut k = Kernel::new(cfg_rr(3));
    let parent = k.spawn_process("parent", 0, 5, vec![UserOp::Halt]).unwrap();
    let child = k
        .spawn_process("child", parent, 5, vec![UserOp::Halt])
        .unwrap();
    let pp = k.dispatch_syscall(child, nr::GETPPID, 0, 0, 0);
    assert_eq!(pp.0, parent as i64);
    let _ = k.dispatch_syscall(child, nr::NICE, 1, 0, 0);
    assert_eq!(k.processes[&child].priority, 1);
}

// 20 ----------------------------------------------------------------------
#[test]
fn demo_runs_cleanly() {
    let mut k = Kernel::new(cfg_prio(2));
    k.boot(&[
        (ProgId::Writer, 2),
        (ProgId::Sleeper, 5),
        (ProgId::Yielder, 3),
        (ProgId::BrkDemo, 6),
    ])
    .unwrap();
    let st = k.run_until_idle();
    assert!(st.ticks > 0);
    assert!(st.syscalls_total > 5);
    assert!(k.console.written.len() > 10);
}

// 21 ----------------------------------------------------------------------
#[test]
fn preempted_process_is_rescheduled() {
    let mut k = Kernel::new(cfg_rr(1));
    let long = vec![
        UserOp::Nop { ticks: 5 },
        UserOp::Syscall {
            num: nr::WRITE,
            a0: 1,
            a1: 8, // "counted\n"
            a2: 0,
        },
        UserOp::Syscall {
            num: nr::EXIT,
            a0: 0,
            a1: 0,
            a2: 0,
        },
    ];
    k.spawn_process("a", 0, 5, long.clone()).unwrap();
    k.spawn_process("b", 0, 5, long).unwrap();
    let st = k.run_until_idle();
    assert!(
        st.ticks < 500,
        "não deve esgotar max_ticks; ticks={}",
        st.ticks
    );
    assert_eq!(k.processes.len(), 0, "ambos devem terminar e ser reaped");
    let text = String::from_utf8_lossy(&k.console.written);
    assert_eq!(text.matches("counted").count(), 2);
}

#[test]
fn shrinking_brk_releases_pages_and_regrowth_reuses_them() {
    let mut cfg = cfg_rr(3);
    cfg.page_frames = 2;
    let mut k = Kernel::new(cfg);
    let pid = k.spawn_process("heap", 0, 5, vec![UserOp::Halt]).unwrap();

    assert_eq!(k.dispatch_syscall(pid, nr::BRK, 8192, 0, 0).0, 0);
    assert_eq!(k.pages.free_pages(), 0);
    assert_eq!(k.dispatch_syscall(pid, nr::BRK, 1, 0, 0).0, 0);
    assert_eq!(k.pages.free_pages(), 1);
    assert_eq!(k.dispatch_syscall(pid, nr::BRK, 8192, 0, 0).0, 0);
    assert_eq!(k.pages.free_pages(), 0);
}

#[test]
fn syscall_integer_arguments_do_not_wrap_into_valid_ids_or_fds() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k.spawn_process("args", 0, 5, vec![UserOp::Halt]).unwrap();
    let wrapped_fd = i64::from(u32::MAX) + 2;

    assert_eq!(
        k.dispatch_syscall(pid, nr::WRITE, wrapped_fd, 0, 0).0,
        errno::EBADF
    );
    assert_eq!(
        k.dispatch_syscall(pid, nr::CLOSE, wrapped_fd, 0, 0).0,
        errno::EBADF
    );
    assert_eq!(
        k.dispatch_syscall(pid, nr::SPAWN, 1_i64 << 32, 5, 0).0,
        errno::EINVAL
    );
    assert_eq!(k.dispatch_syscall(pid, nr::BRK, -1, 0, 0).0, errno::ENOMEM);
    assert_eq!(
        k.dispatch_syscall(pid, nr::SLEEP, -1, 0, 0).0,
        errno::EINVAL
    );
    assert_eq!(
        k.dispatch_syscall(pid, nr::EXIT, i64::MAX, 0, 0).0,
        errno::EINVAL
    );
    assert_eq!(
        k.dispatch_syscall(pid, nr::WRITE, 1, -1, 0).0,
        errno::EINVAL
    );
    assert_eq!(
        k.dispatch_syscall(pid, nr::WRITE, 1, 99_999, 0).0,
        errno::EINVAL
    );
}

#[test]
fn syscalls_from_missing_or_zombie_processes_fail_without_side_effects() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k.spawn_process("dead", 0, 5, vec![UserOp::Halt]).unwrap();
    assert_eq!(k.dispatch_syscall(pid, nr::EXIT, 0, 0, 0).0, 0);
    let calls = k.syscalls_total;

    assert_eq!(k.dispatch_syscall(pid, nr::GETPID, 0, 0, 0).0, errno::ESRCH);
    assert_eq!(
        k.dispatch_syscall(u32::MAX, nr::YIELD, 0, 0, 0).0,
        errno::ESRCH
    );
    assert_eq!(k.syscalls_total, calls);
    assert_eq!(k.sched.yields, 0);
}

#[test]
fn blocked_process_cannot_issue_syscalls() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k
        .spawn_process("blocked", 0, 5, vec![UserOp::Halt])
        .unwrap();
    assert_eq!(k.dispatch_syscall(pid, nr::SLEEP, 10, 0, 0).0, 0);

    let calls = k.syscalls_total;
    assert_eq!(k.dispatch_syscall(pid, nr::YIELD, 0, 0, 0).0, errno::EAGAIN);
    assert_eq!(k.processes[&pid].state, ProcessState::Blocked);
    assert_eq!(k.syscalls_total, calls);
    assert!(!k.sched.ready_pids().any(|ready| ready == pid));
}

#[test]
fn run_until_idle_reports_tick_limit_and_zero_quantum_is_normalized() {
    let mut cfg = cfg_rr(0);
    cfg.max_ticks = 1;
    let mut k = Kernel::new(cfg);
    k.spawn_process("long", 0, 5, vec![UserOp::Nop { ticks: 10 }, UserOp::Halt])
        .unwrap();

    let stats = k.run_until_idle();
    assert!(stats.timed_out);
    assert_eq!(stats.ticks, 1);
    assert_eq!(k.sched.config.quantum, 1);
    assert!(k
        .processes
        .values()
        .any(|p| p.state != ProcessState::Zombie));
}

#[test]
fn pid_exhaustion_is_reported_instead_of_wrapping() {
    let mut k = Kernel::new(cfg_rr(3));
    k.next_pid = u32::MAX;
    let err = k.spawn_process("overflow", 0, 5, vec![UserOp::Halt]);
    assert_eq!(err.unwrap_err(), toy_kernel::KernelError::PidExhausted);
    assert!(k.processes.is_empty());
}

#[test]
fn brk_page_math_handles_usize_boundary_without_overflow() {
    assert_eq!(pages_for_brk(usize::MAX - 1, usize::MAX), 0);
    assert!(pages_for_brk(0, usize::MAX) > 0);
}

#[test]
fn read_and_write_lengths_are_applied() {
    let mut k = Kernel::new(cfg_rr(3));
    let pid = k.spawn_process("io", 0, 5, vec![UserOp::Halt]).unwrap();

    assert_eq!(k.dispatch_syscall(pid, nr::WRITE, 1, 0, 5).0, 5);
    assert!(k.console.written.ends_with(b"hello"));

    let fd = k.dispatch_syscall(pid, nr::OPEN, 0, 0, 0).0;
    assert!(fd >= 3);
    assert_eq!(k.dispatch_syscall(pid, nr::READ, fd, 0, 4).0, 4);
    assert_eq!(
        k.dispatch_syscall(pid, nr::READ, fd, 0, -1).0,
        errno::EINVAL
    );
}

#[test]
fn priority_scheduler_does_not_preempt_for_lower_priority_work() {
    let mut k = Kernel::new(cfg_prio(1));
    let high = vec![
        UserOp::Nop { ticks: 2 },
        UserOp::Syscall {
            num: nr::WRITE,
            a0: 1,
            a1: 1,
            a2: 0,
        },
        UserOp::Halt,
    ];
    let low = vec![
        UserOp::Syscall {
            num: nr::WRITE,
            a0: 1,
            a1: 2,
            a2: 0,
        },
        UserOp::Halt,
    ];
    k.spawn_process("high", 0, 1, high).unwrap();
    k.spawn_process("low", 0, 10, low).unwrap();

    k.tick();
    assert_eq!(k.sched.quantum_left, 1);
    assert_eq!(k.sched.current, Some(1));
    assert_eq!(k.sched.preemptions, 0);

    let stats = k.run_until_idle();
    let output = String::from_utf8_lossy(&k.console.written);
    assert!(output.find("yielder A").unwrap() < output.find("yielder B").unwrap());
    assert_eq!(stats.preemptions, 0);
}

#[test]
fn failed_boot_does_not_leave_partially_spawned_processes() {
    let mut cfg = cfg_rr(3);
    cfg.sched.max_processes = 1;
    let mut k = Kernel::new(cfg);

    assert!(k.boot(&[(ProgId::Hello, 5), (ProgId::Yielder, 5)]).is_err());
    assert!(k.processes.is_empty());
    assert_eq!(k.sched.ready_len(), 0);
    assert!(k.console.written.is_empty());
}

#[test]
fn boot_checks_pid_space_before_mutating_kernel_state() {
    let mut k = Kernel::new(cfg_rr(3));
    k.next_pid = u32::MAX - 1;

    assert_eq!(
        k.boot(&[(ProgId::Hello, 5), (ProgId::Yielder, 5)])
            .unwrap_err(),
        toy_kernel::KernelError::PidExhausted
    );
    assert!(k.processes.is_empty());
    assert_eq!(k.next_pid, u32::MAX - 1);
    assert!(k.console.written.is_empty());
}

#[test]
fn yield_from_non_current_process_does_not_deschedule_current() {
    let mut k = Kernel::new(cfg_rr(10));
    let current = k
        .spawn_process("current", 0, 5, vec![UserOp::Nop { ticks: 10 }])
        .unwrap();
    let other = k.spawn_process("other", 0, 5, vec![UserOp::Halt]).unwrap();
    k.tick();
    assert_eq!(k.sched.current, Some(current));

    assert_eq!(k.dispatch_syscall(other, nr::YIELD, 0, 0, 0).0, 0);

    assert_eq!(k.sched.current, Some(current));
    assert_eq!(k.processes[&current].state, ProcessState::Running);
    assert!(k.sched.ready_pids().any(|pid| pid == other));
}
