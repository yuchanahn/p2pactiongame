use godot::engine::AnimatedSprite2D;
use godot::engine::IAnimatedSprite2D;
use godot::prelude::*;

#[derive(GodotClass)]
#[class(base=AnimatedSprite2D)]
pub struct Effect {
    pub name: String,
    pub max_frame: i32,
    base: Base<AnimatedSprite2D>,
}

#[godot_api]
impl IAnimatedSprite2D for Effect {
    fn init(base: Base<AnimatedSprite2D>) -> Self {
        Self {
            name: "hit1".to_string(),
            max_frame: 0,
            base,
        }
    }

    fn ready(&mut self) {
        self.base_mut().play();
        self.max_frame = self.base()
            .get_sprite_frames()
            .as_mut()
            .unwrap()
            .get_frame_count(self.name.clone().into());

        godot_print!("Effect ready : {}, max frame : {}", self.name.clone(), self.max_frame);
    }

    fn physics_process(&mut self, delta: f64) {
        let mut modulate = self.base().get_modulate();
        modulate.a -= 0.2 * delta as f32;
        self.base_mut().set_modulate(modulate);

        if self.base().get_frame() == self.max_frame - 1 {
            self.base_mut().queue_free();
        } 
    }
}