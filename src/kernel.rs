//! Núcleo do simulador: loop de ticks, despacho de syscalls, gestão de PCBs.

use std::collections::HashMap;

use crate::console::Console;
use crate::error::{KernelError, KernelResult};
use crate::mem::{pages_for_brk, PageAllocator, PAGE_SIZE};
use crate::pcb::{Pcb, Pid, Priority, ProcessState, UserOp, MAX_PROCESS_FDS};
use crate::programs::{self, ProgId};
use crate::ramfs::RamFs;
use crate::sched::{SchedConfig, Scheduler};
use crate::syscall::{errno, nr, SysRet};

/// Configuração do kernel.
#[derive(Debug, Clone)]
pub struct KernelConfig {
    pub sched: SchedConfig,
    pub page_frames: usize,
    /// Máximo de ticks do loop principal (proteção anti-loop infinito).
    pub max_ticks: u64,
}

impl Default for KernelConfig {
    fn default() -> Self {
        Self {
            sched: SchedConfig::default(),
            page_frames: 256, // 1 MiB educativo
            max_ticks: 10_000,
        }
    }
}

/// Instantânea do estado (para CLI / introspecção).
#[derive(Debug, Clone)]
pub struct KernelStats {
    pub ticks: u64,
    pub processes: usize,
    pub zombies: usize,
    pub ready: usize,
    pub current: Option<Pid>,
    pub context_switches: u64,
    pub preemptions: u64,
    pub yields: u64,
    pub free_pages: usize,
    pub syscalls_total: u64,
    pub timed_out: bool,
}

/// O “kernel” rodando no host.
#[derive(Debug)]
pub struct Kernel {
    pub config: KernelConfig,
    pub ticks: u64,
    pub processes: HashMap<Pid, Pcb>,
    pub next_pid: Pid,
    pub sched: Scheduler,
    pub console: Console,
    pub ramfs: RamFs,
    pub pages: PageAllocator,
    pub syscalls_total: u64,
    /// Log de eventos (educativo / testes).
    pub log: Vec<String>,
    /// Buffers nomeados para READ/WRITE com path_id / msg já cobertos; path table:
    pub path_table: Vec<String>,
}

impl Kernel {
    pub fn new(mut config: KernelConfig) -> Self {
        config.sched.quantum = config.sched.quantum.max(1);
        let sched = Scheduler::new(config.sched.clone());
        let pages = PageAllocator::new(config.page_frames);
        let mut k = Self {
            config,
            ticks: 0,
            processes: HashMap::new(),
            next_pid: 1,
            sched,
            console: Console::new(),
            ramfs: RamFs::new(),
            pages,
            syscalls_total: 0,
            log: Vec::new(),
            path_table: vec!["/etc/motd".into(), "/tmp/out".into(), "/dev/stdout".into()],
        };
        // Garante arquivo /tmp/out
        k.ramfs.create("/tmp/out", Vec::new());
        k
    }

    fn log_event(&mut self, msg: impl Into<String>) {
        let m = format!("[t={}] {}", self.ticks, msg.into());
        self.log.push(m);
    }

    /// Cria um processo e coloca na ready queue.
    pub fn spawn_process(
        &mut self,
        name: impl Into<String>,
        ppid: Pid,
        priority: Priority,
        program: Vec<UserOp>,
    ) -> KernelResult<Pid> {
        self.sched.ensure_capacity(self.processes.len())?;
        let pid = self.next_pid;
        let next_pid = pid.checked_add(1).ok_or(KernelError::PidExhausted)?;
        self.next_pid = next_pid;
        let pcb = Pcb::new(pid, ppid, name, priority).with_program(program);
        self.processes.insert(pid, pcb);
        self.sched.enqueue(pid);
        self.log_event(format!("spawn pid={pid} ppid={ppid}"));
        Ok(pid)
    }

    pub fn spawn_prog(&mut self, prog: ProgId, ppid: Pid, priority: Priority) -> KernelResult<Pid> {
        let ops = programs::load_program(prog);
        self.spawn_process(prog.name(), ppid, priority, ops)
    }

    /// Boot: cria idle (pid reservado conceitual) + processos iniciais.
    pub fn boot(&mut self, init_progs: &[(ProgId, Priority)]) -> KernelResult<()> {
        self.sched.ensure_capacity(self.processes.len())?;
        if init_progs.len() > self.sched.config.max_processes - self.processes.len() {
            return Err(KernelError::ProcessLimit(self.sched.config.max_processes));
        }
        let requested_pids =
            u64::try_from(init_progs.len()).map_err(|_| KernelError::PidExhausted)?;
        let available_pids = u64::from(u32::MAX) - u64::from(self.next_pid);
        if requested_pids > available_pids {
            return Err(KernelError::PidExhausted);
        }
        self.console.write_bytes(b"[kernel] boot\n");
        self.log_event("boot");
        for &(prog, prio) in init_progs {
            self.spawn_prog(prog, 0, prio)?;
        }
        Ok(())
    }

    pub fn stats(&self) -> KernelStats {
        let zombies = self
            .processes
            .values()
            .filter(|p| p.state == ProcessState::Zombie)
            .count();
        KernelStats {
            ticks: self.ticks,
            processes: self.processes.len(),
            zombies,
            ready: self.sched.ready_len(),
            current: self.sched.current,
            context_switches: self.sched.context_switches,
            preemptions: self.sched.preemptions,
            yields: self.sched.yields,
            free_pages: self.pages.free_pages(),
            syscalls_total: self.syscalls_total,
            timed_out: self.ticks >= self.config.max_ticks
                && self.processes.values().any(|p| {
                    matches!(
                        p.state,
                        ProcessState::Ready | ProcessState::Running | ProcessState::Blocked
                    )
                }),
        }
    }

    /// Despertar processos cujo sleep expirou.
    fn wake_sleepers(&mut self) {
        let now = self.ticks;
        let mut wake = Vec::new();
        for (pid, pcb) in self.processes.iter_mut() {
            if pcb.state == ProcessState::Blocked {
                if let Some(at) = pcb.wake_at {
                    if now >= at {
                        pcb.state = ProcessState::Ready;
                        pcb.wake_at = None;
                        wake.push(*pid);
                    }
                }
            }
        }
        wake.sort_unstable();
        for pid in wake {
            self.sched.enqueue(pid);
            self.log_event(format!("wake pid={pid}"));
        }
    }

    /// Reap zumbis (remove PCB e libera frames).
    pub fn reap_zombies(&mut self) -> usize {
        let zids: Vec<Pid> = self
            .processes
            .iter()
            .filter(|(_, p)| p.state == ProcessState::Zombie)
            .map(|(pid, _)| *pid)
            .collect();
        let n = zids.len();
        let mut zids = zids;
        zids.sort_unstable();
        for pid in zids {
            self.pages.free_pid(pid);
            self.sched.dequeue_pid(pid);
            self.processes.remove(&pid);
            self.log_event(format!("reap pid={pid}"));
        }
        n
    }

    /// Escala: se não há current, pega próximo; se quantum estourou, preempta.
    fn schedule(&mut self, reason: &str) {
        // Se current morreu / bloqueou, limpa.
        if let Some(cur) = self.sched.current {
            let alive_runnable = self
                .processes
                .get(&cur)
                .map(|p| p.state == ProcessState::Running || p.state == ProcessState::Ready)
                .unwrap_or(false);
            if !alive_runnable {
                self.sched.current = None;
            }
        }

        if self.sched.current.is_none() {
            let prio_map: HashMap<Pid, Priority> = self
                .processes
                .iter()
                .map(|(k, v)| (*k, v.priority))
                .collect();
            if let Some(next) = self.sched.pick_next(|p| *prio_map.get(&p).unwrap_or(&255)) {
                if let Some(pcb) = self.processes.get_mut(&next) {
                    pcb.state = ProcessState::Running;
                    pcb.switches += 1;
                }
                let _ = self.sched.switch_to(next, false, false);
                self.log_event(format!("switch→{next} ({reason})"));
            }
            return;
        }

        // PriorityRr rotates among equal priorities but never preempts for a
        // strictly lower-priority process.
        let should_preempt = self.sched.quantum_left == 0
            && self.sched.ready_len() > 0
            && match self.sched.config.policy {
                crate::sched::Policy::RoundRobin => true,
                crate::sched::Policy::PriorityRr => {
                    let current_priority = self
                        .sched
                        .current
                        .and_then(|pid| self.processes.get(&pid))
                        .map(|pcb| pcb.priority)
                        .unwrap_or(u8::MAX);
                    self.sched.ready_pids().any(|pid| {
                        self.processes
                            .get(&pid)
                            .map(|pcb| pcb.priority <= current_priority)
                            .unwrap_or(false)
                    })
                }
            };
        if should_preempt {
            let old = self.sched.current.unwrap();
            if let Some(pcb) = self.processes.get_mut(&old) {
                if pcb.state == ProcessState::Running {
                    pcb.state = ProcessState::Ready;
                }
            }
            let prio_map: HashMap<Pid, Priority> = self
                .processes
                .iter()
                .map(|(k, v)| (*k, v.priority))
                .collect();
            // Limpa current ANTES do enqueue — senão enqueue ignora o pid atual.
            self.sched.current = None;
            let old_alive = self.processes.contains_key(&old);
            if old_alive && self.processes[&old].state == ProcessState::Ready {
                self.sched.enqueue(old);
            }
            self.sched.note_preempt();
            if let Some(next) = self.sched.pick_next(|p| *prio_map.get(&p).unwrap_or(&255)) {
                if let Some(pcb) = self.processes.get_mut(&next) {
                    pcb.state = ProcessState::Running;
                    pcb.switches += 1;
                }
                let _ = self.sched.switch_to(next, false, false);
                self.log_event(format!("preempt {old}→{next}"));
            }
        } else if self.sched.quantum_left == 0 {
            // Lower-priority work cannot force a preemption; start a fresh slice.
            self.sched.quantum_left = self.sched.config.quantum;
        }
    }

    fn exit_process(&mut self, pid: Pid, code: i32) {
        if let Some(pcb) = self.processes.get_mut(&pid) {
            pcb.state = ProcessState::Zombie;
            pcb.exit_code = Some(code);
            pcb.program.clear();
            pcb.wake_at = None;
        }
        self.sched.dequeue_pid(pid);
        if self.sched.current == Some(pid) {
            self.sched.current = None;
        }
        self.log_event(format!("exit pid={pid} code={code}"));
    }

    /// Despacha uma syscall para o processo `pid`.
    pub fn dispatch_syscall(&mut self, pid: Pid, num: u32, a0: i64, a1: i64, a2: i64) -> SysRet {
        match self.processes.get(&pid).map(|pcb| pcb.state) {
            None | Some(ProcessState::Zombie) => return SysRet::err(errno::ESRCH),
            Some(ProcessState::Blocked) => return SysRet::err(errno::EAGAIN),
            Some(ProcessState::Ready | ProcessState::Running) => {}
        }
        self.syscalls_total += 1;
        match num {
            nr::WRITE => match i32::try_from(a0) {
                Ok(fd) => self.sys_write(pid, fd, a1, a2),
                Err(_) => SysRet::err(errno::EBADF),
            },
            nr::READ => match i32::try_from(a0) {
                Ok(fd) => self.sys_read(pid, fd, a1, a2),
                Err(_) => SysRet::err(errno::EBADF),
            },
            nr::YIELD => {
                self.sched.note_yield();
                if let Some(pcb) = self.processes.get_mut(&pid) {
                    pcb.state = ProcessState::Ready;
                }
                self.sched.dequeue_pid(pid);
                self.sched.enqueue(pid);
                self.log_event(format!("yield pid={pid}"));
                SysRet::OK
            }
            nr::EXIT => match i32::try_from(a0) {
                Ok(code) => {
                    self.exit_process(pid, code);
                    SysRet::OK
                }
                Err(_) => SysRet::err(errno::EINVAL),
            },
            nr::GETPID => SysRet(pid as i64),
            nr::SLEEP => {
                let Ok(ticks) = u64::try_from(a0) else {
                    return SysRet::err(errno::EINVAL);
                };
                let wake = self.ticks.saturating_add(ticks);
                if let Some(pcb) = self.processes.get_mut(&pid) {
                    pcb.state = ProcessState::Blocked;
                    pcb.wake_at = Some(wake);
                }
                self.sched.dequeue_pid(pid);
                if self.sched.current == Some(pid) {
                    self.sched.current = None;
                }
                self.log_event(format!("sleep pid={pid} until={wake}"));
                SysRet::OK
            }
            nr::SPAWN => {
                let Ok(prog_id) = u32::try_from(a0) else {
                    return SysRet::err(errno::EINVAL);
                };
                let Some(prog) = ProgId::from_u32(prog_id) else {
                    return SysRet::err(errno::EINVAL);
                };
                let prio = if a1 < 0 { 10 } else { a1.min(255) as u8 };
                match self.spawn_prog(prog, pid, prio) {
                    Ok(child) => SysRet(child as i64),
                    Err(KernelError::ProcessLimit(_) | KernelError::PidExhausted) => {
                        SysRet::err(errno::EAGAIN)
                    }
                    Err(_) => SysRet::err(errno::EINVAL),
                }
            }
            nr::BRK => match usize::try_from(a0) {
                Ok(new_brk) => self.sys_brk(pid, new_brk),
                Err(_) => SysRet::err(errno::ENOMEM),
            },
            nr::OPEN => self.sys_open(pid, a0),
            nr::CLOSE => match i32::try_from(a0) {
                Ok(fd) => self.sys_close(pid, fd),
                Err(_) => SysRet::err(errno::EBADF),
            },
            nr::GETTICKS => SysRet(self.ticks as i64),
            nr::KILL => {
                let Ok(target) = u32::try_from(a0) else {
                    return SysRet::err(errno::ESRCH);
                };
                if self
                    .processes
                    .get(&target)
                    .map(|pcb| pcb.state == ProcessState::Zombie)
                    .unwrap_or(true)
                {
                    return SysRet::err(errno::ESRCH);
                }
                self.exit_process(target, -9);
                SysRet::OK
            }
            nr::GETPPID => {
                let ppid = self.processes.get(&pid).map(|p| p.ppid as i64).unwrap_or(0);
                SysRet(ppid)
            }
            nr::NICE => {
                let prio = a0.clamp(0, 255) as u8;
                if let Some(pcb) = self.processes.get_mut(&pid) {
                    pcb.priority = prio;
                }
                SysRet::OK
            }
            _ => SysRet::err(errno::EINVAL),
        }
    }

    fn sys_write(&mut self, pid: Pid, fd: i32, msg_id: i64, len: i64) -> SysRet {
        let Ok(len) = usize::try_from(len) else {
            return SysRet::err(errno::EINVAL);
        };
        let path = {
            let Some(pcb) = self.processes.get(&pid) else {
                return SysRet::err(errno::ESRCH);
            };
            if fd < 0 || fd as usize >= pcb.fds.len() {
                return SysRet::err(errno::EBADF);
            }
            match &pcb.fds[fd as usize] {
                Some(p) => p.clone(),
                None => return SysRet::err(errno::EBADF),
            }
        };

        let Some(msg) = programs::message(msg_id) else {
            return SysRet::err(errno::EINVAL);
        };
        let data = msg.as_bytes();
        let len = if len == 0 {
            data.len()
        } else {
            len.min(data.len())
        };
        let data = &data[..len];

        if path == "/dev/stdout" || path == "/dev/stderr" || fd == 1 || fd == 2 {
            self.console.write_bytes(data);
            SysRet(data.len() as i64)
        } else {
            match self.ramfs.append(&path, data) {
                Ok(()) => SysRet(data.len() as i64),
                Err(_) => SysRet::err(errno::ENOENT),
            }
        }
    }

    fn sys_read(&mut self, pid: Pid, fd: i32, _buf_id: i64, max: i64) -> SysRet {
        let Ok(max) = usize::try_from(max) else {
            return SysRet::err(errno::EINVAL);
        };
        let path = {
            let Some(pcb) = self.processes.get(&pid) else {
                return SysRet::err(errno::ESRCH);
            };
            if fd < 0 || fd as usize >= pcb.fds.len() {
                return SysRet::err(errno::EBADF);
            }
            match &pcb.fds[fd as usize] {
                Some(p) => p.clone(),
                None => return SysRet::err(errno::EBADF),
            }
        };
        match self.ramfs.read(&path) {
            Ok(data) => {
                let len = if max == 0 {
                    data.len()
                } else {
                    data.len().min(max)
                };
                SysRet(len as i64)
            }
            Err(_) => SysRet::err(errno::ENOENT),
        }
    }

    fn sys_brk(&mut self, pid: Pid, new_brk: usize) -> SysRet {
        let (old, limit) = {
            let Some(pcb) = self.processes.get(&pid) else {
                return SysRet::err(errno::ESRCH);
            };
            (pcb.brk, pcb.brk_limit)
        };
        if new_brk > limit {
            return SysRet::err(errno::ENOMEM);
        }
        if new_brk > old {
            let need = pages_for_brk(old, new_brk);
            if self.pages.alloc(need, pid).is_err() {
                return SysRet::err(errno::ENOMEM);
            }
        } else if new_brk < old {
            let release = pages_for_brk(new_brk, old);
            self.pages.free_pid_pages(pid, release);
        }
        if let Some(pcb) = self.processes.get_mut(&pid) {
            pcb.brk = new_brk;
        }
        SysRet::OK
    }

    fn sys_open(&mut self, pid: Pid, path_id: i64) -> SysRet {
        let Ok(path_id) = usize::try_from(path_id) else {
            return SysRet::err(errno::EINVAL);
        };
        if path_id >= self.path_table.len() {
            return SysRet::err(errno::EINVAL);
        }
        let path = self.path_table[path_id].clone();
        if !self.ramfs.exists(&path) {
            return SysRet::err(errno::ENOENT);
        }
        let Some(pcb) = self.processes.get_mut(&pid) else {
            return SysRet::err(errno::ESRCH);
        };
        if let Some(fd) = pcb
            .fds
            .iter()
            .enumerate()
            .skip(3)
            .find_map(|(fd, entry)| entry.is_none().then_some(fd))
        {
            pcb.fds[fd] = Some(path);
            return SysRet(fd as i64);
        }
        if pcb.fds.len() >= MAX_PROCESS_FDS {
            return SysRet::err(errno::EMFILE);
        }
        pcb.fds.push(Some(path));
        SysRet((pcb.fds.len() - 1) as i64)
    }

    fn sys_close(&mut self, pid: Pid, fd: i32) -> SysRet {
        let Some(pcb) = self.processes.get_mut(&pid) else {
            return SysRet::err(errno::ESRCH);
        };
        if fd < 3 || fd as usize >= pcb.fds.len() {
            return SysRet::err(errno::EBADF);
        }
        if pcb.fds[fd as usize].is_none() {
            return SysRet::err(errno::EBADF);
        }
        pcb.fds[fd as usize] = None;
        SysRet::OK
    }

    /// Executa **um** tick de CPU do processo current.
    fn run_one_op(&mut self) {
        let Some(pid) = self.sched.current else {
            return;
        };
        // Copia a próxima op
        let op = {
            let Some(pcb) = self.processes.get_mut(&pid) else {
                self.sched.current = None;
                return;
            };
            if pcb.state != ProcessState::Running {
                return;
            }
            match pcb.program.pop_front() {
                Some(o) => o,
                None => {
                    // programa acabou → exit 0
                    UserOp::Syscall {
                        num: nr::EXIT,
                        a0: 0,
                        a1: 0,
                        a2: 0,
                    }
                }
            }
        };

        match op {
            UserOp::Nop { ticks } => {
                let t = ticks.max(1);
                if let Some(pcb) = self.processes.get_mut(&pid) {
                    pcb.cpu_ticks += 1;
                    if t > 1 {
                        pcb.push_front_op(UserOp::Nop { ticks: t - 1 });
                    }
                }
                let _ = self.sched.tick_quantum();
            }
            UserOp::Halt => {
                self.exit_process(pid, 0);
            }
            UserOp::Syscall { num, a0, a1, a2 } => {
                if let Some(pcb) = self.processes.get_mut(&pid) {
                    pcb.cpu_ticks += 1;
                }
                let _ = self.dispatch_syscall(pid, num, a0, a1, a2);
                // Syscalls também consomem quantum
                let _ = self.sched.tick_quantum();
            }
        }
    }

    /// Avança exatamente 1 tick do kernel.
    pub fn tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
        self.wake_sleepers();
        self.schedule("tick-enter");
        self.run_one_op();
        // Após op, se quantum zerou, schedule vai preemptar no próximo tick-enter;
        // força re-schedule imediato se current sumiu.
        if self.sched.current.is_none() {
            self.schedule("post-op");
        } else if self.sched.quantum_left == 0 {
            self.schedule("quantum");
        }
        // reap periódico
        if self.ticks % 8 == 0 {
            self.reap_zombies();
        }
    }

    /// Roda até não haver processos vivos (Ready/Running/Blocked) ou max_ticks.
    pub fn run_until_idle(&mut self) -> KernelStats {
        while self.ticks < self.config.max_ticks {
            let live = self.processes.values().any(|p| {
                matches!(
                    p.state,
                    ProcessState::Ready | ProcessState::Running | ProcessState::Blocked
                )
            });
            if !live {
                self.reap_zombies();
                break;
            }
            self.tick();
        }
        self.reap_zombies();
        self.stats()
    }

    /// Dump amigável de processos.
    pub fn ps_lines(&self) -> Vec<String> {
        let mut pids: Vec<_> = self.processes.keys().copied().collect();
        pids.sort();
        let mut lines = vec![format!(
            "{:>5} {:>5} {:>8} {:>4} {:>6} {:>6} NAME",
            "PID", "PPID", "STATE", "PRI", "CPU", "SW"
        )];
        for pid in pids {
            let p = &self.processes[&pid];
            lines.push(format!(
                "{:>5} {:>5} {:>8} {:>4} {:>6} {:>6} {}",
                p.pid,
                p.ppid,
                p.state.as_str(),
                p.priority,
                p.cpu_ticks,
                p.switches,
                p.name
            ));
        }
        lines
    }

    pub fn page_size() -> usize {
        PAGE_SIZE
    }
}
