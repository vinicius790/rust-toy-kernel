//! Biblioteca de programas userland simulados (IDs para SYS_SPAWN).

use crate::pcb::UserOp;
use crate::syscall::nr;

/// ID de programa conhecido pelo kernel (spawn).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ProgId {
    Hello = 0,
    Yielder = 1,
    Sleeper = 2,
    Spawner = 3,
    Writer = 4,
    BrkDemo = 5,
    Counter = 6,
    IdleHint = 7,
}

impl ProgId {
    pub fn from_u32(v: u32) -> Option<Self> {
        match v {
            0 => Some(Self::Hello),
            1 => Some(Self::Yielder),
            2 => Some(Self::Sleeper),
            3 => Some(Self::Spawner),
            4 => Some(Self::Writer),
            5 => Some(Self::BrkDemo),
            6 => Some(Self::Counter),
            7 => Some(Self::IdleHint),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Hello => "hello",
            Self::Yielder => "yielder",
            Self::Sleeper => "sleeper",
            Self::Spawner => "spawner",
            Self::Writer => "writer",
            Self::BrkDemo => "brk_demo",
            Self::Counter => "counter",
            Self::IdleHint => "idle_hint",
        }
    }
}

fn write_msg(msg_id: i64) -> UserOp {
    // a0=fd(1), a1=msg_id interno, a2=0 (len resolvido pelo kernel via string table)
    UserOp::Syscall {
        num: nr::WRITE,
        a0: 1,
        a1: msg_id,
        a2: 0,
    }
}

/// Constrói o vetor de ops para um ProgId.
pub fn load_program(id: ProgId) -> Vec<UserOp> {
    match id {
        ProgId::Hello => vec![
            write_msg(0), // "hello from userland\n"
            UserOp::Syscall {
                num: nr::EXIT,
                a0: 0,
                a1: 0,
                a2: 0,
            },
        ],
        ProgId::Yielder => vec![
            write_msg(1), // "yielder A\n"
            UserOp::Syscall {
                num: nr::YIELD,
                a0: 0,
                a1: 0,
                a2: 0,
            },
            write_msg(2), // "yielder B\n"
            UserOp::Syscall {
                num: nr::EXIT,
                a0: 0,
                a1: 0,
                a2: 0,
            },
        ],
        ProgId::Sleeper => vec![
            write_msg(3), // "sleeping\n"
            UserOp::Syscall {
                num: nr::SLEEP,
                a0: 5,
                a1: 0,
                a2: 0,
            },
            write_msg(4), // "awake\n"
            UserOp::Syscall {
                num: nr::EXIT,
                a0: 0,
                a1: 0,
                a2: 0,
            },
        ],
        ProgId::Spawner => vec![
            UserOp::Syscall {
                num: nr::SPAWN,
                a0: ProgId::Hello as i64,
                a1: 5,
                a2: 0,
            },
            write_msg(5), // "spawned child\n"
            UserOp::Syscall {
                num: nr::EXIT,
                a0: 0,
                a1: 0,
                a2: 0,
            },
        ],
        ProgId::Writer => vec![
            write_msg(6),
            write_msg(6),
            write_msg(6),
            UserOp::Syscall {
                num: nr::EXIT,
                a0: 0,
                a1: 0,
                a2: 0,
            },
        ],
        ProgId::BrkDemo => vec![
            UserOp::Syscall {
                num: nr::BRK,
                a0: 8192,
                a1: 0,
                a2: 0,
            },
            write_msg(7), // "brk ok\n"
            UserOp::Syscall {
                num: nr::EXIT,
                a0: 0,
                a1: 0,
                a2: 0,
            },
        ],
        ProgId::Counter => vec![
            UserOp::Nop { ticks: 2 },
            UserOp::Syscall {
                num: nr::GETTICKS,
                a0: 0,
                a1: 0,
                a2: 0,
            },
            write_msg(8), // "counted\n"
            UserOp::Syscall {
                num: nr::EXIT,
                a0: 0,
                a1: 0,
                a2: 0,
            },
        ],
        ProgId::IdleHint => vec![UserOp::Nop { ticks: 1 }, UserOp::Halt],
    }
}

/// Mensagens referenciadas por msg_id em WRITE (a1).
pub fn message(id: i64) -> Option<&'static str> {
    match id {
        0 => Some("hello from userland\n"),
        1 => Some("yielder A\n"),
        2 => Some("yielder B\n"),
        3 => Some("sleeping\n"),
        4 => Some("awake\n"),
        5 => Some("spawned child\n"),
        6 => Some("tick\n"),
        7 => Some("brk ok\n"),
        8 => Some("counted\n"),
        9 => Some("kernel boot\n"),
        10 => Some("demo done\n"),
        _ => None,
    }
}
