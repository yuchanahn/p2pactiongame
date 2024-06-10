use std::cmp::max;
use std::collections::HashMap;

use godot::engine::INode2D;
use godot::engine::Label;
use godot::engine::Node2D;
use godot::prelude::*;

use crate::game_manager::GAME_TICK;
use crate::network_controller::NetworkController;
use crate::player::Player;
use crate::udp_net::pack;
use crate::udp_net::InputPacket;
use crate::udp_net::PacketType;
use crate::utils::minus;

pub const INPUT_SIZE: usize = 30;
pub const INPUT_DELAY: usize = 3;

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct InputController {
    base: Base<Node2D>,
    nc: Option<Gd<NetworkController>>,
    gui_text_keypress: Option<Gd<Label>>,
    pub inputs: HashMap<u64, u8>,
}

#[godot_api]
impl INode2D for InputController {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            nc: None,
            gui_text_keypress: None,
            inputs: HashMap::new(),
        }
    }

    fn ready(&mut self) {
        if self.nc.is_none() {
            self.nc = self
                .base()
                .get_tree()
                .unwrap()
                .get_root()
                .unwrap()
                .try_get_node_as::<NetworkController>("Root/NetworkController");
        }
        if self.gui_text_keypress.is_none() {
            self.gui_text_keypress = self.base().try_get_node_as::<Label>("UI_Text_Keypress");
        }

        self.base_mut().set_physics_process_priority(-6);
    }

    fn physics_process(&mut self, _delta: f64) {
        let input = Input::singleton();
        let mut input2send: u8 = 0;
        let mut key_str = "".to_string();
        let tick = GAME_TICK.lock().unwrap().tick + INPUT_DELAY as u64;

        if input.is_action_pressed("d".into()) {
            input2send |= 0b0001;
            key_str.push_str("d");
        }
        if input.is_action_pressed("a".into()) {
            input2send |= 0b0010;
            key_str.push_str("a");
        }
        if input.is_action_pressed("w".into()) {
            input2send |= 0b0100;
            key_str.push_str("w");
        }
        if input.is_action_pressed("attack".into()) {
            input2send |= 0b1000;
            key_str.push_str("mouse1");
        }
        if input.is_action_pressed("roll".into()) {
            input2send |= 0b10000;
            key_str.push_str("shift");
        }
        if input.is_action_pressed("guard".into()) {
            input2send |= 0b100000;
            key_str.push_str("mouse2");
        }

        self.gui_text_keypress
            .as_mut()
            .unwrap()
            .set_text(if key_str == "" {
                "".into()
            } else {
                format!("Keypress: [{}]", key_str).into()
            });

        let mut nc = self.nc.as_mut().unwrap().bind_mut();

        if tick - INPUT_DELAY as u64 <= 0 {
            return;
        }
        
        self.inputs.insert(tick, input2send);
        self.inputs.retain(|k, _| 30 > tick - *k);

        let mut input_packet = InputPacket {
            inputs: [0; INPUT_SIZE],
            tick: max(tick, INPUT_SIZE as u64),
        };
        for (k, v) in self.inputs.iter() {
            input_packet.inputs[INPUT_SIZE - (tick - (k - 1)) as usize] = *v;
        }

        let mut packet = pack::<InputPacket>(&input_packet, PacketType::Input);
        packet.insert(0, (packet.len() + 1) as u8);
        nc.send_buffer.push(packet);
    }
}


pub fn input_to_str(input: u8) -> String {
    let mut s = "".to_string();
    if input & 0b0001 != 0 {
        s.push_str("d");
    }
    if input & 0b0010 != 0 {
        s.push_str("a");
    }
    if input & 0b0100 != 0 {
        s.push_str("w");
    }
    if input & 0b1000 != 0 {
        s.push_str("mouse1");
    }
    if input & 0b10000 != 0 {
        s.push_str("shift");
    }
    if input & 0b100000 != 0 {
        s.push_str("mouse2");
    }
    s
}