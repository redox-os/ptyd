use std::cell::RefCell;
use std::rc::{Rc, Weak};

use syscall::error::{Error, Result, EAGAIN, EINVAL};
use syscall::flag::{EventFlags, F_GETFL, F_SETFL, O_ACCMODE, O_NONBLOCK};

use crate::pty::Pty;
use crate::resource::Resource;

/// Read side of a pipe
#[derive(Clone)]
pub struct PtyControlTerm {
    pty: Rc<RefCell<Pty>>,
    flags: usize,
    notified_read: bool,
    notified_write: bool,
}

impl PtyControlTerm {
    pub fn new(pty: Rc<RefCell<Pty>>, flags: usize) -> Self {
        PtyControlTerm {
            pty: pty,
            flags: flags,
            notified_read: false,
            notified_write: false,
        }
    }
}

impl Resource for PtyControlTerm {
    fn boxed_clone(&self) -> Box<dyn Resource> {
        Box::new(self.clone())
    }

    fn pty(&self) -> Weak<RefCell<Pty>> {
        Rc::downgrade(&self.pty)
    }

    fn flags(&self) -> usize {
        self.flags
    }

    fn path(&mut self, buf: &mut [u8]) -> Result<usize> {
        self.pty.borrow_mut().path(buf)
    }

    fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>> {
        
        self.notified_read = false;

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

    fn write(&mut self, buf: &[u8]) -> Result<Option<usize>> {
        let mut pty = self.pty.borrow_mut();

        if pty.mosi.len() >= 64 {
            return Ok(None);
        }

        pty.input(buf);

        Ok(Some(buf.len()))
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
        self.notified_read = false; // resend
        self.notified_write = false;
        Ok(self.events())
    }

    fn events(&mut self) -> EventFlags {
        let mut events = EventFlags::empty();

        let pty = self.pty.borrow();
        if pty.miso.front().is_some() {
            if !self.notified_read {
                self.notified_read = true;
                events |= syscall::EVENT_READ;
            }
        } else {
            self.notified_read = false;
        }

        if !self.notified_write {
            self.notified_write = true;
            events |= syscall::EVENT_WRITE;
        }

        events
    }

    fn timeout(&self, count: u64) {
        let mut pty = self.pty.borrow_mut();
        pty.timeout(count);
    }
}
