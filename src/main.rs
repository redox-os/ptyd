extern crate redox_termios;
extern crate syscall;

use std::fs::File;
use std::io::{Read, Write};

use syscall::data::Packet;
use syscall::scheme::SchemeBlockMut;

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

            if let Some(a) = scheme.handle(&mut packet) {
                packet.a = a;
                socket.write(&packet).expect("pty: failed to write responses to pty scheme");
            } else {
                todo.push(packet);
            }

            let mut i = 0;
            while i < todo.len() {
                if let Some(a) = scheme.handle(&mut todo[i]) {
                    let mut packet = todo.remove(i);
                    packet.a = a;
                    socket.write(&packet).expect("pty: failed to write responses to pty scheme");
                } else {
                    i += 1;
                }
            }

            for (id, handle) in scheme.handles.iter_mut() {
                let events = handle.events();
                if events > 0 {
                    post_fevent(&mut socket, *id, events, 1);
                }
            }
        }
    }
}

fn post_fevent(socket: &mut File, id: usize, flags: usize, count: usize) {
    socket.write(&Packet {
        id: 0,
        pid: 0,
        uid: 0,
        gid: 0,
        a: syscall::number::SYS_FEVENT,
        b: id,
        c: flags,
        d: count
    }).expect("pty: failed to write event");
}
