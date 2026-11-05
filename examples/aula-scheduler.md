# Roteiro de aula — scheduler

## Objetivo

Ver com os olhos a diferença entre **yield cooperativo** e **preempção por quantum**.

## 1. Dois yielders, quantum alto

```bash
cargo run --release -- trace --progs yielder,yielder --quantum 10 --ticks 40
```

Observe: a troca ocorre nos `SYS_YIELD`, não no fim do quantum.

## 2. Dois Nop longos, quantum=1

```bash
cargo run --release -- run --progs counter,counter --quantum 1 --log
```

No log, procure linhas `preempt A→B`.

## 3. Prioridade

```bash
cargo run --release -- run --progs writer,hello --policy priority --quantum 5
```

Com `PriorityRr`, programas com menor `priority` (o CLI atribui 5+i) tendem a
rodar antes — use `demo` para um cenário montado à mão.
