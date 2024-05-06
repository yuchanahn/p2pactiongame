use core::panic;
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
use crate::game_manager::GAME_TICK;
use crate::gui_player_state::GUIPlayerState;
use crate::player::EActionMessage;
use crate::player::PlayAnimationData;
use crate::player::Player;
use crate::time;
use crate::udp_net;
use crate::udp_net::Connect;
use crate::udp_net::InputOKPacket;
use crate::udp_net::TimeSync;
use crate::udp_net::{send_bytes, unpack, PacketType, Ping, Pong};
use crate::utils::plus;

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
            log_for_debug: None,
            base,
        }
    }

    fn ready(&mut self) {
        let rand = Gd::<RandomNumberGenerator>::default();

        //let port = 55555;
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
        let mut player = root_node.get_node_as::<Player>("Root/Player");
        let mut other_player = root_node.try_get_node_as::<Player>("Root/OtherPlayer");

        let cur_tick = GAME_TICK.lock().unwrap().tick;

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

                        let mut gt = game_tick.bind_mut();

                        if net_data.other_peer_endpoint.is_some() {
                            //gt.game_start_time = connect.game_start_time;
                            //godot_print!("Game Start Time : {}", gt.game_start_time);
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
                        let (mut input, _) =
                            unpack::<udp_net::InputPacket>(&buffer[1..]).expect("Failed to unpack");

                        if let Some(other_player) = other_player.as_mut() {
                            let mut other_player = other_player.bind_mut();

                            let mut last_tick = LASTTICK.lock().unwrap();

                            if *last_tick > input.tick {
                                continue;
                            } else {
                                *last_tick = input.tick;
                            }

                            if cur_tick < input.tick {
                                let delta_tick = (input.tick - cur_tick) as usize;

                                if delta_tick > 30 {
                                    godot_print!("delta time : {}", delta);
                                    godot_print!(
                                        "Delta Tick is too big : {}({}/{})",
                                        delta_tick,
                                        cur_tick,
                                        input.tick
                                    );

                                    continue;
                                }

                                let mut now_input = input.inputs.clone();
                                for i in 0..delta_tick {
                                    now_input[i] = other_player.predicted_inputs[i];
                                }
                                for i in delta_tick..30 {
                                    now_input[i] = input.inputs[i - delta_tick];
                                }
                                input.tick = cur_tick;
                                input.inputs = now_input;
                            }

                            let delta_tick = (cur_tick - input.tick) as usize;
                            let offset = 30 - delta_tick;

                            let mut rollback_tick: Option<u64> = None;

                            for i in 0..offset {
                                other_player.real_inputs[i] = Some(input.inputs[i + delta_tick]);
                            }
                            for i in offset..30 {
                                other_player.real_inputs[i] = None;
                            }
                            for i in 0..offset {
                                if other_player.predicted_inputs[i]
                                    != other_player.real_inputs[i].unwrap()
                                {
                                    rollback_tick = Some(input.tick - (offset - i) as u64);
                                    break;
                                }
                            }

                            let pred = other_player.real_inputs[offset - 1].unwrap();
                            other_player.predicted_inputs =
                                other_player.real_inputs.clone().map(|x| x.unwrap_or(pred));

                            if let Some(rollback_tick) = rollback_tick {
                                other_player.show_rollback_text();
                                //최종 롤백 상태로 복구
                                other_player.restore_state(rollback_tick - 1);
                                other_player.rollback_states = [None; 30];
                                let mut player = player.clone();
                                let mut local_player = player.bind_mut();

                                local_player.restore_state(rollback_tick - 1);

                                for i in (rollback_tick)..cur_tick {
                                    let actions =
                                        other_player.simulated_tick(local_player.to_gd(), i, delta);
                                    for act in actions {
                                        match act {
                                            EActionMessage::Damaged => {
                                                local_player.stat.health -= 10.0;
                                                if local_player.stat.health <= 0.0 {
                                                    local_player.anim_data =
                                                        Some(PlayAnimationData {
                                                            name: "die".into(),
                                                            started_at: cur_tick,
                                                            looped: false,
                                                        });
                                                } else {
                                                    local_player.anim_data =
                                                        Some(PlayAnimationData {
                                                            name: "hit".into(),
                                                            started_at: cur_tick,
                                                            looped: false,
                                                        });
                                                }
                                            }
                                        }
                                    }
                                    other_player.push_rollback_state(i);

                                    let actions =
                                        local_player.simulated_tick(other_player.to_gd(), i, delta);
                                    for act in actions {
                                        match act {
                                            EActionMessage::Damaged => {
                                                other_player.stat.health -= 10.0;
                                                if other_player.stat.health <= 0.0 {
                                                    other_player.anim_data =
                                                        Some(PlayAnimationData {
                                                            name: "die".into(),
                                                            started_at: cur_tick,
                                                            looped: false,
                                                        });
                                                } else {
                                                    other_player.anim_data =
                                                        Some(PlayAnimationData {
                                                            name: "hit".into(),
                                                            started_at: cur_tick,
                                                            looped: false,
                                                        });
                                                }
                                            }
                                        }
                                    }
                                    local_player.push_rollback_state(i);
                                }
                            }
                        }
                    }
                    PacketType::InputOK => {}
                    _ => {}
                }
            }
        }

        if let Some(other_player) = other_player.as_mut() {
            let mut other_player = other_player.bind_mut();

            let predicted_input = other_player.predicted_inputs.last().unwrap().clone();
            let len = other_player.predicted_inputs.len();
            for i in 1..len {
                other_player.predicted_inputs[i - 1] = other_player.predicted_inputs[i];
            }
            other_player.predicted_inputs[len - 1] = predicted_input;

            let actions = other_player.simulated_tick(player.clone(), cur_tick, delta);

            for act in actions {
                match act {
                    EActionMessage::Damaged => {
                        player.clone().bind_mut().stat.health -= 10.0;
                        if player.clone().bind_mut().stat.health <= 0.0 {
                            player.clone().bind_mut().anim_data = Some(PlayAnimationData {
                                name: "die".into(),
                                started_at: cur_tick,
                                looped: false,
                            });
                        } else {
                            player.clone().bind_mut().anim_data = Some(PlayAnimationData {
                                name: "hit".into(),
                                started_at: cur_tick,
                                looped: false,
                            });
                        }
                    }
                }
            }

            other_player.push_rollback_state(cur_tick);
            for i in 0..29 {
                other_player.rollback_states[i] = other_player.rollback_states[i + 1];
            }

            let mut local_player = player.bind_mut();

            let actions = local_player.simulated_tick(other_player.to_gd(), cur_tick, delta);

            for act in actions {
                match act {
                    EActionMessage::Damaged => {
                        other_player.stat.health -= 10.0;
                        if other_player.stat.health <= 0.0 {
                            other_player.anim_data = Some(PlayAnimationData {
                                name: "die".into(),
                                started_at: cur_tick,
                                looped: false,
                            });
                        } else {
                            other_player.anim_data = Some(PlayAnimationData {
                                name: "hit".into(),
                                started_at: cur_tick,
                                looped: false,
                            });
                        }
                    }
                }
            }

            local_player.push_rollback_state(cur_tick);
            for i in 0..29 {
                local_player.rollback_states[i] = local_player.rollback_states[i + 1];
            }
        }
    }

    fn process(&mut self, _: f64) {
        self.start_send_process();
    }
}
