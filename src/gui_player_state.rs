use godot::engine::Label;
use godot::prelude::*;
use godot::engine::Node2D;
use godot::engine::INode2D;

#[derive(GodotClass)]
#[class(base=Node2D)]
pub struct GUIPlayerState {
    base: Base<Node2D>,
    position_text: Option<Gd<Label>>,
    target: Option<Gd<Node2D>>,
}

impl GUIPlayerState {
    pub fn set_target(&mut self, target: Gd<Node2D>) {
        self.target = Some(target);
    }
}

#[godot_api]
impl INode2D for GUIPlayerState {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            base,
            position_text: None,
            target: None,
        }
    }

    fn ready(&mut self) {
        godot_print!("Ready");
        self.position_text = self.base().try_get_node_as::<Label>("Pos");
    }
    
    fn process(&mut self, _: f64) {
        if let Some(target) = self.target.clone() {
            if let Some(position_text) = self.position_text.clone().as_mut() {
                position_text.set_text(format!("Pos: {}, {}", target.get_position().x, target.get_position().y).into());
                //follow the target
                self.base().clone().set_position(target.get_position());
            }
        } else {
            godot_print!("No target set");
        }
    }
}