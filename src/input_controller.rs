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

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct InputController {
    base: Base<Node2D>,
    nc: Option<Gd<NetworkController>>,
    gui_text_keypress: Option<Gd<Label>>,
    pub local_input: u8,
}

#[godot_api]
impl INode2D for InputController {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            nc: None,
            gui_text_keypress: None,
            local_input: 0,
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

        let mut local_player = self
            .base()
            .get_tree()
            .unwrap()
            .get_root()
            .unwrap()
            .get_node_as::<Player>("Root/Player");

        let mut input2send: u8 = 0;
        let mut key_str = "".to_string();
        let tick = GAME_TICK.lock().unwrap().tick;

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

        self.gui_text_keypress
            .as_mut()
            .unwrap()
            .set_text(if key_str == "" {
                "".into()
            } else {
                format!("Keypress: [{}]", key_str).into()
            });

        let mut nc = self.nc.as_mut().unwrap().bind_mut();
        let nd = nc.net.as_mut().unwrap();
        //let dt = 1000.0 / 60.0;
        
        //if input2send == 0 {
        //    self.local_input = input2send;
        //    return;
        //}

        if tick <= 0 {
            self.local_input = input2send;
            return;
        }
        
        local_player.bind_mut().push_input(input2send);
        let inputs = local_player.bind_mut().real_inputs.clone().map(|x| x.unwrap_or(0));
        
        let input_packet = InputPacket {
            inputs: inputs.try_into().unwrap(),
            tick: tick,
        };

        let mut packet = pack::<InputPacket>(&input_packet, PacketType::Input);
        packet.insert(0, (packet.len() + 1) as u8);
        nc.send_buffer.push(packet);

        self.local_input = input2send;
    }
}
