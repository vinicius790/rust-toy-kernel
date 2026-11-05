//! Console VGA-like: buffer de texto 80×25 simulado.

/// Dimensões clássicas VGA text mode.
pub const VGA_COLS: usize = 80;
pub const VGA_ROWS: usize = 25;

/// Célula: caractere + atributo (cor simplificada).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    pub ch: u8,
    pub attr: u8,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            ch: b' ',
            attr: 0x07, // cinza em preto
        }
    }
}

/// Framebuffer de texto + cursor.
#[derive(Debug, Clone)]
pub struct Console {
    pub cells: Vec<Cell>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub attr: u8,
    /// Bytes escritos via SYS_WRITE (auditoria / testes).
    pub written: Vec<u8>,
}

impl Default for Console {
    fn default() -> Self {
        Self::new()
    }
}

impl Console {
    pub fn new() -> Self {
        Self {
            cells: vec![Cell::default(); VGA_COLS * VGA_ROWS],
            cursor_row: 0,
            cursor_col: 0,
            attr: 0x07,
            written: Vec::new(),
        }
    }

    fn idx(&self, row: usize, col: usize) -> usize {
        row * VGA_COLS + col
    }

    /// Escreve bytes como texto (UTF-8 tratado como bytes; `\n` avança linha).
    pub fn write_bytes(&mut self, data: &[u8]) {
        self.written.extend_from_slice(data);
        for &b in data {
            match b {
                b'\n' => self.newline(),
                b'\r' => self.cursor_col = 0,
                b'\t' => {
                    let next = ((self.cursor_col / 8) + 1) * 8;
                    while self.cursor_col < next.min(VGA_COLS) {
                        self.put_char(b' ');
                    }
                }
                _ => self.put_char(b),
            }
        }
    }

    fn put_char(&mut self, ch: u8) {
        if self.cursor_col >= VGA_COLS {
            self.newline();
        }
        let i = self.idx(self.cursor_row, self.cursor_col);
        self.cells[i] = Cell {
            ch,
            attr: self.attr,
        };
        self.cursor_col += 1;
    }

    fn newline(&mut self) {
        self.cursor_col = 0;
        self.cursor_row += 1;
        if self.cursor_row >= VGA_ROWS {
            self.scroll_up();
            self.cursor_row = VGA_ROWS - 1;
        }
    }

    fn scroll_up(&mut self) {
        self.cells.rotate_left(VGA_COLS);
        let start = (VGA_ROWS - 1) * VGA_COLS;
        for c in &mut self.cells[start..] {
            *c = Cell::default();
        }
    }

    /// Dump das linhas não-vazias (para CLI / testes).
    pub fn dump_text(&self) -> String {
        let mut out = String::new();
        for row in 0..VGA_ROWS {
            let start = row * VGA_COLS;
            let line: String = self.cells[start..start + VGA_COLS]
                .iter()
                .map(|c| c.ch as char)
                .collect();
            let trimmed = line.trim_end();
            if !trimmed.is_empty() {
                out.push_str(trimmed);
                out.push('\n');
            }
        }
        out
    }

    pub fn clear(&mut self) {
        self.cells.fill(Cell::default());
        self.cursor_row = 0;
        self.cursor_col = 0;
        self.written.clear();
    }
}
