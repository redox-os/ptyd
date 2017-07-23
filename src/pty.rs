use std::collections::VecDeque;

use redox_termios::*;
use syscall::error::Result;

pub struct Pty {
    pub id: usize,
    pub termios: Termios,
    pub winsize: Winsize,
    pub miso: VecDeque<Vec<u8>>,
    pub mosi: VecDeque<u8>,
}

impl Pty {
    pub fn new(id: usize) -> Self {
        let mut pty = Pty {
            id: id,
            termios: Termios::default(),
            winsize: Winsize::default(),
            miso: VecDeque::new(),
            mosi: VecDeque::new()
        };

        pty.termios.c_iflag = ICRNL | IXON;
        pty.termios.c_oflag = OPOST | ONLCR;
        pty.termios.c_cflag = B38400 | CS8 | CREAD | HUPCL;
        pty.termios.c_lflag = ISIG | ICANON | ECHO | ECHOE | ECHOK | IEXTEN;

        {
            let mut cc = |i: usize, b: cc_t| {
                pty.termios.c_cc[i] = b;
            };

            cc(VEOF, 0o004);    // CTRL-D
            cc(VEOL, 0o000);    // NUL
            cc(VEOL2, 0o000);   // NUL
            cc(VERASE, 0o177);  // DEL
            cc(VWERASE, 0o027); // CTRL-W
            cc(VKILL, 0o025);   // CTRL-U
            cc(VREPRINT, 0o022);// CTRL-R
            cc(VSWTC, 0o000);   // NUL
            cc(VINTR, 0o003);   // CTRL-C
            cc(VQUIT, 0o034);   // CTRL-\
            cc(VSUSP, 0o032);   // CTRL-Z
            cc(VSTART, 0o021);  // CTRL-Q
            cc(VSTOP, 0o023);   // CTRL-S
            cc(VLNEXT, 0o026);  // CTRL-V
            cc(VDISCARD, 0o017);// CTRL-U
            cc(VMIN, 1);
            cc(VTIME, 0);
        }

        pty
    }

    pub fn path(&self, buf: &mut [u8]) -> Result<usize> {
        let path_str = format!("pty:{}", self.id);
        let path = path_str.as_bytes();

        let mut i = 0;
        while i < buf.len() && i < path.len() {
            buf[i] = path[i];
            i += 1;
        }

        Ok(i)
    }
}
