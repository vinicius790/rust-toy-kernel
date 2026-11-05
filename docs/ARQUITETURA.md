# Arquitetura — toy-kernel

## 1. Modelo mental

O “CPU” é o método `Kernel::tick()`:

1. `ticks += 1`
2. Acorda sleepers (`wake_at <= now`) → `Ready` + enqueue
3. `schedule`: se não há `current`, `pick_next`; com quantum zerado, decide se
   o processo atual deve ceder a CPU conforme a política e a fila Ready
4. Executa **uma** `UserOp` do processo current (Nop / Syscall / Halt)
5. Periodicamente `reap_zombies()` (libera PCB + frames)

Não há threads OS reais: tudo é cooperativo/preemptivo **dentro** do simulador.

## 2. PCB (`src/pcb.rs`)

Campos principais:

- `pid`, `ppid`, `name`, `priority`
- `state`: Ready | Running | Blocked | Zombie
- `program: VecDeque<UserOp>` — o “binário” userland
- `brk` / `brk_limit` — heap simulado
- `wake_at` — deadline de sleep
- `fds` — tabela de arquivos (0/1/2 = stdin/out/err virtuais)
- contadores `cpu_ticks`, `switches`

### UserOp

```text
Syscall { num, a0, a1, a2 }  →  dispatch_syscall
Nop { ticks }                →  gasta quantum sem syscall
Halt                         →  exit(0)
```

Programas prontos em `src/programs.rs` (`ProgId`).

## 3. Scheduler (`src/sched.rs`)

### Políticas

| Policy | Comportamento |
|--------|----------------|
| `RoundRobin` | Fila FIFO; quantum corta a fatia |
| `PriorityRr` | Menor `priority` primeiro; empate ≈ RR |

### Quantum e preempção

Cada op (Nop step ou syscall) decrementa `quantum_left`. Em `RoundRobin`, ao
zerar o quantum e existir outro Ready, o current volta para a fila e outro é
eleito. Em `PriorityRr`, a preempção por quantum ocorre apenas quando o próximo
processo Ready tem prioridade igual ou maior (número menor ou igual); caso
contrário, o atual continua. A troca por quantum incrementa `preemptions`.

`SYS_YIELD` força o mesmo caminho sem esperar o quantum.

## 4. Syscalls (`src/syscall.rs` + despacho em `kernel.rs`)

Tabela estática `SYSCALL_TABLE` (metadados) + `match` em `dispatch_syscall`.
Erros negativos estilo Linux (`-EINVAL`, `-ENOMEM`, …) em `syscall::errno`.

Fluxos notáveis:

- **write**: `a1` indexa mensagens em `programs::message`; `a2` limita o tamanho
  (`0` = mensagem inteira); fd 1/2 → `Console`
- **read**: `a2` limita os bytes contados (`0` = arquivo inteiro); não copia para
  memória userland real
- **spawn**: `a0` = `ProgId`; retorna child pid
- **brk**: calcula páginas extras via `pages_for_brk` e `PageAllocator::alloc`
- **sleep**: `Blocked` + `wake_at = now + ticks`
- **open/close**: descritores 0–2 são reservados; cada processo pode ter até
  256 entradas e descritores fechados são reutilizados

## 5. Memória (`src/mem.rs`)

- `PAGE_SIZE = 4096`
- Pool de `Frame { id, used, owner_pid }`
- `free_pid` no reap — não há COW nem swap

Isso ensina a **contabilidade** de frames; não emula page walk x86.

## 6. Console e RamFs

- `Console`: 80×25 células `(ch, attr)`, scroll, `written` para asserts
- `RamFs`: `HashMap<path, bytes>` com `/etc/motd`, `/tmp/out`, `/dev/*`

## 7. Dois caminhos de execução

```text
 Simulador host                         Kernel freestanding
 ───────────────                         ───────────────────
 CLI → Kernel::boot/tick                 Limine → _start
 RR/PriorityRR + UserOp                  serial COM1 → halt
 dispatch de syscalls simulado           sem scheduler/syscalls ainda
 PageAllocator em memória                sem allocator próprio
```

O kernel de `boot/kernel/` é carregável via Limine e escreve uma confirmação em
COM1. O simulador em `src/` permanece o único alvo com scheduler, PCBs e
syscalls. O linker script e `limine.conf` participam da criação da ISO;
`multiboot2_header.S` é apenas referência antiga. Veja `LIMITATIONS.md`.

## 8. Diagrama de estados do processo

```text
            spawn
              │
              ▼
           Ready ◄──── yield / preempt
            │ ▲
     schedule│ │ sleep ends
            │ │
            ▼ │
         Running ──sleep──► Blocked
            │
         exit/kill/halt
            │
            ▼
         Zombie ──reap──► (removido)
```
