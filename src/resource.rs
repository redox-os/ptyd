use std::cell::RefCell;
use std::rc::Weak;

use syscall::error::Result;

use pty::Pty;

pub trait Resource {
    fn boxed_clone(&self) -> Box<Resource>;
    fn pty(&self) -> Weak<RefCell<Pty>>;
    fn flags(&self) -> usize;

    fn path(&self, buf: &mut [u8]) -> Result<usize>;
    fn read(&self, buf: &mut [u8]) -> Result<Option<usize>>;
    fn write(&self, buf: &[u8]) -> Result<Option<usize>>;
    fn sync(&self) -> Result<usize>;
    fn fcntl(&mut self, cmd: usize, arg: usize) -> Result<usize>;
    fn fevent(&mut self) -> Result<usize>;
    fn events(&mut self) -> usize;
}
