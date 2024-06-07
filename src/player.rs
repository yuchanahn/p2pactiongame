use godot::engine::AnimatedSprite2D;
use godot::engine::INode2D;
use godot::engine::Label;
use godot::engine::Node;
use godot::engine::Node2D;
use godot::engine::ProgressBar;
use godot::prelude::*;

use crate::animation_controller::PlayAnimationData;
use crate::game_manager::GAME_TICK;
use crate::gui_player_state::GUIPlayerState;

pub const MAX_SPEED: f32 = 5.0;
pub const ACCELERATION_SPEED: f32 = MAX_SPEED * 6.0;
pub const DECELERATION_SPEED: f32 = MAX_SPEED * 6.0;
pub const JUMP_VELOCITY: f32 = -30.0;
pub const TERMINAL_VELOCITY: f32 = 700.0;
pub const GRAVITY: f32 = 100.0;

#[derive(GodotClass, Debug)]
#[class(base=Node2D)]
pub struct Player {
    pub id: Option<u8>,
    pub animation_player: Option<Gd<AnimatedSprite2D>>,
    effect: Option<Gd<PackedScene>>,
    base: Base<Node2D>,
}

#[godot_api]
impl Player {
    #[func]
    pub fn set_gui(&mut self) {
        if let Ok(scene) = try_load::<PackedScene>("res://PlayerState.tscn") {
            let mut player_state = scene.instantiate_as::<GUIPlayerState>();
            player_state
                .bind_mut()
                .set_target(self.base().clone().upcast::<Node2D>());
            self.base()
                .get_node_as::<Node2D>("../")
                .add_child(player_state.upcast::<Node>());
        }
    }

    pub fn show_rollback_text(&mut self) {
        let mut rollback_text = self.base().get_node_as::<Label>("RollbackText");
        let mut modulate = rollback_text.get_modulate();
        modulate.a = 1.0;
        rollback_text.set_modulate(modulate);
    }

    pub fn gui_update(&mut self) {
        let mut player_health = self.base().get_node_as::<ProgressBar>("ProgressBar");
        //TODO: player_health.set_value(self.stat.health);
    }
}

#[godot_api]
impl INode2D for Player {
    fn init(base: Base<Node2D>) -> Self {
        Self {
            id: None,
            animation_player: None,
            effect: None,
            base,
        }
    }

    fn ready(&mut self) {
        self.animation_player = self
            .base()
            .try_get_node_as::<AnimatedSprite2D>("AnimatedSprite2D");
        //self.anim_data = Some(PlayAnimationData {
        //    name: "idle".into(), // "idle", "run", "attack
        //    started_at: GAME_TICK.lock().unwrap().tick,
        //    looped: true,
        //});
        
        self.effect = try_load::<PackedScene>("res://Effect/Hit/hit_1.tscn").ok();

        self.base_mut().call_deferred("set_gui".into(), &[]);
    }

    fn physics_process(&mut self, delta: f64) {
        let mut rollback_text = self.base().get_node_as::<Label>("RollbackText");
        let mut modulate = rollback_text.get_modulate();
        modulate.a -= 1.0 * delta as f32;
        rollback_text.set_modulate(modulate);
    }
}
