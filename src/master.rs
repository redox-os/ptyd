use std::cell::RefCell;
use std::rc::{Rc, Weak};

use syscall::error::{Error, Result, EINVAL, EAGAIN};
use syscall::flag::{F_GETFL, F_SETFL, O_ACCMODE, O_NONBLOCK};

use pty::Pty;
use resource::Resource;

/// Read side of a pipe
#[derive(Clone)]
pub struct PtyMaster {
    pty: Rc<RefCell<Pty>>,
    flags: usize,
    notified_read: bool,
    notified_write: bool
}

impl PtyMaster {
    pub fn new(pty: Rc<RefCell<Pty>>, flags: usize) -> Self {
        PtyMaster {
            pty: pty,
            flags: flags,
            notified_read: false,
            notified_write: false
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

    fn flags(&self) -> usize {
        self.flags
    }

    fn path(&self, buf: &mut [u8]) -> Result<usize> {
        self.pty.borrow_mut().path(buf)
    }

    fn read(&self, buf: &mut [u8]) -> Result<Option<usize>> {
        let mut pty = self.pty.borrow_mut();

        if let Some(packet) = pty.miso.pop_front() {
            let mut i = 0;

            while i < buf.len() && i < packet.len() {
                buf[i] = packet[i];
                i += 1;
            }

            if i < packet.len() {
                let packet_remaining = &packet[i..];
                let mut new_packet = Vec::with_capacity(packet_remaining.len() + 1);
                new_packet.push(packet[0]);
                new_packet.extend(packet_remaining);
                pty.miso.push_front(new_packet);
            }

            Ok(Some(i))
        } else if self.flags & O_NONBLOCK == O_NONBLOCK || Rc::weak_count(&self.pty) == 0 {
            Err(Error::new(EAGAIN))
        } else {
            Ok(None)
        }
    }

    fn write(&self, buf: &[u8]) -> Result<Option<usize>> {
        let mut pty = self.pty.borrow_mut();

        if pty.mosi.len() >= 64 {
            return Ok(None);
        }

        pty.input(buf);

        Ok(Some(buf.len()))
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

    fn fevent(&mut self) -> Result<()> {
        self.notified_read = false; // resend
        self.notified_write = false;
        Ok(())
    }

    fn fevent_count(&mut self) -> Option<usize> {
        let pty = self.pty.borrow();
        if let Some(data) = pty.miso.front() {
            if !self.notified_read {
                self.notified_read = true;
                Some(data.len())
            } else {
                None
            }
        } else {
            self.notified_read = false;
            None
        }
    }
    fn fevent_writable(&mut self) -> bool {
        let notified = self.notified_write;
        self.notified_write = true;
        !notified
    }
}
