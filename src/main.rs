#![deny(warnings)]

extern crate redox_termios;
extern crate syscall;

use std::fs::File;
use std::io::{Read, Write};

use syscall::data::Packet;
use syscall::error::EWOULDBLOCK;
use syscall::scheme::SchemeMut;

mod master;
mod pgrp;
mod pty;
mod resource;
mod scheme;
mod slave;
mod termios;
mod winsize;

use scheme::PtyScheme;

fn main(){
    // Daemonize
    if unsafe { syscall::clone(0).unwrap() } == 0 {
        let mut socket = File::create(":pty").expect("pty: failed to create pty scheme");
        let mut scheme = PtyScheme::new();

        syscall::setrens(0, 0).expect("ptyd: failed to enter null namespace");

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
                if let Some(count) = handle.fevent_count() {
                    socket.write(&Packet {
                        id: 0,
                        pid: 0,
                        uid: 0,
                        gid: 0,
                        a: syscall::number::SYS_FEVENT,
                        b: *id,
                        c: syscall::flag::EVENT_READ,
                        d: count
                    }).expect("pty: failed to write event");
                }
            }
        }
    }
}
