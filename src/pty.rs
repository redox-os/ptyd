use std::collections::VecDeque;

use redox_termios::*;
use syscall;
use syscall::error::Result;

pub struct Pty {
    pub id: usize,
    pub pgrp: usize,
    pub termios: Termios,
    pub winsize: Winsize,
    pub cooked: Vec<u8>,
    pub miso: VecDeque<Vec<u8>>,
    pub mosi: VecDeque<Vec<u8>>,
}

impl Pty {
    pub fn new(id: usize) -> Self {
        Pty {
            id: id,
            pgrp: 0,
            termios: Termios::default(),
            winsize: Winsize::default(),
            cooked: Vec::new(),
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

        let is_cc = |b: u8, i: usize| -> bool {
            b != 0 && b == cc[i]
        };

        let inlcr = ifl & INLCR == INLCR;
        let igncr = ifl & IGNCR == IGNCR;
        let icrnl = ifl & ICRNL == ICRNL;

        let echo = lfl & ECHO == ECHO;
        let echoe = lfl & ECHOE == ECHOE;
        let echonl = lfl & ECHONL == ECHONL;
        let icanon = lfl & ICANON == ICANON;
        let isig = lfl & ISIG == ISIG;
        let iexten = lfl & IEXTEN == IEXTEN;
        let ixon = lfl & IXON == IXON;

        for &byte in buf.iter() {
            let mut b = byte;

            // Input tranlation
            if b == b'\n' {
                if inlcr {
                    b = b'\r';
                }
            } else if b == b'\r' {
                if igncr {
                    b = 0;
                } else if icrnl {
                    b = b'\n';
                }
            }

            // Link settings
            if icanon {
                if b == b'\n' {
                    if echo || echonl {
                        self.output(&[b]);
                    }

                    self.cooked.push(b);
                    self.mosi.push_back(self.cooked.clone());
                    self.cooked.clear();

                    b = 0;
                }

                if is_cc(b, VEOF) {
                    self.mosi.push_back(self.cooked.clone());
                    self.cooked.clear();

                    b = 0;
                }

                if is_cc(b, VEOL) {
                    if echo {
                        self.output(&[b]);
                    }

                    self.cooked.push(b);
                    self.mosi.push_back(self.cooked.clone());
                    self.cooked.clear();

                    b = 0;
                }

                if is_cc(b, VEOL2) {
                    if echo {
                        self.output(&[b]);
                    }

                    self.cooked.push(b);
                    self.mosi.push_back(self.cooked.clone());
                    self.cooked.clear();

                    b = 0;
                }

                if is_cc(b, VERASE) {
                    if let Some(_c) = self.cooked.pop() {
                        if echoe {
                            self.output(&[8, b' ', 8]);
                        }
                    }

                    b = 0;
                }

                if is_cc(b, VWERASE) && iexten {
                    println!("VWERASE");
                    b = 0;
                }

                if is_cc(b, VKILL) {
                    println!("VKILL");
                    b = 0;
                }

                if is_cc(b, VREPRINT) && iexten {
                    println!("VREPRINT");
                    b = 0;
                }
            }

            if isig {
                if is_cc(b, VINTR) {
                    if self.pgrp != 0 {
                        let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGINT);
                    }

                    b = 0;
                }

                if is_cc(b, VQUIT) {
                    if self.pgrp != 0 {
                        let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGQUIT);
                    }

                    b = 0;
                }

                if is_cc(b, VSUSP) {
                    if self.pgrp != 0 {
                        let _ = syscall::kill(-(self.pgrp as isize) as usize, syscall::SIGTSTP);
                    }

                    b = 0;
                }
            }

            if ixon {
                if is_cc(b, VSTART) {
                    println!("VSTART");
                    b = 0;
                }

                if is_cc(b, VSTOP) {
                    println!("VSTOP");
                    b = 0;
                }
            }

            if is_cc(b, VLNEXT) && iexten {
                println!("VLNEXT");
                b = 0;
            }

            if is_cc(b, VDISCARD) && iexten {
                println!("VDISCARD");
                b = 0;
            }

            if b != 0 {
                if echo {
                    self.output(&[b]);
                }
                self.cooked.push(b);
            }
        }

        if ! icanon && self.cooked.len() >= cc[VMIN] as usize {
            self.mosi.push_back(self.cooked.clone());
            self.cooked.clear();
        }
    }

    pub fn output(&mut self, buf: &[u8]) {
        //TODO: more output flags

        let ofl = &self.termios.c_oflag;

        let opost = ofl & OPOST == OPOST;
        let onlcr = ofl & ONLCR == ONLCR;

        let mut vec = Vec::with_capacity(buf.len() + 1);
        vec.push(0);

        for &b in buf.iter() {
            if opost && onlcr && b == b'\n' {
                vec.push(b'\r');
            }
            vec.push(b);
        }

        self.miso.push_back(vec);
    }
}
