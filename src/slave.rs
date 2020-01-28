use std::cell::RefCell;
use std::rc::Weak;

use syscall::error::{Error, Result, EINVAL, EPIPE, EAGAIN};
use syscall::flag::{F_GETFL, F_SETFL, O_ACCMODE, O_NONBLOCK};

use pty::Pty;
use resource::Resource;

/// Read side of a pipe
#[derive(Clone)]
pub struct PtySlave {
    pty: Weak<RefCell<Pty>>,
    flags: usize,
    notified_read: bool,
    notified_write: bool
}

impl PtySlave {
    pub fn new(pty: Weak<RefCell<Pty>>, flags: usize) -> Self {
        PtySlave {
            pty: pty,
            flags: flags,
            notified_read: false,
            notified_write: false
        }
    }
}

impl Resource for PtySlave {
    fn boxed_clone(&self) -> Box<dyn Resource> {
        Box::new(self.clone())
    }

    fn pty(&self) -> Weak<RefCell<Pty>> {
        self.pty.clone()
    }

    fn flags(&self) -> usize {
        self.flags
    }

    fn path(&self, buf: &mut [u8]) -> Result<usize> {
        if let Some(pty_lock) = self.pty.upgrade() {
            pty_lock.borrow_mut().path(buf)
        } else {
            Err(Error::new(EPIPE))
        }
    }

    fn read(&self, buf: &mut [u8]) -> Result<Option<usize>> {
        if let Some(pty_lock) = self.pty.upgrade() {
            let mut pty = pty_lock.borrow_mut();

            if let Some(packet) = pty.mosi.pop_front() {
                let mut i = 0;

                while i < buf.len() && i < packet.len() {
                    buf[i] = packet[i];
                    i += 1;
                }

                if i < packet.len() {
                    pty.mosi.push_front(packet[i..].to_vec());
                }

                Ok(Some(i))
            } else if self.flags & O_NONBLOCK == O_NONBLOCK {
                Err(Error::new(EAGAIN))
            } else {
                Ok(None)
            }
        } else {
            Ok(Some(0))
        }
    }

    fn write(&self, buf: &[u8]) -> Result<Option<usize>> {
        if let Some(pty_lock) = self.pty.upgrade() {
            let mut pty = pty_lock.borrow_mut();

            if pty.miso.len() >= 64 {
                return Ok(None);
            }

            pty.output(buf);

            Ok(Some(buf.len()))
        } else {
            Err(Error::new(EPIPE))
        }
    }

    fn sync(&self) -> Result<usize> {
        if let Some(pty_lock) = self.pty.upgrade() {
            let mut pty = pty_lock.borrow_mut();

            pty.miso.push_back(vec![1]);

            Ok(0)
        } else {
            Err(Error::new(EPIPE))
        }
    }

    fn fcntl(&mut self, cmd: usize, arg: usize) -> Result<usize> {
        match cmd {
            F_GETFL => Ok(self.flags),
            F_SETFL => {
                self.flags = (self.flags & O_ACCMODE) | (arg & ! O_ACCMODE);
                Ok(0)
            },
            _ => Err(Error::new(EINVAL))
        }
    }

    fn fevent(&mut self) -> Result<usize> {
        self.notified_read = false; // resend
        self.notified_write = false;
        Ok(self.events())
    }

    fn events(&mut self) -> usize {
        let mut events = 0;

        if let Some(pty_lock) = self.pty.upgrade() {
            let pty = pty_lock.borrow();
            if pty.mosi.front().is_some() {
                if !self.notified_read {
                    self.notified_read = true;
                    events |= syscall::EVENT_READ;
                }
            } else {
                self.notified_read = false;
            }
        }

        if ! self.notified_write {
            self.notified_write = true;
            events |= syscall::EVENT_WRITE;
        }

        events
    }
}
