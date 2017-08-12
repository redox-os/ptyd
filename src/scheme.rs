use std::collections::BTreeMap;
use std::cell::RefCell;
use std::rc::Rc;
use std::str;

use syscall::data::Stat;
use syscall::error::{Error, Result, EBADF, EINVAL, ENOENT};
use syscall::flag::MODE_CHR;
use syscall::scheme::SchemeMut;

use master::PtyMaster;
use pgrp::PtyPgrp;
use pty::Pty;
use resource::Resource;
use slave::PtySlave;
use termios::PtyTermios;
use winsize::PtyWinsize;

pub struct PtyScheme {
    next_id: usize,
    pub handles: BTreeMap<usize, Box<Resource>>,
}

impl PtyScheme {
    pub fn new() -> Self {
        PtyScheme {
            next_id: 0,
            handles: BTreeMap::new(),
        }
    }
}

impl SchemeMut for PtyScheme {
    fn open(&mut self, path: &[u8], flags: usize, _uid: u32, _gid: u32) -> Result<usize> {
        let path = str::from_utf8(path).or(Err(Error::new(EINVAL)))?.trim_matches('/');

        if path.is_empty() {
            let id = self.next_id;
            self.next_id += 1;

            let pty = Rc::new(RefCell::new(Pty::new(id)));
            self.handles.insert(id, Box::new(PtyMaster::new(pty, flags)));

            Ok(id)
        } else {
            let master_id = path.parse::<usize>().or(Err(Error::new(EINVAL)))?;
            let pty = {
                let handle = self.handles.get(&master_id).ok_or(Error::new(ENOENT))?;
                handle.pty()
            };

            let id = self.next_id;
            self.next_id += 1;

            self.handles.insert(id, Box::new(PtySlave::new(pty, flags)));

            Ok(id)
        }
    }

    fn dup(&mut self, old_id: usize, buf: &[u8]) -> Result<usize> {
        let handle: Box<Resource> = {
            let old_handle = self.handles.get(&old_id).ok_or(Error::new(EBADF))?;

            if buf.is_empty() {
                old_handle.boxed_clone()
            } else if buf == b"pgrp" {
                Box::new(PtyPgrp::new(old_handle.pty(), old_handle.flags()))
            } else if buf == b"termios" {
                Box::new(PtyTermios::new(old_handle.pty(), old_handle.flags()))
            } else if buf == b"winsize" {
                Box::new(PtyWinsize::new(old_handle.pty(), old_handle.flags()))
            } else {
                return Err(Error::new(EINVAL));
            }
        };

        let id = self.next_id;
        self.next_id += 1;
        self.handles.insert(id, handle);

        Ok(id)
    }

    fn read(&mut self, id: usize, buf: &mut [u8]) -> Result<usize> {
        let handle = self.handles.get(&id).ok_or(Error::new(EBADF))?;
        handle.read(buf)
    }

    fn write(&mut self, id: usize, buf: &[u8]) -> Result<usize> {
        let handle = self.handles.get(&id).ok_or(Error::new(EBADF))?;
        handle.write(buf)
    }

    fn fcntl(&mut self, id: usize, cmd: usize, arg: usize) -> Result<usize> {
        let handle = self.handles.get_mut(&id).ok_or(Error::new(EBADF))?;
        handle.fcntl(cmd, arg)
    }

    fn fevent(&mut self, id: usize, _flags: usize) -> Result<usize> {
        let handle = self.handles.get(&id).ok_or(Error::new(EBADF))?;
        handle.fevent().and(Ok(id))
    }

    fn fpath(&mut self, id: usize, buf: &mut [u8]) -> Result<usize> {
        let handle = self.handles.get(&id).ok_or(Error::new(EBADF))?;
        handle.path(buf)
    }

    fn fstat(&mut self, id: usize, stat: &mut Stat) -> Result<usize> {
        let _handle = self.handles.get(&id).ok_or(Error::new(EBADF))?;

        *stat = Stat {
            st_mode: MODE_CHR | 0o666,
            ..Default::default()
        };

        Ok(0)
    }

    fn fsync(&mut self, id: usize) -> Result<usize> {
        let handle = self.handles.get(&id).ok_or(Error::new(EBADF))?;
        handle.sync()
    }

    fn close(&mut self, id: usize) -> Result<usize> {
        drop(self.handles.remove(&id));

        Ok(0)
    }
}
