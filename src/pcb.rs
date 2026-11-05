//! Process Control Block (PCB) e estados de processo.

use std::collections::VecDeque;

/// Identificador de processo (PID).
pub type Pid = u32;

/// Estados clássicos de um processo no simulador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessState {
    /// Pronto para rodar; na ready queue.
    Ready,
    /// Em execução no “CPU” simulado.
    Running,
    /// Bloqueado (ex.: sleep até um tick futuro).
    Blocked,
    /// Terminou; aguarda reaping / remoção.
    Zombie,
}

impl ProcessState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Running => "Running",
            Self::Blocked => "Blocked",
            Self::Zombie => "Zombie",
        }
    }
}

/// Prioridade: menor número = maior prioridade (estilo nice invertido simplificado).
pub type Priority = u8;

pub const MAX_PROCESS_FDS: usize = 256;

/// Programa userland simulado: sequência de “instruções” de alto nível.
#[derive(Debug, Clone)]
pub enum UserOp {
    /// Chama syscall com número e até 3 args (i64).
    Syscall { num: u32, a0: i64, a1: i64, a2: i64 },
    /// Pseudo-op: “trabalho” de CPU por N ticks sem syscall (só gasta quantum).
    Nop { ticks: u32 },
    /// Fim implícito do programa (vira EXIT se chegar aqui).
    Halt,
}

/// PCB — bloco de controle de processo.
#[derive(Debug, Clone)]
pub struct Pcb {
    pub pid: Pid,
    pub ppid: Pid,
    pub name: String,
    pub state: ProcessState,
    pub priority: Priority,
    /// Programa: fila de UserOps restantes.
    pub program: VecDeque<UserOp>,
    /// Heap “brk” (bytes alocados no address space simulado).
    pub brk: usize,
    /// Heap máximo permitido para este processo.
    pub brk_limit: usize,
    /// Tick absoluto em que um sleep desbloqueia (se Blocked).
    pub wake_at: Option<u64>,
    /// Código de saída (quando Zombie).
    pub exit_code: Option<i32>,
    /// Contador de context switches sofridos.
    pub switches: u64,
    /// Ticks de CPU consumidos.
    pub cpu_ticks: u64,
    /// FDs abertos: índice → caminho no ramfs (None = fechado / reservado).
    pub fds: Vec<Option<String>>,
}

impl Pcb {
    pub fn new(pid: Pid, ppid: Pid, name: impl Into<String>, priority: Priority) -> Self {
        Self {
            pid,
            ppid,
            name: name.into(),
            state: ProcessState::Ready,
            priority,
            program: VecDeque::new(),
            brk: 0,
            brk_limit: 64 * 1024,
            wake_at: None,
            exit_code: None,
            switches: 0,
            cpu_ticks: 0,
            fds: vec![
                Some("/dev/stdin".into()),
                Some("/dev/stdout".into()),
                Some("/dev/stderr".into()),
            ],
        }
    }

    pub fn with_program(mut self, ops: impl IntoIterator<Item = UserOp>) -> Self {
        self.program = ops.into_iter().collect();
        self
    }

    pub fn is_runnable(&self) -> bool {
        self.state == ProcessState::Ready || self.state == ProcessState::Running
    }

    pub fn push_front_op(&mut self, op: UserOp) {
        self.program.push_front(op);
    }
}
