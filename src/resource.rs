use std::cell::RefCell;
use std::rc::Weak;

use syscall::error::Result;
use syscall::flag::EventFlags;

use crate::pty::Pty;

pub trait Resource {
    fn boxed_clone(&self) -> Box<dyn Resource>;
    fn pty(&self) -> Weak<RefCell<Pty>>;
    fn flags(&self) -> usize;

    fn path(&mut self, buf: &mut [u8]) -> Result<usize>;
    fn read(&mut self, buf: &mut [u8]) -> Result<Option<usize>>;
    fn write(&mut self, buf: &[u8]) -> Result<Option<usize>>;
    fn sync(&mut self) -> Result<usize>;
    fn fcntl(&mut self, cmd: usize, arg: usize) -> Result<usize>;
    fn fevent(&mut self) -> Result<EventFlags>;
    fn events(&mut self) -> EventFlags;
    fn timeout(&self, _count: u64) {
        // Handled only by PTY control term
    }
}
