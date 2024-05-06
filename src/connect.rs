use godot::engine::INode2D;
use godot::engine::Label;
use godot::engine::Node2D;
use godot::engine::TextEdit;
use godot::prelude::*;

use crate::game_manager::GameTick;
use crate::game_manager::GAME_TICK;
use crate::network_controller::NetworkController;
use crate::network_controller::TIMESTAMP_FOR_TIMESYNC;
use crate::network_controller::TIME_BASED;
use crate::time;
use crate::udp_net;
use crate::udp_net::send_bytes;
use crate::udp_net::Connect;
use crate::udp_net::PacketType;
use crate::udp_net::TimeSync;

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct GUIConnect {
    base: Base<Node2D>,
    text_edit: Option<Gd<TextEdit>>,
    ping_text: Option<Gd<Label>>,
    tick_text: Option<Gd<Label>>,
    nc: Option<Gd<NetworkController>>,
    init_port: bool,
}

#[godot_api]
impl INode2D for GUIConnect {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            text_edit: None,
            ping_text: None,
            tick_text: None,
            nc: None,
            init_port: false,
        }
    }

    fn ready(&mut self) {
        if self.nc.is_none() {
            self.nc = self
                .base().get_tree().unwrap().get_root().unwrap()
                .try_get_node_as::<NetworkController>("Root/NetworkController");
        }
        if self.text_edit.is_none() {
            self.text_edit = self
              .base()
              .try_get_node_as::<TextEdit>("../TextEdit");
        }
        if self.ping_text.is_none() {
            self.ping_text = self
            .base().get_tree().unwrap().get_root().unwrap()
            .try_get_node_as::<Label>("Root/UI_Text_Ping");
        }

        if self.tick_text.is_none() {
            self.tick_text = self
            .base().get_tree().unwrap().get_root().unwrap()
            .try_get_node_as::<Label>("Root/UI_Text_Tick");
        }
    }

    fn process(&mut self, _: f64) {
        if let Some(label) = self.tick_text.clone().as_mut() {
            label.set_text(format!("Tick: {}", GAME_TICK.lock().unwrap().tick).into());
        }

        if let Some(nc) = self.nc.clone() {
            if let Some(text_edit) = self.text_edit.clone().as_mut() {
                if text_edit.is_editable() {
                    if let Some(endpoint) =
                        nc.bind().net.as_ref().unwrap().other_peer_endpoint.as_ref()
                    {
                        text_edit.set_editable(false);
                        text_edit.release_focus();
                        text_edit.set_text(endpoint.as_str().into());
                    } else {
                      if text_edit.has_focus() {
                        let input = Input::singleton();
                        if input.is_action_pressed("send_chat".into()) {
                          let text = text_edit.get_text().to_string().trim().to_string();
                          let mut player = self.base().get_tree().unwrap().get_root().unwrap().get_node_as::<Node2D>("Root/Player");
                          let pos = player.get_position();
                          
                          player.set_position(Vector2::new(393.0, pos.y));

                          let game_start_time = time::get_ms_timestamp() + 3000;
                          let mut game_tick = self.base().get_tree().unwrap().get_root().unwrap().get_node_as::<GameTick>("Root/GameTick");
                          game_tick.bind_mut().game_start_time = game_start_time;

                          let mut packet = udp_net::pack::<Connect>(&Connect { x: 393.0, y: pos.y, game_start_time }, PacketType::Connect);
                          packet.insert(0, (packet.len() + 1) as u8);
                          
                          *TIME_BASED.lock().unwrap() = true;
                          *TIMESTAMP_FOR_TIMESYNC.lock().unwrap() = time::get_ms_timestamp();
                          drop(TIMESTAMP_FOR_TIMESYNC.lock().unwrap());
                          drop(TIME_BASED.lock().unwrap());
                          
                          let pkt: TimeSync = TimeSync { time: time::get_ms_timestamp() };
                          let mut pkt = udp_net::pack::<TimeSync>(&pkt, PacketType::TimeSync);
                          pkt .insert(0, (pkt .len() + 1) as u8);
                          send_bytes(nc.bind().get_socket(), pkt .as_slice(), text.as_str());
                        
                          nc.bind().send_to(packet.as_slice(), text.as_str());
                        
                          godot_print!("Sent connect packet to {}", text.as_str());
                          text_edit.set_text(text.into());
                          text_edit.release_focus();
                        }
                      } else if !self.init_port {
                        self.init_port = true;
                        let mut text = text_edit.get_text().to_string();
                        text.push_str(self.nc.as_ref().unwrap().bind().net.as_ref().unwrap().my_port.to_string().as_str());
                        text_edit.set_text(text.into());
                      } 
                    }
                }
                else {
                    if let Some(label) = self.ping_text.clone().as_mut() {
                        label.set_text(format!("Ping: {}ms", nc.bind().net.as_ref().unwrap().network_latency).into());
                    }
                }
            } else {
                godot_print!("No TextEdit found");
            }
        } else {
            godot_print!("No NetworkController found");
        }
    }
}
