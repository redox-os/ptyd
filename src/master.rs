use std::cell::RefCell;
use std::rc::{Rc, Weak};

use syscall::error::{Error, Result, EINVAL, EWOULDBLOCK};
use syscall::flag::{F_GETFL, F_SETFL, O_ACCMODE, O_NONBLOCK};

use pty::Pty;
use resource::Resource;

/// Read side of a pipe
#[derive(Clone)]
pub struct PtyMaster {
    pty: Rc<RefCell<Pty>>,
    flags: usize,
}

impl PtyMaster {
    pub fn new(pty: Rc<RefCell<Pty>>, flags: usize) -> Self {
        PtyMaster {
            pty: pty,
            flags: flags,
        }
    }
}

impl Resource for PtyMaster {
    fn boxed_clone(&self) -> Box<Resource> {
        Box::new(self.clone())
    }

    fn pty(&self) -> Weak<RefCell<Pty>> {
        Rc::downgrade(&self.pty)
    }

    fn path(&self, buf: &mut [u8]) -> Result<usize> {
        self.pty.borrow_mut().path(buf)
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        let mut pty = self.pty.borrow_mut();

        if let Some(packet) = pty.miso.pop_front() {
            let mut i = 0;

            while i < buf.len() && i < packet.len() {
                buf[i] = packet[i];
                i += 1;
            }

            Ok(i)
        } else if self.flags & O_NONBLOCK == O_NONBLOCK || Rc::weak_count(&self.pty) == 0 {
            Ok(0)
        } else {
            Err(Error::new(EWOULDBLOCK))
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        let mut pty = self.pty.borrow_mut();

        let mut i = 0;
        while i < buf.len() {
            pty.mosi.push_back(buf[i]);
            i += 1;
        }

        Ok(i)
    }

    fn sync(&self) -> Result<usize> {
        Ok(0)
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

    fn fevent(&self) -> Result<()> {
        Ok(())
    }

    fn fevent_count(&self) -> Option<usize> {
        {
            let pty = self.pty.borrow();
            if let Some(data) = pty.miso.front() {
                return Some(data.len());
            }
        }

        if Rc::weak_count(&self.pty) == 0 {
            Some(0)
        } else {
            None
        }
    }
}
