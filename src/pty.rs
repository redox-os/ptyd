use std::collections::VecDeque;

use redox_termios::*;
use syscall;
use syscall::error::Result;

pub struct Pty {
    pub id: usize,
    pub pgrp: usize,
    pub termios: Termios,
    pub winsize: Winsize,
    pub miso: VecDeque<Vec<u8>>,
    pub mosi: VecDeque<u8>,
}

impl Pty {
    pub fn new(id: usize) -> Self {
        let mut pty = Pty {
            id: id,
            pgrp: 0,
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

    pub fn input(&mut self, buf: &[u8]) {
        //let ifl = &self.termios.c_iflag;
        //let ofl = &self.termios.c_oflag;
        //let cfl = &self.termios.c_cflag;
        let lfl = &self.termios.c_lflag;
        let cc = &self.termios.c_cc;

        let icanon = lfl & ICANON == ICANON;
        let isig = lfl & ISIG == ISIG;
        let iexten = lfl & IEXTEN == IEXTEN;
        let ixon = lfl & IXON == IXON;

        for &b in buf.iter() {
            let mut ignore = false;
            if b == 0 {
                println!("NUL");
            } else {
                if icanon {
                    if b == cc[VEOF] {
                        println!("VEOF");
                        ignore = true;
                    }

                    if b == cc[VEOL] {
                        println!("VEOL");
                    }

                    if b == cc[VEOL2] {
                        println!("VEOL2");
                    }

                    if b == cc[VERASE] {
                        println!("ERASE");
                        //ignore = true;
                    }

                    if b == cc[VWERASE] && iexten {
                        println!("VWERASE");
                        ignore = true;
                    }

                    if b == cc[VKILL] {
                        println!("VKILL");
                        ignore = true;
                    }

                    if b == cc[VREPRINT] && iexten {
                        println!("VREPRINT");
                        ignore = true;
                    }
                }

                if isig {
                    if b == cc[VINTR] {
                        println!("VINTR");

                        if self.pgrp != 0 {
                            println!("pgrp {}", self.pgrp);
                            let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGINT);
                        }

                        ignore = true;
                    }

                    if b == cc[VQUIT] {
                        println!("VQUIT");

                        if self.pgrp != 0 {
                            println!("pgrp {}", self.pgrp);
                            let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGQUIT);
                        }

                        ignore = true;
                    }

                    if b == cc[VSUSP] {
                        println!("VSUSP");

                        if self.pgrp != 0 {
                            println!("pgrp {}", self.pgrp);
                            let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGTSTP);
                        }

                        ignore = true;
                    }
                }

                if ixon {
                    if b == cc[VSTART] {
                        println!("VSTART");
                        ignore = true;
                    }

                    if b == cc[VSTOP] {
                        println!("VSTOP");
                        ignore = true;
                    }
                }

                if b == cc[VLNEXT] && iexten {
                    println!("VLNEXT");
                    ignore = true;
                }

                if b == cc[VDISCARD] && iexten {
                    println!("VDISCARD");
                    ignore = true;
                }
            }

            if ! ignore {
                self.mosi.push_back(b);
            }
        }
    }
}
