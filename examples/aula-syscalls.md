# Roteiro de aula — syscalls

```bash
cargo run --release -- syscalls
```

Depois rode:

```bash
cargo run --release -- run --progs spawner,brk_demo --log
```

Pontos de discussão:

1. `spawn` devolve o pid filho (registrador virtual `SysRet`)
2. `brk` falha com `-ENOMEM` se passar de `brk_limit` (teste unitário)
3. `write` usa `msg_id` → string table; `len=0` grava a mensagem toda. Não há
   ponteiro nem cópia de userspace real (em um kernel real seria `copy_from_user`).
4. `read` retorna a contagem limitada por `max`, mas não copia bytes para
   userspace.
