use std::clone;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::ops::Deref;
use std::sync::Mutex;
use std::sync::MutexGuard;

use godot::engine::INode2D;
use godot::engine::Node2D;
use godot::engine::RandomNumberGenerator;
use godot::prelude::*;

use crate::game_manager::GameTick;
use crate::gui_player_state::GUIPlayerState;
use crate::player::Player;
use crate::time;
use crate::udp_net;
use crate::udp_net::Connect;
use crate::udp_net::InputOKPacket;
use crate::udp_net::{send_bytes, unpack, PacketType, Ping, Pong};

use lazy_static::lazy_static;

lazy_static! {
    pub static ref RAW_PACKETS: Mutex<HashMap<SocketAddr, Vec<Vec<u8>>>> =
        Mutex::new(HashMap::new());
}

pub struct NetData {
    pub socket: Option<std::net::UdpSocket>,
    pub network_latency: u64,
    pub other_peer_endpoint: Option<String>,
    pub my_port: i32,
}

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct NetworkController {
    pub net: Option<NetData>,
    ids: HashMap<u8, u64>,
    time2ping: Option<u64>,
    ping_counter: u8,
    thread: Option<std::thread::JoinHandle<()>>,
    pub send_buffer: Vec<Vec<u8>>,
    base: Base<Node2D>,
}

impl NetworkController {
    pub fn get_socket(&self) -> Option<&std::net::UdpSocket> {
        self.net.as_ref().unwrap().socket.as_ref()
    }

    pub fn start_send_process(&mut self) {
        let net_data = self.net.as_ref().unwrap();
        net_data.other_peer_endpoint.is_some().then(|| {
            if self.send_buffer.is_empty() {
                return;
            }
            let pks: Vec<_> = self
                .send_buffer
                .iter()
                .flat_map(|v| v.iter())
                .cloned()
                .collect();
            send_bytes(
                net_data.socket.as_ref(),
                pks.as_slice(),
                net_data.other_peer_endpoint.as_ref().unwrap().as_str(),
            );
            self.send_buffer.clear();
        });
    }

    pub fn send_to(&self, packet: &[u8], endpoint: &str) {
        let net_data = self.net.as_ref().unwrap();
        send_bytes(net_data.socket.as_ref(), packet, endpoint);
    }
}

#[godot_api]
impl INode2D for NetworkController {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            net: None,
            ids: HashMap::new(),
            time2ping: None,
            ping_counter: 0,
            thread: None,
            send_buffer: Vec::new(),
            base,
        }
    }

    fn ready(&mut self) {
        let rand = Gd::<RandomNumberGenerator>::default();
        let port = 5000 + (rand.clone().randi_range(50000, 60000));
        let socket = udp_net::start_udp(port as u16).expect("Failed to start UDP");
        let socket_for_thread = socket.try_clone();
        self.net = Some(NetData {
            socket: Some(socket),
            network_latency: 0,
            other_peer_endpoint: None,
            my_port: port,
        });

        self.thread = Some(std::thread::spawn(move || {
            let socket = socket_for_thread.unwrap();
            let mut buffer = [0; 1024];
            loop {
                let result = socket.recv_from(&mut buffer);
                match result {
                    Ok((size, addr)) => {
                        let mut raw_packets = RAW_PACKETS.lock().unwrap();
                        if !(*raw_packets).contains_key(&addr) {
                            (*raw_packets).insert(addr, Vec::new());
                        }
                        let mut i = 0;
                        while i < size {
                            let pkt_size = buffer[i] as usize;
                            if let Some(vec_ref) = (*raw_packets).get_mut(&addr) {
                                vec_ref.push(buffer[(i + 1)..(i + pkt_size) as usize].to_vec());
                                i += pkt_size as usize;
                            }
                        }
                        drop(raw_packets);
                    },
                    Err(ref err) if err.kind() != ErrorKind::WouldBlock => {
                        godot_print!("Something went wrong: {}", err)
                    }
                    _ => {}
                }
            }
        }));

        godot_print!("Network Controller Ready");
    }

    fn physics_process(&mut self, _: f64) {
        let root_node = self.base().get_tree().unwrap().get_root().unwrap();
        let mut game_tick = root_node.get_node_as::<GameTick>("Root/GameTick");
        let mut root = self.base().get_node_as::<Node2D>("../");
        let mut player = root_node.get_node_as::<Player>("Root/Player");
        let mut other_player = root_node.try_get_node_as::<Player>("Root/OtherPlayer");

        let net_data = self.net.as_mut().unwrap();
        let timestamp = time::get_ms_timestamp();

        if net_data.other_peer_endpoint.is_some() {
            if self.time2ping.is_none() || timestamp - self.time2ping.unwrap() > 1000 {
                let ping = Ping {
                    id: self.ping_counter as u8,
                };
                self.ids.insert(ping.id, timestamp);
                let mut packet = udp_net::pack::<Ping>(&ping, PacketType::Ping);

                packet.insert(0, (packet.len() + 1) as u8);
                self.send_buffer.push(packet);

                self.time2ping = Some(timestamp);
                self.ping_counter = self.ping_counter.wrapping_add(1);

                godot_print!("Sent ping packet : {}", ping.id);
            }
        }
        
        let mut raw = RAW_PACKETS.lock().unwrap();
        
        let packets = (*raw).clone();
        (*raw).clear();
        for (addr, q) in packets {
            for pkt in q {
                let buffer = pkt.as_slice();
                match PacketType::from(buffer[0]) {
                    PacketType::Ping => {
                        let (ping, _) = unpack::<Ping>(&buffer[1..]).expect("Failed to unpack");
                        godot_print!("Ping packet received : {}", ping.id);

                        let pong = Pong { id: ping.id };
                        let mut packet = udp_net::pack::<Pong>(&pong, PacketType::Pong);

                        packet.insert(0, (packet.len() + 1) as u8);
                        self.send_buffer.push(packet);
                    }
                    PacketType::Pong => {
                        let (pong, _) = unpack::<Pong>(&buffer[1..]).expect("Failed to unpack");
                        if self.ids.get(&pong.id).is_none() {
                            godot_print!("Unknown Pong packet received : {}", pong.id);
                            godot_print!("IDs : {:?}", self.ids);
                            return;
                        }
                        let latency = time::get_ms_timestamp() - self.ids.get(&pong.id).unwrap();
                        self.ids.remove(&pong.id);

                        net_data.network_latency = latency;

                        godot_print!("Pong packet received : {}", pong.id);
                    }
                    PacketType::Connect => {
                        let (connect, _) =
                            unpack::<Connect>(&buffer[1..]).expect("Failed to unpack");

                        game_tick.bind_mut().game_start_time = connect.game_start_time;

                        if net_data.other_peer_endpoint.is_some() {
                            return;
                        }
                        if let Ok(scene) = try_load::<PackedScene>("res://Player/player.tscn") {
                            let other_player = scene.instantiate_as::<Player>();
                            let mut other = other_player.clone();

                            root.add_child(other_player.upcast::<Node>());
                            other.set_position(Vector2::new(connect.x, connect.y));
                            other.set_name("OtherPlayer".into());

                            if let Ok(scene) = try_load::<PackedScene>("res://PlayerState.tscn") {
                                let mut player_state = scene.instantiate_as::<GUIPlayerState>();
                                player_state
                                    .bind_mut()
                                    .set_target(other.clone().upcast::<Node2D>());
                                root.add_child(player_state.upcast::<Node>());
                            }
                        }

                        net_data.other_peer_endpoint = Some(addr.to_string());
                        godot_print!("Connected to : {}", addr);

                        let pos = player.get_position();

                        let mut packet = udp_net::pack::<Connect>(
                            &Connect {
                                x: pos.x,
                                y: pos.y,
                                game_start_time: connect.game_start_time,
                            },
                            PacketType::Connect,
                        );
                        packet.insert(0, (packet.len() + 1) as u8);
                        self.send_buffer.push(packet);
                    }
                    PacketType::Input => {
                        let (input, _) =
                            unpack::<udp_net::InputPacket>(&buffer[1..]).expect("Failed to unpack");

                        if let Some(other_player) = other_player.as_mut() {
                            let mut other_player = other_player.bind_mut();
                            for i in 0..5 {
                                other_player.push_input(input.input[i], input.tick[i]);
                                other_player.push_input_ok(input.tick[i]);
                            }
                        }

                        let mut packet = udp_net::pack::<InputOKPacket>(
                            &InputOKPacket { tick: input.tick },
                            PacketType::InputOK,
                        );
                        packet.insert(0, (packet.len() + 1) as u8);
                        self.send_buffer.push(packet);
                    }
                    PacketType::InputOK => {
                        let (input_ok, _) =
                            unpack::<InputOKPacket>(&buffer[1..]).expect("Failed to unpack");

                        for i in 0..5 {
                            let mut local_player = player.bind_mut();
                            local_player.push_input_ok(input_ok.tick[i]);
                        }
                    }
                }
            }
        }
    }

    fn process(&mut self, _: f64) {
        self.start_send_process();
    }
}
