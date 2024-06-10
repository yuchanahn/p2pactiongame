use godot::prelude::*;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

mod player;
mod udp_net;
mod gui_player_state;
mod network_controller;
mod time;
mod connect;
mod input_controller;
mod game_manager;
mod shape_cast2d;
mod effect;
mod utils;
mod main_menu;
mod world;
mod animation_controller;
mod col2d;
mod random;