use std::mem;
use std::net::UdpSocket;
use std::io;

use godot::log::godot_print;


#[repr(u8)]
#[derive(Debug)]
pub enum PacketType {
    Ping,
    Pong,
    Connect,
    Input,
    InputOK,
    TimeSync
}

impl From<u8> for PacketType {
    fn from(v: u8) -> Self {
        match v {
            0 => PacketType::Ping,
            1 => PacketType::Pong,
            2 => PacketType::Connect,
            3 => PacketType::Input,
            4 => PacketType::InputOK,
            5 => PacketType::TimeSync,
            _ => panic!("Unknown packet type")
        }
    }
}


pub struct Ping {
    pub id: u8
}

pub struct Pong {
    pub id: u8
}

pub struct Connect {
    pub x: f32,
    pub y: f32,
    pub game_start_time: u64
}

pub struct InputPacket {
    pub inputs: [u8; 30],
    pub tick: u64,
}

pub struct InputOKPacket {
    pub tick: [u64; 30]
}

pub struct TimeSync {
    pub time: u64,
}

//Error Type for unpacking
#[derive(Debug)]
pub enum UnpackError {
    InvalidSize
}

pub fn unpack<T>(data: &[u8]) -> Result<(T, u32), UnpackError>
{
    if data.len() < mem::size_of::<T>() {
        return Err(UnpackError::InvalidSize);
    }

    let my_struct: T;
    let raw: &[u8] = &data[..mem::size_of::<T>()];
    unsafe {
        let ptr = raw.as_ptr() as *const T;
        my_struct = ptr.read();
    }
    Ok((my_struct, mem::size_of::<T>() as u32))
}

pub fn pack<T>(data: &T, packet_type: PacketType) -> Vec<u8> {
    let mut packet = Vec::new();
    packet.push(packet_type as u8);
    let data_ptr: *const u8 = data as *const T as *const u8;
    let data_size = mem::size_of::<T>();
    unsafe {
        let data_slice = std::slice::from_raw_parts(data_ptr, data_size);
        packet.extend_from_slice(data_slice);
    }
    packet
}

pub fn start_udp(port: u16) -> io::Result<UdpSocket> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))?;
    godot_print!("UDP socket started on port {}", port);
    //socket.set_nonblocking(true)?;
    Ok(socket)
}

pub fn send_bytes(socket: Option<&UdpSocket>, packet: &[u8], addr: &str) {
    let r = socket.unwrap().send_to(&packet, addr);
    if r.is_err() {
        godot_print!("Failed to send message : {}", r.err().unwrap());
    }
}