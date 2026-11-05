# toy-kernel — Kernel de SO de brinquedo (catálogo 3.1 MVP)

> **EN abstract.** Educational Rust toy OS with two separate targets: a
> **host-runnable** kernel simulator and a minimal **x86_64 Limine-bootable**
> freestanding kernel that writes to COM1 and halts. The boot target is not a
> complete operating system; see `LIMITATIONS.md`.
>
> The host simulator provides:
> PCBs, round-robin / priority scheduler with quantum-based scheduling, a real syscall
> table (≥12), VGA-like text console, ramfs, page-frame allocator, CLI demos, and
> host tests.

> **Resumo PT-BR.** Projeto educacional em Rust com dois alvos separados:
> simulador de mini-kernel no host e kernel freestanding x86_64 que inicia pelo
> Limine, escreve no console serial COM1 e para a CPU. O kernel bootável é um
> ponto de partida, não um SO completo; veja `LIMITATIONS.md`.

## Por que este formato

O simulador e o kernel bootável são alvos diferentes. O segundo valida a entrada
freestanding e o boot via Limine sem alegar que as abstrações do simulador já
foram portadas para hardware.

| Meta | Simulador host | Kernel bootável |
|------|----------------|-----------------|
| Inicialização | `Kernel::boot` | Limine → `_start` x86_64 |
| Saída | Buffer VGA-like em RAM | UART COM1 / serial |
| Scheduler e processos | RR/prioridade, PCB e estados simulados | Não implementados |
| Syscalls/userland | 14 syscalls e `UserOp` simulados | Não implementados |
| Memória | Contabilidade de frames simulados | Sem alocador/page tables próprios |
| Boot QEMU | Não se aplica | ISO Limine; BIOS e UEFI |

## Arquitetura (visão rápida)

```text
Host CLI ──► Kernel::tick ──► scheduler / PCBs / syscalls / RamFs / console

Limine ──► boot/kernel (_start) ──► UART COM1 ──► halt
```

Detalhes: [docs/ARQUITETURA.md](./docs/ARQUITETURA.md) ·
stack: [docs/TECNOLOGIAS.md](./docs/TECNOLOGIAS.md) ·
limites: [LIMITATIONS.md](./LIMITATIONS.md) ·
boot: [boot/README.md](./boot/README.md).

## Syscalls (ABI do simulador)

| nr | nome | argc | efeito |
|----|------|------|--------|
| 1 | write | 3 | escreve mensagem; `len=0` usa a mensagem inteira |
| 2 | read | 3 | retorna contagem até `max`; não consome nem copia bytes |
| 3 | yield | 0 | cede CPU |
| 4 | exit | 1 | termina → Zombie |
| 5 | getpid | 0 | retorna pid |
| 6 | sleep | 1 | bloqueia N ticks |
| 7 | spawn | 2 | cria filho a partir de `ProgId` |
| 8 | brk | 1 | ajusta heap; aloca frames |
| 9 | open | 1 | abre path da path_table |
| 10 | close | 1 | fecha fd |
| 11 | getticks | 0 | tick do kernel |
| 12 | kill | 1 | força exit no alvo |
| 13 | getppid | 0 | ppid |
| 14 | nice | 1 | muda prioridade |

`cargo run -- syscalls` imprime a tabela completa.

## Install / build / test

```bash
cargo build --release
cargo test --locked
./scripts/smoke.sh
```

Requisitos do simulador: Rust/Cargo 1.85 ou superior.
`scripts/smoke.sh` requer Bash; no Windows, execute-o no Git Bash ou WSL.

Para gerar e iniciar o kernel freestanding no QEMU (BIOS e UEFI):

```bash
rustup target add x86_64-unknown-none
python tools/boot.py --run
```

O fluxo baixa os binários Limine v12.9.6 e requer Python 3.9+, `xorriso` e
`qemu-system-x86_64` disponíveis no `PATH`. `--run` testa o boot tanto em BIOS
quanto em UEFI. O workflow de CI executa esse teste em um runner Windows
isolado; nenhuma ferramenta de boot é instalada no computador local.

## CLI

```bash
# Demo rica (prioridade + sleeper + spawner + brk)
cargo run --release -- demo

# Programas escolhidos
cargo run --release -- run --progs hello,yielder,sleeper --quantum 2 --log

# Trace tick a tick (aula)
cargo run --release -- trace --progs yielder,yielder --ticks 40 --quantum 2

# Tabela de syscalls
cargo run --release -- syscalls
```

Programas embutidos: `hello`, `yielder`, `sleeper`, `spawner`, `writer`,
`brk_demo`, `counter`.

## Exemplos

Ver [examples/](./examples/) — roteiros de aula e saída esperada.

## Licença

MIT — ver [LICENSE](./LICENSE).

## Recursos adicionados nesta expansão

O pipe agora usa FIFO circular, backpressure e fechamento explícito. Dados pendentes podem ser lidos após `close`; `is_eof` só fica verdadeiro quando a fila esvazia.
