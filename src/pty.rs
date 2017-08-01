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
        Pty {
            id: id,
            pgrp: 0,
            termios: Termios::default(),
            winsize: Winsize::default(),
            miso: VecDeque::new(),
            mosi: VecDeque::new()
        }
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
        let ifl = self.termios.c_iflag;
        //let ofl = &self.termios.c_oflag;
        //let cfl = &self.termios.c_cflag;
        let lfl = self.termios.c_lflag;
        let cc = self.termios.c_cc;

        let inlcr = ifl & INLCR == INLCR;
        let igncr = ifl & IGNCR == IGNCR;
        let icrnl = ifl & ICRNL == ICRNL;

        let echo = lfl & ECHO == ECHO;
        let echonl = lfl & ECHONL == ECHONL;
        let icanon = lfl & ICANON == ICANON;
        let isig = lfl & ISIG == ISIG;
        let iexten = lfl & IEXTEN == IEXTEN;
        let ixon = lfl & IXON == IXON;

        for &byte in buf.iter() {
            let mut b = byte;

            let mut ignore = false;
            if b == 0 {
                println!("NUL");
            } else {
                if b == b'\n' {
                    if inlcr {
                        b = b'\r';
                    }
                } else if b == b'\r' {
                    if igncr {
                        ignore = true;
                    } else if icrnl {
                        b = b'\n';
                    }
                }

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
                        //println!("ERASE");
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
                            let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGINT);
                        }

                        ignore = true;
                    }

                    if b == cc[VQUIT] {
                        println!("VQUIT");

                        if self.pgrp != 0 {
                            //let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGQUIT);
                        }

                        ignore = true;
                    }

                    if b == cc[VSUSP] {
                        println!("VSUSP");

                        if self.pgrp != 0 {
                            //let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGTSTP);
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
                if echo || echonl && b == b'\n' {
                    self.output(&[b]);
                }
            }
        }
    }

    pub fn output(&mut self, buf: &[u8]) {
        //TODO: Output flags

        let mut vec = Vec::with_capacity(buf.len() + 1);
        vec.push(0);

        for &byte in buf.iter() {
            let b = byte;

            vec.push(b);
        }

        self.miso.push_back(vec);
    }
}
