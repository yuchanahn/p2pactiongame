use godot::engine::INode2D;
use godot::engine::Node2D;
use godot::engine::Button;
use godot::prelude::*;

use crate::network_controller::NetworkController;

#[derive(GodotClass, Debug)]
#[class(base=Node2D)]
pub struct MainMenu {
    button_p2p: Option<Gd<Button>>,
    button_server: Option<Gd<Button>>,
    base: Base<Node2D>,
}

#[godot_api]
impl INode2D for MainMenu {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            button_p2p: None,
            button_server: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.button_p2p = self.base().try_get_node_as::<Button>("ButtonP2P");
        self.button_server = self.base().try_get_node_as::<Button>("ButtonServer");
    }

    fn physics_process(&mut self, delta: f64) {
        let nc = self
                .base()
                .get_tree()
                .unwrap()
                .get_root()
                .unwrap()
                .try_get_node_as::<NetworkController>("Root/NetworkController");
        let mainmenu = self.base()
        .get_tree()
        .unwrap()
        .get_root()
        .unwrap()
        .try_get_node_as::<Node>("Root/MainMenu");

        if let Some(button) = self.button_p2p.clone().as_mut() {
            if button.is_pressed() {
                godot_print!("P2P button pressed");
                nc.clone().unwrap().bind_mut().connect_to_server();
                //destory parent node
                mainmenu.clone().unwrap().queue_free();
            }
        }

        if let Some(button) = self.button_server.clone().as_mut() {
            if button.is_pressed() {
                godot_print!("Server button pressed");
                nc.clone().unwrap().bind_mut().time_out3 = 1;
                nc.clone().unwrap().bind_mut().connect_to_server();
                mainmenu.unwrap().queue_free();
            }
        }
    }
}
