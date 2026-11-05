//! Scheduler: round-robin cooperativo + preemptivo por tick (quantum).

use std::collections::VecDeque;

use crate::error::{KernelError, KernelResult};
use crate::pcb::{Pid, Priority, ProcessState};

/// Política de escalonamento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Round-robin puro (ignora prioridade na escolha; ordem de chegada).
    RoundRobin,
    /// Prioridade: menor número primeiro; empate → RR entre iguais.
    PriorityRr,
}

/// Configuração do scheduler.
#[derive(Debug, Clone)]
pub struct SchedConfig {
    pub policy: Policy,
    /// Quantum em ticks de CPU por processo (preempção quando estoura).
    pub quantum: u32,
    pub max_processes: usize,
}

impl Default for SchedConfig {
    fn default() -> Self {
        Self {
            policy: Policy::RoundRobin,
            quantum: 3,
            max_processes: 64,
        }
    }
}

/// Ready queue + metadados de escalonamento.
#[derive(Debug, Clone)]
pub struct Scheduler {
    pub config: SchedConfig,
    /// PIDs prontos (não inclui o current).
    ready: VecDeque<Pid>,
    /// PID em execução (None = idle).
    pub current: Option<Pid>,
    /// Ticks restantes do quantum atual.
    pub quantum_left: u32,
    /// Contadores.
    pub context_switches: u64,
    pub preemptions: u64,
    pub yields: u64,
}

impl Scheduler {
    pub fn new(mut config: SchedConfig) -> Self {
        config.quantum = config.quantum.max(1);
        Self {
            quantum_left: config.quantum,
            config,
            ready: VecDeque::new(),
            current: None,
            context_switches: 0,
            preemptions: 0,
            yields: 0,
        }
    }

    pub fn ready_len(&self) -> usize {
        self.ready.len()
    }

    pub fn ready_pids(&self) -> impl Iterator<Item = Pid> + '_ {
        self.ready.iter().copied()
    }

    pub fn enqueue(&mut self, pid: Pid) {
        if !self.ready.contains(&pid) && self.current != Some(pid) {
            self.ready.push_back(pid);
        }
    }

    pub fn dequeue_pid(&mut self, pid: Pid) {
        self.ready.retain(|p| *p != pid);
        if self.current == Some(pid) {
            self.current = None;
        }
    }

    /// Insere na ready queue respeitando a política (para PriorityRr reordena).
    pub fn requeue_with_priority<F>(&mut self, pid: Pid, priority_of: F)
    where
        F: Fn(Pid) -> Priority,
    {
        self.dequeue_pid(pid);
        match self.config.policy {
            Policy::RoundRobin => {
                self.ready.push_back(pid);
            }
            Policy::PriorityRr => {
                let prio = priority_of(pid);
                let pos = self
                    .ready
                    .iter()
                    .position(|p| priority_of(*p) > prio)
                    .unwrap_or(self.ready.len());
                self.ready.insert(pos, pid);
            }
        }
    }

    /// Escolhe o próximo processo. `pick_priority` usado em PriorityRr.
    pub fn pick_next<F>(&mut self, priority_of: F) -> Option<Pid>
    where
        F: Fn(Pid) -> Priority,
    {
        if self.ready.is_empty() {
            return None;
        }
        match self.config.policy {
            Policy::RoundRobin => self.ready.pop_front(),
            Policy::PriorityRr => {
                // Já mantemos ordenado; pega a frente.
                // Re-sort defensivo caso prioridades mudem via nice.
                let mut v: Vec<Pid> = self.ready.drain(..).collect();
                v.sort_by_key(|p| priority_of(*p));
                let next = v.remove(0);
                self.ready.extend(v);
                Some(next)
            }
        }
    }

    /// Troca de contexto: current → Ready (se ainda vivo) e pick_next → Running.
    pub fn switch_to(
        &mut self,
        next: Pid,
        mark_old_ready: bool,
        old_still_alive: bool,
    ) -> KernelResult<()> {
        if let Some(old) = self.current {
            if old == next {
                self.quantum_left = self.config.quantum;
                return Ok(());
            }
            if mark_old_ready && old_still_alive {
                self.enqueue(old);
            }
        }
        self.current = Some(next);
        self.quantum_left = self.config.quantum;
        self.context_switches += 1;
        Ok(())
    }

    pub fn force_idle(&mut self) {
        self.current = None;
        self.quantum_left = self.config.quantum;
    }

    pub fn tick_quantum(&mut self) -> bool {
        if self.current.is_none() {
            return false;
        }
        if self.quantum_left > 0 {
            self.quantum_left -= 1;
        }
        self.quantum_left == 0
    }

    pub fn note_yield(&mut self) {
        self.yields += 1;
    }

    pub fn note_preempt(&mut self) {
        self.preemptions += 1;
    }

    pub fn ensure_capacity(&self, current_count: usize) -> KernelResult<()> {
        if current_count >= self.config.max_processes {
            Err(KernelError::ProcessLimit(self.config.max_processes))
        } else {
            Ok(())
        }
    }
}

/// Helper: estado deve ir para Ready ao voltar da CPU.
pub fn should_requeue(state: ProcessState) -> bool {
    matches!(state, ProcessState::Ready | ProcessState::Running)
}
