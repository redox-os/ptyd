use std::cell::RefCell;
use std::rc::Weak;
use std::{mem, slice};

use syscall::error::{Error, Result, EBADF, EINVAL, EPIPE};
use syscall::flag::{EventFlags, F_GETFL, F_SETFL, O_ACCMODE};

use crate::pty::Pty;
use crate::resource::Resource;

/// Read side of a pipe
#[derive(Clone)]
pub struct PtyPgrp {
    pty: Weak<RefCell<Pty>>,
    flags: usize,
}

impl PtyPgrp {
    pub fn new(pty: Weak<RefCell<Pty>>, flags: usize) -> Self {
        PtyPgrp {
            pty: pty,
            flags: flags,
        }
    }
}

impl Resource for PtyPgrp {
    fn boxed_clone(&self) -> Box<dyn Resource> {
        Box::new(self.clone())
    }

    fn pty(&self) -> Weak<RefCell<Pty>> {
        self.pty.clone()
    }

    fn flags(&self) -> usize {
        self.flags
    }

    fn path(&mut self, buf: &mut [u8]) -> Result<usize> {
        if let Some(pty_lock) = self.pty.upgrade() {
            pty_lock.borrow_mut().path(buf)
        } else {
            Err(Error::new(EPIPE))
        }
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        if let Some(pty_lock) = self.pty.upgrade() {
            let pty = pty_lock.borrow();
            let pgrp: &[u8] = unsafe {
                slice::from_raw_parts(
                    &pty.pgrp as *const usize as *const u8,
                    mem::size_of::<usize>(),
                )
            };

            let mut i = 0;
            while i < buf.len() && i < pgrp.len() {
                buf[i] = pgrp[i];
                i += 1;
            }
            Ok(Some(i))
        } else {
            Ok(Some(0))
        }
    }

    fn write(&mut self, buf: &[u8]) -> Result<Option<usize>> {
        if let Some(pty_lock) = self.pty.upgrade() {
            let mut pty = pty_lock.borrow_mut();
            let pgrp: &mut [u8] = unsafe {
                slice::from_raw_parts_mut(
                    &mut pty.pgrp as *mut usize as *mut u8,
                    mem::size_of::<usize>(),
                )
            };

            let mut i = 0;
            while i < buf.len() && i < pgrp.len() {
                pgrp[i] = buf[i];
                i += 1;
            }
            Ok(Some(i))
        } else {
            Err(Error::new(EPIPE))
        }
    }

    fn sync(&mut self) -> Result<usize> {
        Ok(0)
    }

    fn fcntl(&mut self, cmd: usize, arg: usize) -> Result<usize> {
        match cmd {
            F_GETFL => Ok(self.flags),
            F_SETFL => {
                self.flags = (self.flags & O_ACCMODE) | (arg & !O_ACCMODE);
                Ok(0)
            }
            _ => Err(Error::new(EINVAL)),
        }
    }

    fn fevent(&mut self) -> Result<EventFlags> {
        Err(Error::new(EBADF))
    }

    fn events(&mut self) -> EventFlags {
        EventFlags::empty()
    }
}
