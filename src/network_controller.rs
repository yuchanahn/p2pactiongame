use std::cmp::min;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::net::SocketAddr;
use std::net::UdpSocket;
use std::sync::Mutex;

use godot::engine::INode2D;
use godot::engine::Node2D;
use godot::engine::Window;
use godot::prelude::*;

use crate::animation_controller::PlayAnimationData;
use crate::game_manager::GameTick;
use crate::game_manager::GAME_TICK;
use crate::gui_player_state::GUIPlayerState;
use crate::input_controller;
use crate::input_controller::INPUT_SIZE;
use crate::player::Player;
use crate::time;
use crate::udp_net;
use crate::udp_net::pack;
use crate::udp_net::Connect;
use crate::udp_net::TimeSync;
use crate::udp_net::{send_bytes, unpack, Endpoint, PacketType, Ping, Pong};
use crate::utils::plus;
use crate::world::simulate_world;
use crate::world::simulate_world_range;
use crate::world::update_all_player_gd;
use crate::world::PlayerData;
use crate::world::WorldData;

use lazy_static::lazy_static;

lazy_static! {
    pub static ref RAW_PACKETS: Mutex<HashMap<SocketAddr, Vec<Vec<u8>>>> =
        Mutex::new(HashMap::new());
    pub static ref LASTTICK: Mutex<u64> = Mutex::new(0);
    pub static ref LAST_ROLLBACK_TICK: Mutex<u64> = Mutex::new(0);
    pub static ref TIMESTAMP_FOR_TIMESYNC: Mutex<u64> = Mutex::new(0);
    pub static ref TIMESYNC_OFFSET: Mutex<i32> = Mutex::new(0);
    pub static ref TIME_BASED: Mutex<bool> = Mutex::new(false);
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
    pub log_for_debug: Option<String>,
    pub time_out1: u64,
    pub time_out2: u64,
    pub time_out3: u64,
    pub peer_addr: Option<Endpoint>,
    pub peer_endpoint: Option<SocketAddr>,
    pub player_input: HashMap<u64, HashMap<u64, u8>>,
    pub world_snapshot: HashMap<u64, WorldData>,
    pub world: WorldData,
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

    pub fn connect_to_server(&mut self) {
        let mut sock = self.net.as_mut().unwrap().socket.as_ref().unwrap();
        let local_addr = sock.local_addr().unwrap();
        let mut packet = pack::<SocketAddr>(&local_addr, PacketType::RegisterEndpoint);
        packet.insert(0, (packet.len() + 1) as u8);
        send_bytes(Some(&sock), &packet, "13.124.53.124:55555");
    }
}

pub fn connect_process(root_node: Gd<Window>, sock: Option<&UdpSocket>, target_addr: SocketAddr) {
    let mut player = root_node.get_node_as::<Node2D>("Root/Player");
    let pos = player.get_position();

    player.set_position(Vector2::new(393.0, pos.y));

    let game_start_time = time::get_ms_timestamp() + 3000;
    let mut game_tick = root_node.get_node_as::<GameTick>("Root/GameTick");
    game_tick.bind_mut().game_start_time = game_start_time;

    let mut packet = udp_net::pack::<Connect>(
        &Connect {
            x: 393.0,
            y: pos.y,
            game_start_time,
        },
        PacketType::Connect,
    );
    packet.insert(0, (packet.len() + 1) as u8);

    *TIME_BASED.lock().unwrap() = true;
    *TIMESTAMP_FOR_TIMESYNC.lock().unwrap() = time::get_ms_timestamp();
    drop(TIMESTAMP_FOR_TIMESYNC.lock().unwrap());
    drop(TIME_BASED.lock().unwrap());

    let pkt: TimeSync = TimeSync {
        time: time::get_ms_timestamp(),
    };
    let mut pkt = udp_net::pack::<TimeSync>(&pkt, PacketType::TimeSync);
    pkt.insert(0, (pkt.len() + 1) as u8);

    let ep = target_addr.to_string();

    send_bytes(sock, pkt.as_slice(), ep.as_str());
    send_bytes(sock, packet.as_slice(), ep.as_str());
}

impl NetworkController {
    pub fn update_world_process(&mut self, cur_tick: u64, input_controller: Gd<input_controller::InputController>, my_port: u64) {
        let mut current_inputs = HashMap::new();
        let port = self.peer_addr.as_ref().unwrap().addr.port();

        let mut player_input = self.player_input.clone();
        let target_player_input = player_input.get(&(port as u64)).unwrap().clone();

        if !target_player_input.contains_key(&cur_tick) {
            self.world_snapshot.insert(cur_tick, self.world.clone());
            let inputs_in_other_player = player_input.get_mut(&(port as u64)).unwrap();
            inputs_in_other_player.insert(cur_tick, inputs_in_other_player.get(&(cur_tick - 1)).or(Some(&0)).unwrap().clone());
        }

        //Local Player Input Update
        let local_input = input_controller.bind().inputs.get(&cur_tick).unwrap_or(&0).clone();
        player_input.get_mut(&my_port).unwrap().insert(cur_tick, local_input);

        self.player_input = player_input;

        for (k, v) in self.player_input.iter() {
            current_inputs.insert(*k, v[&cur_tick]);
        }

        self.world = simulate_world(self.world.clone(), current_inputs, cur_tick);
        update_all_player_gd(&mut self.world);
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
            log_for_debug: None,
            time_out1: 0,
            time_out2: 0,
            time_out3: 0,
            peer_addr: None,
            peer_endpoint: None,
            player_input: HashMap::new(),
            world_snapshot: HashMap::new(),
            world: WorldData {
                players: HashMap::new(),
            },
            base,
        }
    }

    fn ready(&mut self) {
        let socket = udp_net::start_udp(0).expect("Failed to start UDP");
        socket
            .connect("13.124.53.124:55555")
            .expect("connect function failed");
        let port = socket.local_addr().unwrap().port() as i32;
        let socket_for_thread = socket.try_clone();

        godot_print!(
            "UDP on : {} | {}",
            socket.peer_addr().unwrap(),
            socket.local_addr().unwrap()
        );

        self.net = Some(NetData {
            socket: Some(socket),
            network_latency: 0,
            other_peer_endpoint: None,
            my_port: port,
        });

        let root_node = self.base().get_tree().unwrap().get_root().unwrap();
        let my_player = root_node.get_node_as::<Player>("Root/Player");

        self.world.players.insert(port as u64, 
        PlayerData {
            pos: Vector2::new(-300.0, 65.0),
            vel: Vector2::new(0.0, 0.0),
            attack_cooldown: 0.0,
            health: 100.0,
            anim_data: Some(PlayAnimationData {
                name: "idle".into(),
                started_at: 0,
                looped: true,
            }),
            object: Some(my_player.clone()),
        }
        );
        self.player_input.insert(port as u64, HashMap::new());

        self.thread = Some(std::thread::spawn(move || {
            let socket = socket_for_thread.unwrap();
            let mut buffer = [0; 1024];
            loop {
                let result = socket.recv_from(&mut buffer);
                match result {
                    Ok((size, addr)) => {
                        //godot_print!("is it working? : {:?}", PacketType::from(buffer[1] as u8));
                        let mut raw_packets = RAW_PACKETS.lock().unwrap();
                        if !(*raw_packets).contains_key(&addr) {
                            (*raw_packets).insert(addr, Vec::new());
                        }
                        let mut i = 0;
                        while i < size {
                            let pkt_size = buffer[i] as usize;
                            if let Some(vec_ref) = (*raw_packets).get_mut(&addr) {
                                if buffer[i + 1] as u8 == PacketType::TimeSync as u8 {
                                    let timestamp_for_timesync =
                                        TIMESTAMP_FOR_TIMESYNC.lock().unwrap().clone();
                                    let timesync_offset = TIMESYNC_OFFSET.lock().unwrap().clone();
                                    if timestamp_for_timesync == 0 {
                                        let pkt: TimeSync = TimeSync {
                                            time: time::get_ms_timestamp(),
                                        };
                                        let mut packet =
                                            udp_net::pack::<TimeSync>(&pkt, PacketType::TimeSync);
                                        packet.insert(0, (packet.len() + 1) as u8);
                                        send_bytes(
                                            Some(&socket),
                                            packet.as_slice(),
                                            addr.to_string().as_str(),
                                        );
                                        *TIMESTAMP_FOR_TIMESYNC.lock().unwrap() =
                                            time::get_ms_timestamp();
                                        drop(TIMESTAMP_FOR_TIMESYNC.lock().unwrap());
                                    } else if timesync_offset == 0 {
                                        let (time, _) = unpack::<TimeSync>(
                                            &buffer[(i + 2)..(i + pkt_size) as usize],
                                        )
                                        .expect("Failed to unpack");
                                        let rtt = (time::get_ms_timestamp()
                                            - *TIMESTAMP_FOR_TIMESYNC.lock().unwrap())
                                            / 2;
                                        let order_time_stamp = time.time + rtt;
                                        let timestamp = time::get_ms_timestamp();
                                        let mut offset = 0;
                                        if order_time_stamp > timestamp {
                                            offset = -((order_time_stamp - timestamp) as i32);
                                        } else {
                                            offset = (timestamp - order_time_stamp) as i32;
                                        }

                                        *TIMESYNC_OFFSET.lock().unwrap() = offset;
                                        drop(TIMESYNC_OFFSET.lock().unwrap());
                                        godot_print!("TIMESYNC_OFFSET : {}", offset);
                                        let pkt: TimeSync = TimeSync {
                                            time: time::get_ms_timestamp(),
                                        };
                                        let mut packet =
                                            udp_net::pack::<TimeSync>(&pkt, PacketType::TimeSync);
                                        packet.insert(0, (packet.len() + 1) as u8);
                                        send_bytes(
                                            Some(&socket),
                                            packet.as_slice(),
                                            addr.to_string().as_str(),
                                        );
                                    }
                                }
                                vec_ref.push(buffer[(i + 1)..(i + pkt_size) as usize].to_vec());
                                i += pkt_size as usize;
                            }
                        }
                        drop(raw_packets);
                    }
                    Err(ref err) if err.kind() != ErrorKind::WouldBlock => {
                        godot_print!("Something went wrong: {}", err)
                    }
                    _ => {}
                }
            }
        }));

        self.base_mut().set_physics_process_priority(-5);

        godot_print!("Network Controller Ready");
    }

    

    fn physics_process(&mut self, delta: f64) {
        let root_node = self.base().get_tree().unwrap().get_root().unwrap();
        let mut game_tick = root_node.get_node_as::<GameTick>("Root/GameTick");
        let mut root = self.base().get_node_as::<Node2D>("../");
        let input_controller = root_node.get_node_as::<input_controller::InputController>("Root/InputController");

        let cur_tick = GAME_TICK.lock().unwrap().tick;

        let net_data = self.net.as_mut().unwrap();
        let my_port = net_data.my_port;
        let timestamp = time::get_ms_timestamp();

        // PING PONG
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

        // HOLE PUNCH
        if let Some(peer) = self.peer_addr.as_ref() {
            if (self.time_out1 != 0) && (self.time_out1 < timestamp) && self.peer_endpoint.is_none()
            {
                godot_print!("HolePunch packet send to : {}", peer.addr);

                let mut packet = pack::<u8>(&1, PacketType::HolePunch);
                packet.insert(0, (packet.len() + 1) as u8);

                let sock = net_data.socket.as_ref();

                sock.unwrap().connect(peer.addr).expect("Failed to connect");
                send_bytes(sock, &packet, peer.addr.to_string().as_str());

                self.time_out1 = 0;
                self.time_out2 = timestamp + 1000;
            }
            if (self.time_out2 != 0) && self.time_out2 < timestamp && self.peer_endpoint.is_none() {
                godot_print!("HolePunch packet send2 to : {}", peer.addr);

                let mut packet = pack::<u8>(&1, PacketType::HolePunch);
                packet.insert(0, (packet.len() + 1) as u8);

                let sock = net_data.socket.as_ref();

                sock.unwrap().connect(peer.addr).expect("Failed to connect");
                send_bytes(sock, &packet, peer.addr.to_string().as_str());

                //now relay server mode...
                self.time_out2 = 0;
                self.time_out3 = timestamp + 1000;
            }
            if self.time_out3 != 0 && self.time_out3 < timestamp && self.peer_endpoint.is_none() {
                godot_print!("Relay Server Start : {}", peer.addr);

                self.peer_endpoint = "13.124.53.124:55555".parse().ok();

                net_data
                    .socket
                    .as_ref()
                    .unwrap()
                    .connect("13.124.53.124:55555")
                    .expect("connect function failed");

                let connect_packet_send = peer.addr.port()
                    > net_data
                        .socket
                        .as_ref()
                        .unwrap()
                        .local_addr()
                        .unwrap()
                        .port();
                if connect_packet_send {
                    connect_process(
                        root_node.clone(),
                        net_data.socket.as_ref(),
                        self.peer_endpoint.unwrap(),
                    );
                }
                self.time_out3 = 0;
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

                        let mut gt = game_tick.bind_mut();

                        if net_data.other_peer_endpoint.is_some() {
                            return;
                        } else {
                            gt.game_start_time =
                                plus(connect.game_start_time, *TIMESYNC_OFFSET.lock().unwrap());
                            godot_print!("Game Start Time : {}", connect.game_start_time);
                        }
                        if let Ok(scene) = try_load::<PackedScene>("res://Player/player.tscn") {
                            let other_player = scene.instantiate_as::<Player>();
                            let mut other = other_player.clone();

                            root.add_child(other_player.upcast::<Node>());
                            other.set_position(Vector2::new(connect.x, connect.y));
                            other.set_name("OtherPlayer".into());

                            let other_player_id = self.peer_addr.as_ref().unwrap().addr.port() as u64;
                            self.world.players.insert(other_player_id, 
                            PlayerData {
                                pos: Vector2::new(connect.x, connect.y),
                                vel: Vector2::new(0.0, 0.0),
                                attack_cooldown: 0.0,
                                health: 100.0,
                                anim_data: Some(PlayAnimationData {
                                    name: "idle".into(),
                                    started_at: cur_tick,
                                    looped: true,
                                }),
                                object: Some(other.clone()),
                            }
                            );
                            self.player_input.insert(other_player_id, HashMap::new());
                            godot_print!("Other Player Connected : {}", other_player_id);


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

                        let mut packet = udp_net::pack::<Connect>(
                            &Connect {
                                x: -393.0,
                                y: 65.0,
                                game_start_time: connect.game_start_time,
                            },
                            PacketType::Connect,
                        );
                        packet.insert(0, (packet.len() + 1) as u8);
                        self.send_buffer.push(packet);

                        let mut my_player = self.world.players.get_mut(&(my_port as u64)).unwrap().clone();
                        my_player.pos = my_player.object.clone().unwrap().get_position();
                        self.world.players.insert(my_port as u64, my_player.clone());
                    }
                    PacketType::Input => {
                        let (input, _) =
                            unpack::<udp_net::InputPacket>(&buffer[1..]).expect("Failed to unpack");

                        if cur_tick == 0 {
                            return;
                        }

                        let mut inputs = self.player_input.get(&(addr.port() as u64)).unwrap().clone();
                        let input_start_tick = input.tick - (INPUT_SIZE - 1) as u64;
                        let mut rollback_tick: Option<u64> = None;

                        for i in 0..min(cur_tick, INPUT_SIZE as u64) as usize {
                            let key = input_start_tick + i as u64;
                            if inputs.contains_key(&key) {
                                if rollback_tick.is_none() && inputs[&key] != input.inputs[i] {
                                    rollback_tick = Some(key);
                                }
                            }
                            inputs.insert(key, input.inputs[i]);
                        }

                        for i in input.tick..(cur_tick-1) {
                            inputs.insert(i, input.inputs[29]);
                        }

                        let port = self.peer_addr.as_ref().unwrap().addr.port();
                        self.player_input.insert(port as u64, inputs);
                        let snapshot = self.world_snapshot.clone();
                        self.world_snapshot.retain(|k, _| k > &input.tick);
                        

                        if let Some(rollback_tick) = rollback_tick {
                            let mut other_player = root_node.get_node_as::<Player>("Root/OtherPlayer");
                            other_player.bind_mut().show_rollback_text();

                            let rollback_world = snapshot[&rollback_tick].clone();

                            let (world, snapshot) = simulate_world_range(
                                rollback_world.clone(),
                                self.player_input.clone(),
                                rollback_tick,
                                cur_tick,
                                input.tick,
                            );

                            self.world = world;
                            self.world_snapshot = snapshot;
                        }
                        update_all_player_gd(&mut self.world);
                    }
                    PacketType::InputOK => {}
                    PacketType::GetEndpoint => {
                        let (ep, _) = unpack::<Endpoint>(&buffer[1..]).unwrap();

                        let mut packet = pack::<u8>(&0, PacketType::HolePunch);
                        packet.insert(0, (packet.len() + 1) as u8);
                        let sock = net_data.socket.as_ref();
                        sock.unwrap()
                            .connect(ep.local_addr)
                            .expect("Failed to connect");
                        send_bytes(sock, &packet, ep.local_addr.to_string().as_str());

                        godot_print!("GetEndpoint packet received : {}", ep.local_addr);

                        self.peer_addr = Some(ep.clone());
                        self.time_out1 = timestamp + 3000;
                    }
                    PacketType::HolePunch => {
                        godot_print!("HolePunch packet received");
                        if self.peer_endpoint.is_some() {
                            continue;
                        }
                        if let Some(peer) = self.peer_addr.as_ref() {
                            let (type_of_addr, _) = unpack::<u8>(&buffer[1..]).unwrap();

                            let mut connect_packet_send = false;

                            if type_of_addr == 0 {
                                self.peer_endpoint = Some(peer.local_addr);
                                connect_packet_send =
                                    peer.local_addr.port() > net_data.my_port as u16;
                            } else {
                                self.peer_endpoint = Some(peer.addr);
                                connect_packet_send = peer.addr.port()
                                    > net_data
                                        .socket
                                        .as_ref()
                                        .unwrap()
                                        .local_addr()
                                        .unwrap()
                                        .port();
                            }

                            let mut packet = pack::<u8>(
                                if type_of_addr == 0 { &0 } else { &1 },
                                PacketType::HolePunch,
                            );
                            packet.insert(0, (packet.len() + 1) as u8);
                            send_bytes(
                                net_data.socket.as_ref(),
                                &packet,
                                addr.to_string().as_str(),
                            );

                            if connect_packet_send {
                                connect_process(
                                    root_node.clone(),
                                    net_data.socket.as_ref(),
                                    self.peer_endpoint.unwrap(),
                                );

                                godot_print!("Connected to : {}", self.peer_endpoint.unwrap());
                            };
                        }
                    }
                    _ => {}
                }
            }
        }
        
        if self.peer_addr.is_none() || cur_tick == 0 {
            return;
        }

        self.update_world_process(cur_tick, input_controller, my_port as u64);
    }

    fn process(&mut self, _: f64) {
        self.start_send_process();
    }
}
