#![deny(warnings)]

extern crate syscall;

use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};
use std::fs::File;
use std::io::{Read, Write};
use std::rc::{Rc, Weak};
use std::str;

use syscall::data::{Packet, Stat};
use syscall::error::{Error, Result, EBADF, EINVAL, ENOENT, EPIPE, EWOULDBLOCK};
use syscall::flag::{F_GETFL, F_SETFL, O_ACCMODE, O_NONBLOCK, MODE_CHR};
use syscall::scheme::SchemeMut;

#[derive(Clone)]
enum Handle {
    Master(PtyMaster),
    Slave(PtySlave),
}

pub struct PtyScheme {
    next_id: usize,
    handles: BTreeMap<usize, Handle>,
}

impl PtyScheme {
    fn new() -> Self {
        PtyScheme {
            next_id: 0,
            handles: BTreeMap::new(),
        }
    }

    fn get_handle(&self, id: usize) -> Option<Handle> {
        self.handles.get(&id).map(|handle| handle.clone())
    }
}

impl SchemeMut for PtyScheme {
    fn open(&mut self, path: &[u8], flags: usize, _uid: u32, _gid: u32) -> Result<usize> {
        let path = str::from_utf8(path).or(Err(Error::new(EINVAL)))?.trim_matches('/');

        if path.is_empty() {
            let id = self.next_id;
            self.next_id += 1;

            self.handles.insert(id, Handle::Master(PtyMaster::new(id, flags)));

            Ok(id)
        } else {
            let master_id = path.parse::<usize>().or(Err(Error::new(EINVAL)))?;
            let handle = self.get_handle(master_id).ok_or(Error::new(ENOENT))?;
            if let Handle::Master(master) = handle {
                let id = self.next_id;
                self.next_id += 1;

                self.handles.insert(id, Handle::Slave(PtySlave::new(&master, flags)));

                Ok(id)
            } else {
                Err(Error::new(ENOENT))
            }
        }
    }

    fn dup(&mut self, old_id: usize, buf: &[u8]) -> Result<usize> {
        if ! buf.is_empty() {
            return Err(Error::new(EINVAL));
        }

        let handle = self.get_handle(old_id).ok_or(Error::new(EBADF))?;

        let id = self.next_id;
        self.next_id += 1;
        self.handles.insert(id, handle);

        Ok(id)
    }

    fn read(&mut self, id: usize, buf: &mut [u8]) -> Result<usize> {
        let handle = self.get_handle(id).ok_or(Error::new(EBADF))?;
        match handle {
            Handle::Master(master) => master.read(buf),
            Handle::Slave(slave) => slave.read(buf),
        }
    }

    fn write(&mut self, id: usize, buf: &[u8]) -> Result<usize> {
        let handle = self.get_handle(id).ok_or(Error::new(EBADF))?;
        match handle {
            Handle::Master(master) => master.write(buf),
            Handle::Slave(slave) => slave.write(buf),
        }
    }

    fn fcntl(&mut self, id: usize, cmd: usize, arg: usize) -> Result<usize> {
        match self.handles.get_mut(&id) {
            Some(mut handle) => match *handle {
                Handle::Master(ref mut master) => master.fcntl(cmd, arg),
                Handle::Slave(ref mut slave) => slave.fcntl(cmd, arg),
            },
            None => Err(Error::new(EBADF))
        }
    }

    fn fevent(&mut self, id: usize, _flags: usize) -> Result<usize> {
        let handle = self.get_handle(id).ok_or(Error::new(EBADF))?;
        match handle {
            Handle::Master(_master) => Ok(id),
            Handle::Slave(_slave) => Ok(id),
        }
    }

    fn fpath(&mut self, id: usize, buf: &mut [u8]) -> Result<usize> {
        let handle = self.get_handle(id).ok_or(Error::new(EBADF))?;
        match handle {
            Handle::Master(master) => master.path(buf),
            Handle::Slave(slave) => slave.path(buf),
        }
    }

    fn fstat(&mut self, id: usize, stat: &mut Stat) -> Result<usize> {
        let _handle = self.get_handle(id).ok_or(Error::new(EBADF))?;

        *stat = Stat {
            st_mode: MODE_CHR | 0o666,
            ..Default::default()
        };

        Ok(0)
    }

    fn fsync(&mut self, id: usize) -> Result<usize> {
        let handle = self.get_handle(id).ok_or(Error::new(EBADF))?;
        match handle {
            Handle::Master(_master) => Ok(0),
            Handle::Slave(slave) => slave.sync(),
        }
    }

    fn close(&mut self, id: usize) -> Result<usize> {
        drop(self.handles.remove(&id));

        Ok(0)
    }
}

pub struct Pty {
    id: usize,
    miso: VecDeque<Vec<u8>>,
    mosi: VecDeque<u8>,
}

impl Pty {
    pub fn new(id: usize) -> Self {
        Pty {
            id: id,
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
}

/// Read side of a pipe
#[derive(Clone)]
pub struct PtyMaster {
    pty: Rc<RefCell<Pty>>,
    flags: usize,
}

impl PtyMaster {
    pub fn new(id: usize, flags: usize) -> Self {
        PtyMaster {
            pty: Rc::new(RefCell::new(Pty::new(id))),
            flags: flags,
        }
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
}

/// Read side of a pipe
#[derive(Clone)]
pub struct PtySlave {
    pty: Weak<RefCell<Pty>>,
    flags: usize,
}

impl PtySlave {
    pub fn new(master: &PtyMaster, flags: usize) -> Self {
        PtySlave {
            pty: Rc::downgrade(&master.pty),
            flags: flags,
        }
    }

    fn path(&self, buf: &mut [u8]) -> Result<usize> {
        if let Some(pty_lock) = self.pty.upgrade() {
            pty_lock.borrow_mut().path(buf)
        } else {
            Err(Error::new(EPIPE))
        }
    }

    fn read(&self, buf: &mut [u8]) -> Result<usize> {
        if let Some(pty_lock) = self.pty.upgrade() {
            let mut pty = pty_lock.borrow_mut();

            let mut i = 0;

            while i < buf.len() && ! pty.mosi.is_empty() {
                buf[i] = pty.mosi.pop_front().unwrap();
                i += 1;
            }

            if i > 0 || self.flags & O_NONBLOCK == O_NONBLOCK {
                Ok(i)
            } else {
                Err(Error::new(EWOULDBLOCK))
            }
        } else {
            Ok(0)
        }
    }

    fn write(&self, buf: &[u8]) -> Result<usize> {
        if let Some(pty_lock) = self.pty.upgrade() {
            let mut vec = Vec::new();
            vec.push(0);
            vec.extend_from_slice(buf);

            let mut pty = pty_lock.borrow_mut();
            pty.miso.push_back(vec);

            Ok(buf.len())
        } else {
            Err(Error::new(EPIPE))
        }
    }

    fn sync(&self) -> Result<usize> {
        if let Some(pty_lock) = self.pty.upgrade() {
            let mut vec = Vec::new();
            vec.push(1);

            let mut pty = pty_lock.borrow_mut();
            pty.miso.push_back(vec);

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
}

fn main(){
    // Daemonize
    if unsafe { syscall::clone(0).unwrap() } == 0 {
        let mut socket = File::create(":pty").expect("pty: failed to create pty scheme");
        let mut scheme = PtyScheme::new();
        let mut todo = Vec::new();
        loop {
            let mut packet = Packet::default();
            socket.read(&mut packet).expect("pty: failed to read events from pty scheme");

            let a = packet.a;
            scheme.handle(&mut packet);
            if packet.a == (-EWOULDBLOCK) as usize {
                packet.a = a;
                todo.push(packet);
            } else {
                socket.write(&packet).expect("pty: failed to write responses to pty scheme");
            }

            let mut i = 0;
            while i < todo.len() {
                let a = todo[i].a;
                scheme.handle(&mut todo[i]);
                if todo[i].a == (-EWOULDBLOCK) as usize {
                    todo[i].a = a;
                    i += 1;
                } else {
                    let packet = todo.remove(i);
                    socket.write(&packet).expect("pty: failed to write responses to pty scheme");
                }
            }

            for (id, handle) in scheme.handles.iter() {
                match *handle {
                    Handle::Master(ref master) => {
                        let pty = master.pty.borrow();
                        if let Some(data) = pty.miso.front() {
                            socket.write(&Packet {
                                id: 0,
                                pid: 0,
                                uid: 0,
                                gid: 0,
                                a: syscall::number::SYS_FEVENT,
                                b: *id,
                                c: syscall::flag::EVENT_READ,
                                d: data.len()
                            }).expect("pty: failed to write event");
                        } else if Rc::weak_count(&master.pty) == 0 {
                            socket.write(&Packet {
                                id: 0,
                                pid: 0,
                                uid: 0,
                                gid: 0,
                                a: syscall::number::SYS_FEVENT,
                                b: *id,
                                c: syscall::flag::EVENT_READ,
                                d: 0
                            }).expect("pty: failed to write event");
                        }
                    },
                    Handle::Slave(ref slave) => {
                        if let Some(pty_lock) = slave.pty.upgrade() {
                            let pty = pty_lock.borrow();
                            if ! pty.mosi.is_empty() {
                                socket.write(&Packet {
                                    id: 0,
                                    pid: 0,
                                    uid: 0,
                                    gid: 0,
                                    a: syscall::number::SYS_FEVENT,
                                    b: *id,
                                    c: syscall::flag::EVENT_READ,
                                    d: pty.mosi.len()
                                }).expect("pty: failed to write event");
                            }
                        } else {
                            socket.write(&Packet {
                                id: 0,
                                pid: 0,
                                uid: 0,
                                gid: 0,
                                a: syscall::number::SYS_FEVENT,
                                b: *id,
                                c: syscall::flag::EVENT_READ,
                                d: 0
                            }).expect("pty: failed to write event");
                        }
                    }
                }
            }
        }
    }
}
