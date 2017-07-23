use std::cell::RefCell;
use std::rc::Weak;

use syscall::error::Result;

use pty::Pty;

pub trait Resource {
    fn boxed_clone(&self) -> Box<Resource>;
    fn pty(&self) -> Weak<RefCell<Pty>>;
    fn path(&self, buf: &mut [u8]) -> Result<usize>;
    fn read(&self, buf: &mut [u8]) -> Result<usize>;
    fn write(&self, buf: &[u8]) -> Result<usize>;
    fn sync(&self) -> Result<usize>;
    fn fcntl(&mut self, cmd: usize, arg: usize) -> Result<usize>;
    fn fevent(&self) -> Result<()>;
    fn fevent_count(&self) -> Option<usize>;
}
